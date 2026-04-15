use anchor_lang::prelude::*;
use anchor_spl::token::{
    burn, close_account, transfer, Burn, CloseAccount, Mint, Token, TokenAccount, Transfer,
};

use crate::{
    errors::ConcentratedLiquidityError,
    math::{
        accrue_position_fees, amounts_for_liquidity, fee_growth_inside_for_ticks, tick_offset_in_array,
        update_tick_liquidity,
    },
    state::{PoolState, Position, TickArray},
};

#[derive(Accounts)]
pub struct ClosePosition<'info> {
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
        mut,
        close = owner,
        constraint = position.pool == pool_state.key() @ ConcentratedLiquidityError::PositionPoolMismatch
    )]
    pub position: Account<'info, Position>,

    #[account(mut, constraint = position_mint.key() == position.position_mint @ ConcentratedLiquidityError::InvalidPositionTokenAccount)]
    pub position_mint: Account<'info, Mint>,

    #[account(
        mut,
        token::mint = position_mint,
        token::authority = owner,
        constraint = owner_position_token_account.amount == 1 @ ConcentratedLiquidityError::InvalidPositionTokenAccount
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
}

pub fn handler(ctx: Context<ClosePosition>) -> Result<()> {
    let current_tick = ctx.accounts.pool_state.current_tick;
    let tick_spacing = ctx.accounts.pool_state.tick_spacing;
    let sqrt_price_x64 = ctx.accounts.pool_state.sqrt_price_x64;
    let position_tick_lower = ctx.accounts.position.tick_lower;
    let position_tick_upper = ctx.accounts.position.tick_upper;
    let position_liquidity = ctx.accounts.position.liquidity_amount;

    let fee_growth_inside = {
        let lower_tick_snapshot = {
            let tick_array_lower = ctx.accounts.tick_array_lower.load()?;
            let lower_offset = tick_offset_in_array(
                tick_array_lower.start_tick_index,
                position_tick_lower,
                tick_spacing,
            )?;
            tick_array_lower.ticks[lower_offset]
        };
        let upper_tick_snapshot = {
            let tick_array_upper = ctx.accounts.tick_array_upper.load()?;
            let upper_offset = tick_offset_in_array(
                tick_array_upper.start_tick_index,
                position_tick_upper,
                tick_spacing,
            )?;
            tick_array_upper.ticks[upper_offset]
        };

        fee_growth_inside_for_ticks(
            &ctx.accounts.pool_state,
            position_tick_lower,
            &lower_tick_snapshot,
            position_tick_upper,
            &upper_tick_snapshot,
        )
    };

    let (amount_a_liquidity, amount_b_liquidity, fees_a_owed, fees_b_owed) = {
        let position = &mut ctx.accounts.position;
        accrue_position_fees(position, fee_growth_inside)?;
        let (amount_a_liquidity, amount_b_liquidity) = amounts_for_liquidity(
            position.liquidity_amount,
            position.tick_lower,
            position.tick_upper,
            sqrt_price_x64,
        )?;
        (
            amount_a_liquidity,
            amount_b_liquidity,
            position.fees_a_owed,
            position.fees_b_owed,
        )
    };

    let signed_liquidity = i128::try_from(position_liquidity)
        .map_err(|_| ConcentratedLiquidityError::TickMathOverflow)?
        .checked_neg()
        .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;

    {
        let mut tick_array_lower = ctx.accounts.tick_array_lower.load_mut()?;
        let lower_offset =
            tick_offset_in_array(tick_array_lower.start_tick_index, position_tick_lower, tick_spacing)?;
        let lower_tick = &mut tick_array_lower.ticks[lower_offset];
        update_tick_liquidity(lower_tick, signed_liquidity, false)?;
    }

    {
        let mut tick_array_upper = ctx.accounts.tick_array_upper.load_mut()?;
        let upper_offset =
            tick_offset_in_array(tick_array_upper.start_tick_index, position_tick_upper, tick_spacing)?;
        let upper_tick = &mut tick_array_upper.ticks[upper_offset];
        update_tick_liquidity(upper_tick, signed_liquidity, true)?;
    }

    if position_tick_lower <= current_tick && current_tick < position_tick_upper {
        let pool_state = &mut ctx.accounts.pool_state;
        pool_state.liquidity = pool_state
            .liquidity
            .checked_sub(position_liquidity)
            .ok_or(ConcentratedLiquidityError::NegativeLiquidity)?;
    }

    let amount_a_total_u128 = u128::from(amount_a_liquidity)
        .checked_add(fees_a_owed)
        .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
    let amount_b_total_u128 = u128::from(amount_b_liquidity)
        .checked_add(fees_b_owed)
        .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
    let amount_a_total =
        u64::try_from(amount_a_total_u128).map_err(|_| ConcentratedLiquidityError::TickMathOverflow)?;
    let amount_b_total =
        u64::try_from(amount_b_total_u128).map_err(|_| ConcentratedLiquidityError::TickMathOverflow)?;

    let token_a_mint_key = ctx.accounts.token_a_mint.key();
    let token_b_mint_key = ctx.accounts.token_b_mint.key();
    let signer_seeds: &[&[u8]] = &[
        b"pool",
        token_a_mint_key.as_ref(),
        token_b_mint_key.as_ref(),
        &[ctx.accounts.pool_state.bump],
    ];

    if amount_a_total > 0 {
        let transfer_a_accounts = Transfer {
            from: ctx.accounts.token_a_vault.to_account_info(),
            to: ctx.accounts.owner_token_a.to_account_info(),
            authority: ctx.accounts.pool_state.to_account_info(),
        };
        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                transfer_a_accounts,
                &[signer_seeds],
            ),
            amount_a_total,
        )?;
    }

    if amount_b_total > 0 {
        let transfer_b_accounts = Transfer {
            from: ctx.accounts.token_b_vault.to_account_info(),
            to: ctx.accounts.owner_token_b.to_account_info(),
            authority: ctx.accounts.pool_state.to_account_info(),
        };
        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                transfer_b_accounts,
                &[signer_seeds],
            ),
            amount_b_total,
        )?;
    }

    burn(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.position_mint.to_account_info(),
                from: ctx.accounts.owner_position_token_account.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            },
        ),
        1,
    )?;

    close_account(CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        CloseAccount {
            account: ctx.accounts.owner_position_token_account.to_account_info(),
            destination: ctx.accounts.owner.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        },
    ))?;

    Ok(())
}
