use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, transfer, Mint, MintTo, Token, TokenAccount, Transfer},
};

use crate::{
    errors::ConcentratedLiquidityError,
    math::{
        liquidity_from_amounts, sqrt_price_x64_to_tick, tick_array_start_index,
        tick_offset_in_array, validate_position_token_amounts, validate_tick_alignment,
    },
    state::{PoolState, Position, TickArray},
};

#[derive(Accounts)]
pub struct CreatePosition<'info> {
    /// LP (liquidity provider) creating the position and paying for accounts
    #[account(mut)]
    pub owner: Signer<'info>,

    /// Pool state (must match token mints and vaults)
    #[account(
        mut,
        has_one = token_a_mint,
        has_one = token_b_mint,
        has_one = token_a_vault,
        has_one = token_b_vault,
    )]
    pub pool_state: Account<'info, PoolState>,

    /// Token mint (read-only, used for validation)
    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,

    /// Position PDA storing LP's liquidity metadata
    /// Seeds: [b"position", position_mint]
    /// Unique per position_mint, enables NFT-like transferability
    #[account(
        init,
        payer = owner,
        space = 8 + Position::INIT_SPACE,
        seeds = [
            b"position",
            position_mint.key().as_ref(),
        ],
        bump
    )]
    pub position: Account<'info, Position>,

    /// Unique mint for this position (NFT-like: decimals=0, supply=1)
    /// Authority: pool_state PDA (only program can mint)
    /// Serves as both unique identifier and transferable ownership token
    #[account(
        init,
        payer = owner,
        mint::decimals = 0,
        mint::authority = pool_state,
        mint::freeze_authority = pool_state,
    )]
    pub position_mint: Account<'info, Mint>,

    /// Owner's token account to receive the position NFT
    /// Will hold exactly 1 token after minting
    #[account(
        init,
        payer = owner,
        associated_token::mint = position_mint,
        associated_token::authority = owner,
    )]
    pub owner_position_token_account: Account<'info, TokenAccount>,

    /// Owner's token A account (will decrease by amount_a)
    #[account(
        mut,
        token::mint = token_a_mint,
        token::authority = owner,
    )]
    pub owner_token_a: Account<'info, TokenAccount>,

    /// Owner's token B account (will decrease by amount_b)
    #[account(
        mut,
        token::mint = token_b_mint,
        token::authority = owner,
    )]
    pub owner_token_b: Account<'info, TokenAccount>,

    /// Pool's token A vault (will increase by amount_a)
    #[account(mut)]
    pub token_a_vault: Account<'info, TokenAccount>,

    /// Pool's token B vault (will increase by amount_b)
    #[account(mut)]
    pub token_b_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = tick_array_lower.load()?.pool == pool_state.key()
            @ ConcentratedLiquidityError::InvalidTickArrayStart
    )]
    pub tick_array_lower: AccountLoader<'info, TickArray>,

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

