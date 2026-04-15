use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

use crate::{
    errors::ConcentratedLiquidityError,
    math::{
        add_fee_growth, apply_liquidity_delta, compute_swap_step, cross_tick, find_next_initialized_tick,
        sqrt_price_x64_to_tick, tick_to_sqrt_price_x64, FeeSide,
    },
    state::{PoolState, TickArray},
};

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub swapper: Signer<'info>,

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
        token::mint = token_a_mint,
        token::authority = swapper,
    )]
    pub user_token_a: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = token_b_mint,
        token::authority = swapper,
    )]
    pub user_token_b: Account<'info, TokenAccount>,

    #[account(mut)]
    pub token_a_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub token_b_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(
    ctx: Context<Swap>,
    amount_in: u64,
    minimum_amount_out: u64,
    a_to_b: bool,
) -> Result<()> {
    require!(amount_in > 0, ConcentratedLiquidityError::ZeroAmountSpecified);

    let tick_arrays = load_tick_arrays(&ctx.remaining_accounts)?;
    require!(!tick_arrays.is_empty(), ConcentratedLiquidityError::MissingTickArrayForSwap);

    let pool_key = ctx.accounts.pool_state.key();
    for tick_array in &tick_arrays {
        require!(tick_array.pool == pool_key, ConcentratedLiquidityError::InvalidTickArrayStart);
    }

    let pool_state = &mut ctx.accounts.pool_state;
    require!(pool_state.liquidity > 0, ConcentratedLiquidityError::NoActiveLiquidity);

    let fee_side = if a_to_b { FeeSide::TokenA } else { FeeSide::TokenB };
    let mut amount_remaining = amount_in;
    let mut amount_out_total = 0u64;

    while amount_remaining > 0 {
        require!(pool_state.liquidity > 0, ConcentratedLiquidityError::NoActiveLiquidity);

        let next_crossing = find_next_initialized_tick(
            &tick_arrays,
            pool_state.current_tick,
            pool_state.tick_spacing,
            a_to_b,
        )
        .ok_or(ConcentratedLiquidityError::MissingTickArrayForSwap)?;

        let target_sqrt_price_x64 = tick_to_sqrt_price_x64(next_crossing.tick_index);
        let step = compute_swap_step(
            pool_state.sqrt_price_x64,
            target_sqrt_price_x64,
            pool_state.liquidity,
            amount_remaining,
            pool_state.fee_bps,
            a_to_b,
        )?;

        amount_remaining = amount_remaining
            .checked_sub(step.amount_in)
            .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
        amount_out_total = amount_out_total
            .checked_add(step.amount_out)
            .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;

        pool_state.sqrt_price_x64 = step.next_sqrt_price_x64;
        add_fee_growth(pool_state, fee_side, step.fee_amount)?;

        if step.reached_target_tick {
            let tick_array_info = &ctx.remaining_accounts[next_crossing.tick_array_list_index];
            let tick_array_loader: AccountLoader<TickArray> = AccountLoader::try_from(tick_array_info)?;
            let mut tick_array = tick_array_loader.load_mut()?;
            let tick = &mut tick_array.ticks[next_crossing.tick_offset];
            let liquidity_net = cross_tick(pool_state, tick, a_to_b)?;
            pool_state.liquidity = apply_liquidity_delta(pool_state.liquidity, liquidity_net)?;
            pool_state.current_tick = if a_to_b {
                next_crossing
                    .tick_index
                    .checked_sub(1)
                    .ok_or(ConcentratedLiquidityError::TickMathOverflow)?
            } else {
                next_crossing.tick_index
            };
        } else {
            pool_state.current_tick = sqrt_price_x64_to_tick(pool_state.sqrt_price_x64);
        }
    }

    require!(
        amount_out_total >= minimum_amount_out,
        ConcentratedLiquidityError::SlippageExceeded
    );

    let transfer_in_accounts = if a_to_b {
        Transfer {
            from: ctx.accounts.user_token_a.to_account_info(),
            to: ctx.accounts.token_a_vault.to_account_info(),
            authority: ctx.accounts.swapper.to_account_info(),
        }
    } else {
        Transfer {
            from: ctx.accounts.user_token_b.to_account_info(),
            to: ctx.accounts.token_b_vault.to_account_info(),
            authority: ctx.accounts.swapper.to_account_info(),
        }
    };
    transfer(
        CpiContext::new(ctx.accounts.token_program.to_account_info(), transfer_in_accounts),
        amount_in,
    )?;

    let token_a_mint_key = ctx.accounts.token_a_mint.key();
    let token_b_mint_key = ctx.accounts.token_b_mint.key();
    let signer_seeds: &[&[u8]] = &[
        b"pool",
        token_a_mint_key.as_ref(),
        token_b_mint_key.as_ref(),
        &[pool_state.bump],
    ];

    let transfer_out_accounts = if a_to_b {
        Transfer {
            from: ctx.accounts.token_b_vault.to_account_info(),
            to: ctx.accounts.user_token_b.to_account_info(),
            authority: ctx.accounts.pool_state.to_account_info(),
        }
    } else {
        Transfer {
            from: ctx.accounts.token_a_vault.to_account_info(),
            to: ctx.accounts.user_token_a.to_account_info(),
            authority: ctx.accounts.pool_state.to_account_info(),
        }
    };
    transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            transfer_out_accounts,
            &[signer_seeds],
        ),
        amount_out_total,
    )?;

    Ok(())
}

fn load_tick_arrays(remaining_accounts: &[AccountInfo<'_>]) -> Result<Vec<TickArray>> {
    let mut tick_arrays = Vec::with_capacity(remaining_accounts.len());
    for account_info in remaining_accounts {
        let loader: AccountLoader<TickArray> = AccountLoader::try_from(account_info)?;
        tick_arrays.push(*loader.load()?);
    }
    Ok(tick_arrays)
}
