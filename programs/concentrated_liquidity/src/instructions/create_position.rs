use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, transfer, Mint, MintTo, Token, TokenAccount, Transfer},
};

use crate::{
    errors::ConcentratedLiquidityError,
    math::{
        fee_growth_inside_for_ticks, initialize_tick_fee_growths, liquidity_quote,
        tick_array_start_index, tick_offset_in_array, update_tick_liquidity,
        validate_position_token_amounts, validate_tick_alignment,
    },
    state::{PoolState, Position, TickArray},
};

/// This instruction turns an LP's token budgets into concentrated liquidity for
/// one pool. It validates the requested tick range, quotes the liquidity that
/// can be created at the current pool price, transfers only the consumed token
/// amounts into the pool vaults, creates the position PDA, mints the position
/// ownership token, and writes the lower/upper tick boundary liquidity deltas.
///
/// The position PDA is protocol accounting state. The decimals-0 position mint
/// and the owner's associated token account are the ownership layer used later
/// by `close_position`.
#[derive(Accounts)]
pub struct CreatePosition<'info> {
    /// LP wallet creating the position.
    ///
    /// The owner pays rent for the position PDA, position mint, and position
    /// token account. This signer also authorizes token A/B transfers from the
    /// owner's token accounts into the pool vaults.
    #[account(mut)]
    pub owner: Signer<'info>,

    /// Pool receiving the new liquidity.
    ///
    /// The `has_one` constraints make the supplied mints and vaults match the
    /// addresses stored in the pool state, preventing a client from mixing pool
    /// state with unrelated token accounts.
    #[account(
        mut,
        has_one = token_a_mint,
        has_one = token_b_mint,
        has_one = token_a_vault,
        has_one = token_b_vault,
    )]
    pub pool_state: Account<'info, PoolState>,

    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,

    /// Position PDA storing the LP's protocol accounting.
    ///
    /// Seeds: `[b"position", position_mint]`.
    /// The position mint is the unique identifier, so users do not need to
    /// coordinate position ids and ownership can be transferred via SPL token
    /// ownership.
    #[account(
        init,
        payer = owner,
        space = 8 + Position::INIT_SPACE,
        seeds = [b"position", position_mint.key().as_ref()],
        bump
    )]
    pub position: Account<'info, Position>,

    /// New NFT-like SPL mint for this position.
    ///
    /// The mint has `decimals = 0`; the handler mints exactly one token to the
    /// owner's position token account. The pool PDA is mint/freeze authority so
    /// the program controls the position-token supply.
    #[account(
        init,
        payer = owner,
        mint::decimals = 0,
        mint::authority = pool_state,
        mint::freeze_authority = pool_state,
    )]
    pub position_mint: Account<'info, Mint>,

    /// Owner's associated token account for the new position mint.
    ///
    /// Holding one token from `position_mint` is what proves ownership when the
    /// position is later closed.
    #[account(
        init,
        payer = owner,
        associated_token::mint = position_mint,
        associated_token::authority = owner,
    )]
    pub owner_position_token_account: Account<'info, TokenAccount>,

    /// Owner's token A account used as a possible liquidity source.
    ///
    /// The account must hold token A and be owned by `owner`. Depending on the
    /// current price relative to the range, the actual transfer can be zero.
    #[account(
        mut,
        token::mint = token_a_mint,
        token::authority = owner,
    )]
    pub owner_token_a: Account<'info, TokenAccount>,

    /// Owner's token B account used as a possible liquidity source.
    ///
    /// In-range positions consume both tokens. Ranges below or above the current
    /// price can be one-sided and consume only one of the two tokens.
    #[account(
        mut,
        token::mint = token_b_mint,
        token::authority = owner,
    )]
    pub owner_token_b: Account<'info, TokenAccount>,

    #[account(mut)]
    pub token_a_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub token_b_vault: Account<'info, TokenAccount>,

    /// Tick array containing `tick_lower`.
    ///
    /// Mutable because the lower boundary tick may be initialized and its
    /// `liquidity_net`, `liquidity_gross`, and fee-growth-outside checkpoints
    /// may be updated.
    #[account(
        mut,
        constraint = tick_array_lower.load()?.pool == pool_state.key()
            @ ConcentratedLiquidityError::InvalidTickArrayStart
    )]
    pub tick_array_lower: AccountLoader<'info, TickArray>,

    /// Tick array containing `tick_upper`.
    ///
    /// Mutable for the same reason as the lower array: the upper boundary is
    /// part of the position's range accounting and later swap crossing logic.
    #[account(
        mut,
        constraint = tick_array_upper.load()?.pool == pool_state.key()
            @ ConcentratedLiquidityError::InvalidTickArrayStart
    )]
    pub tick_array_upper: AccountLoader<'info, TickArray>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