pub fn handler(
    ctx: Context<CreatePosition>,
    tick_lower: i32,
    tick_upper: i32,
    amount_a: u64,
    amount_b: u64,
) -> Result<()> {
    // Validate tick range (lower must be strictly less than upper)
    require!(tick_lower < tick_upper, ConcentratedLiquidityError::InvalidTickRange);

    let current_tick = sqrt_price_x64_to_tick(ctx.accounts.pool_state.sqrt_price_x64);
    validate_position_token_amounts(current_tick, tick_lower, tick_upper, amount_a, amount_b)?;
    validate_tick_alignment(tick_lower, ctx.accounts.pool_state.tick_spacing)
        .map_err(|_| ConcentratedLiquidityError::TickNotAligned)?;
    validate_tick_alignment(tick_upper, ctx.accounts.pool_state.tick_spacing)
        .map_err(|_| ConcentratedLiquidityError::TickNotAligned)?;

    let lower_start = tick_array_start_index(tick_lower, ctx.accounts.pool_state.tick_spacing)
        .map_err(|_| ConcentratedLiquidityError::InvalidTickArrayStart)?;
    let upper_start = tick_array_start_index(tick_upper, ctx.accounts.pool_state.tick_spacing)
        .map_err(|_| ConcentratedLiquidityError::InvalidTickArrayStart)?;

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

    // Transfer token A from owner to pool vault
    // CPI to SPL Token Program: transfer(from, to, authority, amount)
    if amount_a > 0 {
        let cpi_accounts_a = Transfer {
            from: ctx.accounts.owner_token_a.to_account_info(),
            to: ctx.accounts.token_a_vault.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        transfer(
            CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts_a),
            amount_a,
        )?;
    }

    // Transfer token B from owner to pool vault
    if amount_b > 0 {
        let cpi_accounts_b = Transfer {
            from: ctx.accounts.owner_token_b.to_account_info(),
            to: ctx.accounts.token_b_vault.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        transfer(
            CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts_b),
            amount_b,
        )?;
    }

    // Mint 1 position NFT to owner
    // CPI to SPL Token Program with PDA signer (pool_state signs on behalf of program)
    // Seeds: [b"pool", token_a_mint, token_b_mint, bump]
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
        1, // Mint exactly 1 NFT-like token
    )?;

    // Placeholder math interface for future concentrated-liquidity formulas.
    let liquidity_amount = liquidity_from_amounts(
        amount_a,
        amount_b,
        tick_lower,
        tick_upper,
        ctx.accounts.pool_state.sqrt_price_x64,
    );
    let signed_liquidity =
        i128::try_from(liquidity_amount).map_err(|_| ConcentratedLiquidityError::TickMathOverflow)?;

    // Initialize position state
    let position = &mut ctx.accounts.position;
    position.bump = ctx.bumps.position;
    position.position_mint = ctx.accounts.position_mint.key();
    position.owner = ctx.accounts.owner.key();
    position.pool = pool_state_key;
    position.tick_lower = tick_lower;
    position.tick_upper = tick_upper;
    position.liquidity_amount = liquidity_amount;
    position.fees_a_owed = 0;
    position.fees_b_owed = 0;

    // Tick boundary updates: lower adds liquidity, upper removes liquidity.
    {
        let mut tick_array_lower = ctx.accounts.tick_array_lower.load_mut()?;
        let lower_offset = tick_offset_in_array(
            tick_array_lower.start_tick_index,
            tick_lower,
            ctx.accounts.pool_state.tick_spacing,
        )
        .map_err(|_| ConcentratedLiquidityError::TickIndexOutOfBounds)?;

        let lower_tick = &mut tick_array_lower.ticks[lower_offset];
        lower_tick.initialized = 1;
        lower_tick.liquidity_gross = lower_tick
            .liquidity_gross
            .checked_add(liquidity_amount)
            .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
        lower_tick.liquidity_net = lower_tick
            .liquidity_net
            .checked_add(signed_liquidity)
            .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
    }

    {
        let mut tick_array_upper = ctx.accounts.tick_array_upper.load_mut()?;
        let upper_offset = tick_offset_in_array(
            tick_array_upper.start_tick_index,
            tick_upper,
            ctx.accounts.pool_state.tick_spacing,
        )
        .map_err(|_| ConcentratedLiquidityError::TickIndexOutOfBounds)?;

        let upper_tick = &mut tick_array_upper.ticks[upper_offset];
        upper_tick.initialized = 1;
        upper_tick.liquidity_gross = upper_tick
            .liquidity_gross
            .checked_add(liquidity_amount)
            .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
        upper_tick.liquidity_net = upper_tick
            .liquidity_net
            .checked_sub(signed_liquidity)
            .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
    }

    // Update active pool liquidity only if current price is in this range.
    let pool_state = &mut ctx.accounts.pool_state;
    if tick_lower <= current_tick && current_tick < tick_upper {
        pool_state.liquidity = pool_state
            .liquidity
            .checked_add(liquidity_amount)
            .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
    }

    Ok(())
}
