use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, transfer, Mint, MintTo, Token, TokenAccount, Transfer},
};

use crate::{
    errors::ConcentratedLiquidityError,
    math::{
        fee_growth_inside_for_ticks, initialize_tick_fee_growths, liquidity_quote, tick_array_start_index,
        tick_offset_in_array, update_tick_liquidity, validate_position_token_amounts, validate_tick_alignment,
    },
    state::{PoolState, Position, TickArray},
};

#[derive(Accounts)]
pub struct CreatePosition<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

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

    #[account(
        init,
        payer = owner,
        space = 8 + Position::INIT_SPACE,
        seeds = [b"position", position_mint.key().as_ref()],
        bump
    )]
    pub position: Account<'info, Position>,

    #[account(
        init,
        payer = owner,
        mint::decimals = 0,
        mint::authority = pool_state,
        mint::freeze_authority = pool_state,
    )]
    pub position_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = owner,
        associated_token::mint = position_mint,
        associated_token::authority = owner,
    )]
    pub owner_position_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = token_a_mint,
        token::authority = owner,
    )]
    pub owner_token_a: Account<'info, TokenAccount>,

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
    amount_a_max: u64,
    amount_b_max: u64,
) -> Result<()> {
    require!(tick_lower < tick_upper, ConcentratedLiquidityError::InvalidTickRange);

    let current_tick = ctx.accounts.pool_state.current_tick;
    let tick_spacing = ctx.accounts.pool_state.tick_spacing;
    let sqrt_price_x64 = ctx.accounts.pool_state.sqrt_price_x64;
    let fee_growth_global_a_x64 = ctx.accounts.pool_state.fee_growth_global_a_x64;
    let fee_growth_global_b_x64 = ctx.accounts.pool_state.fee_growth_global_b_x64;
    validate_position_token_amounts(current_tick, tick_lower, tick_upper, amount_a_max, amount_b_max)?;
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
            &ctx.accounts.pool_state,
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