/// Create a new LP position over `[tick_lower, tick_upper)`.
///
/// The tick range is half-open: `tick_lower` is inclusive and `tick_upper` is
/// exclusive. Both ticks must be aligned to the pool's raw `tick_spacing`, and
/// both tick arrays must already be initialized.
///
/// `amount_a_max` and `amount_b_max` are maximum budgets from the LP, not fixed
/// deposit amounts. The handler quotes the maximum liquidity that can be minted
/// from those budgets at the current sqrt price, then transfers only the token
/// amounts actually needed. This preserves normal CLMM behavior where a range
/// fully below or above the current price is one-sided.
///
/// The position stores fee-growth-inside checkpoints at creation time. Later
/// close/claim logic uses those checkpoints to calculate only the fees earned
/// after this position was opened. The lower and upper ticks store the liquidity
/// deltas that swaps use when crossing into or out of this range.
///
/// # Arguments
/// * `tick_lower` - Inclusive lower raw tick boundary.
/// * `tick_upper` - Exclusive upper raw tick boundary.
/// * `amount_a_max` - Maximum token A amount the LP is willing to deposit.
/// * `amount_b_max` - Maximum token B amount the LP is willing to deposit.
pub fn handler(
    ctx: Context<CreatePosition>,
    tick_lower: i32,
    tick_upper: i32,
    amount_a_max: u64,
    amount_b_max: u64,
) -> Result<()> {
    require!(
        tick_lower < tick_upper,
        ConcentratedLiquidityError::InvalidTickRange
    );

    let current_tick = ctx.accounts.pool_state.current_tick;
    let tick_spacing = ctx.accounts.pool_state.tick_spacing;
    let sqrt_price_x64 = ctx.accounts.pool_state.sqrt_price_x64;
    let fee_growth_global_a_x64 = ctx.accounts.pool_state.fee_growth_global_a_x64;
    let fee_growth_global_b_x64 = ctx.accounts.pool_state.fee_growth_global_b_x64;
    validate_position_token_amounts(
        current_tick,
        tick_lower,
        tick_upper,
        amount_a_max,
        amount_b_max,
    )?;
    validate_tick_alignment(tick_lower, tick_spacing)?;
    validate_tick_alignment(tick_upper, tick_spacing)?;

    let lower_start = tick_array_start_index(tick_lower, tick_spacing)?;
    let upper_start = tick_array_start_index(tick_upper, tick_spacing)?;

    {
        let tick_array_lower = ctx.accounts.tick_array_lower.load()?;
        require!(
            tick_array_lower.start_tick_index == lower_start,
            ConcentratedLiquidityError::InvalidTickArrayStart
        );
    }
    {
        let tick_array_upper = ctx.accounts.tick_array_upper.load()?;
        require!(
            tick_array_upper.start_tick_index == upper_start,
            ConcentratedLiquidityError::InvalidTickArrayStart
        );
    }

    let quote = liquidity_quote(
        amount_a_max,
        amount_b_max,
        tick_lower,
        tick_upper,
        sqrt_price_x64,
    )?;
    let signed_liquidity = i128::try_from(quote.liquidity_delta)
        .map_err(|_| ConcentratedLiquidityError::TickMathOverflow)?;

    let fee_growth_inside = {
        let lower_tick_snapshot = {
            let tick_array_lower = ctx.accounts.tick_array_lower.load()?;
            let lower_offset =
                tick_offset_in_array(tick_array_lower.start_tick_index, tick_lower, tick_spacing)?;
            let mut tick = tick_array_lower.ticks[lower_offset];
            initialize_tick_fee_growths(
                &mut tick,
                tick_lower,
                current_tick,
                fee_growth_global_a_x64,
                fee_growth_global_b_x64,
            );
            tick
        };
        let upper_tick_snapshot = {
            let tick_array_upper = ctx.accounts.tick_array_upper.load()?;
            let upper_offset =
                tick_offset_in_array(tick_array_upper.start_tick_index, tick_upper, tick_spacing)?;
            let mut tick = tick_array_upper.ticks[upper_offset];
            initialize_tick_fee_growths(
                &mut tick,
                tick_upper,
                current_tick,
                fee_growth_global_a_x64,
                fee_growth_global_b_x64,
            );
            tick
        };

        fee_growth_inside_for_ticks(
            &*ctx.accounts.pool_state,
            tick_lower,
            &lower_tick_snapshot,
            tick_upper,
            &upper_tick_snapshot,
        )
    };

    if quote.amount_a > 0 {
        let cpi_accounts_a = Transfer {
            from: ctx.accounts.owner_token_a.to_account_info(),
            to: ctx.accounts.token_a_vault.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        transfer(
            CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts_a),
            quote.amount_a,
        )?;
    }

    if quote.amount_b > 0 {
        let cpi_accounts_b = Transfer {
            from: ctx.accounts.owner_token_b.to_account_info(),
            to: ctx.accounts.token_b_vault.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        transfer(
            CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts_b),
            quote.amount_b,
        )?;
    }

    let pool_state_key = ctx.accounts.pool_state.key();
    let token_a_mint_key = ctx.accounts.token_a_mint.key();
    let token_b_mint_key = ctx.accounts.token_b_mint.key();
    let signer_seeds: &[&[u8]] = &[
        b"pool",
        token_a_mint_key.as_ref(),
        token_b_mint_key.as_ref(),
        &[ctx.accounts.pool_state.bump],
    ];
    let mint_to_accounts = MintTo {
        mint: ctx.accounts.position_mint.to_account_info(),
        to: ctx.accounts.owner_position_token_account.to_account_info(),
        authority: ctx.accounts.pool_state.to_account_info(),
    };
    mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            mint_to_accounts,
            &[signer_seeds],
        ),
        1,
    )?;

    let position = &mut ctx.accounts.position;
    position.bump = ctx.bumps.position;
    position.position_mint = ctx.accounts.position_mint.key();
    position.owner = ctx.accounts.owner.key();
    position.pool = pool_state_key;
    position.tick_lower = tick_lower;
    position.tick_upper = tick_upper;
    position.liquidity_amount = quote.liquidity_delta;
    position.fee_growth_checkpoint_a_x64 = fee_growth_inside.token_a_x64;
    position.fee_growth_checkpoint_b_x64 = fee_growth_inside.token_b_x64;
    position.fees_a_owed = 0;
    position.fees_b_owed = 0;

    {
        let mut tick_array_lower = ctx.accounts.tick_array_lower.load_mut()?;
        let lower_offset =
            tick_offset_in_array(tick_array_lower.start_tick_index, tick_lower, tick_spacing)?;
        let lower_tick = &mut tick_array_lower.ticks[lower_offset];
        initialize_tick_fee_growths(
            lower_tick,
            tick_lower,
            current_tick,
            fee_growth_global_a_x64,
            fee_growth_global_b_x64,
        );
        update_tick_liquidity(lower_tick, signed_liquidity, false)?;
    }

    {
        let mut tick_array_upper = ctx.accounts.tick_array_upper.load_mut()?;
        let upper_offset =
            tick_offset_in_array(tick_array_upper.start_tick_index, tick_upper, tick_spacing)?;
        let upper_tick = &mut tick_array_upper.ticks[upper_offset];
        initialize_tick_fee_growths(
            upper_tick,
            tick_upper,
            current_tick,
            fee_growth_global_a_x64,
            fee_growth_global_b_x64,
        );
        update_tick_liquidity(upper_tick, signed_liquidity, true)?;
    }

    if tick_lower <= current_tick && current_tick < tick_upper {
        let pool_state = &mut ctx.accounts.pool_state;
        pool_state.liquidity = pool_state
            .liquidity
            .checked_add(quote.liquidity_delta)
            .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
    }

    Ok(())
}
