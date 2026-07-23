use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::errors::ConcentratedLiquidityError;
use crate::math::{
    calculate_token_a_for_liquidity, calculate_token_b_for_liquidity, sqrt_price_x64_to_f64,
    tick_to_sqrt_price_x64,
};
use crate::state::{PoolState, TickArray};

/// Swaps are exact-input. The user pays `amount_in`, the program walks the pool
/// price through active concentrated liquidity, and the final output must meet
/// `minimum_amount_out`. Tick arrays are supplied as `remaining_accounts`
/// because the number needed depends on how far price moves.
#[derive(Accounts)]
pub struct Swap<'info> {
    /// User paying the input token and receiving the output token.
    #[account(mut)]
    pub swapper: Signer<'info>,

    /// Pool whose price, active liquidity, and fee-growth state are updated.
    ///
    /// `has_one` constraints bind the passed mints and vaults to this pool.
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

    /// Swapper token A account.
    ///
    /// Source for A-to-B swaps and destination for B-to-A swaps.
    #[account(
        mut,
        token::mint = token_a_mint,
        token::authority = swapper,
    )]
    pub user_token_a: Account<'info, TokenAccount>,

    /// Swapper token B account.
    ///
    /// Destination for A-to-B swaps and source for B-to-A swaps.
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

/// Exact-input CLMM swap. Rebuild the body from basic premises.
pub fn handler(
    ctx: Context<Swap>,
    amount_in: u64,
    minimum_amount_out: u64,
    a_to_b: bool,
) -> Result<()> {
    require!(
        amount_in > 0,
        ConcentratedLiquidityError::ZeroAmountSpecified
    );
    let pool_state = &ctx.accounts.pool_state;
    let mut remaining_amount = amount_in;
    let tick_spacing = pool_state.tick_spacing;
    let mut tick_arrays = Vec::with_capacity(ctx.remaining_accounts.len());
    for acc in ctx.remaining_accounts {
        let loader = AccountLoader::<TickArray>::try_from(acc)?;
        tick_arrays.push(*loader.load()?);
    }
    let fee_bps = pool_state.fee_bps;

    while remaining_amount > 0 {
        let current_tick = pool_state.current_tick;
        let mut best_tick: Option<(i32, usize, usize)> = None;

        for (array_index, tick_array) in tick_arrays.iter().enumerate() {
            for (offset, tick) in tick_array.ticks.iter().enumerate() {
                if tick.initialized == 0 {
                    continue;
                }

                let tick_index =
                    tick_array.start_tick_index + (offset as i32 * tick_spacing as i32);
                let is_candidate = if a_to_b {
                    tick_index <= current_tick // tick_index lower/higher = true/false
                } else {
                    tick_index > current_tick // tick_index lower/higher = false/true
                }; // `<=`  because we can cross downward through the current tick itself.

                if !is_candidate {
                    continue;
                }
                match best_tick {
                    None => {
                        best_tick = Some((tick_index, array_index, offset));
                    }
                    // Arrays are not sorted by start_tick_index in the input,
                    // so we must compare candidates across all arrays.
                    // A closer tick may appear in a later array.
                    Some((best_index, _, _)) => {
                        let is_closer = if a_to_b {
                            tick_index > best_index
                        } else {
                            tick_index < best_index
                        };

                        if is_closer {
                            best_tick = Some((tick_index, array_index, offset));
                        }
                    }
                }
            }
        }
        let (target_tick_index, _array_index, _offset) =
            best_tick.ok_or(ConcentratedLiquidityError::TickArrayNotFound)?;

        let sqrt_current = pool_state.sqrt_price_x64;
        let sqrt_target = tick_to_sqrt_price_x64(target_tick_index);

        let sqrt_current_f64 = sqrt_price_x64_to_f64(sqrt_current);
        let sqrt_target_f64 = sqrt_price_x64_to_f64(sqrt_target);

        // calculate swap amount
        let amount_in_to_reach_target = if a_to_b {
            calculate_token_a_for_liquidity(
                pool_state.liquidity,
                sqrt_current_f64,
                sqrt_target_f64,
                true,
            )?
        } else {
            calculate_token_b_for_liquidity(
                pool_state.liquidity,
                sqrt_current_f64,
                sqrt_target_f64,
                true,
            )?
        };
        let post_fee_rate = 10_000u64 - fee_bps as u64; // 9970 = 10000 - 30 <=> 100% - 0.3%
        let net_available = remaining_amount as u128 * post_fee_rate as u128 / 10_000u128;
        let (step_amount_in, swap_amount, step_fee) =
            if net_available >= amount_in_to_reach_target as u128 {
                let total = amount_in_to_reach_target as u128 * 10_000u128 / post_fee_rate as u128;
                let swap = amount_in_to_reach_target as u128;
                let fee = total - swap;
                (total, swap, fee)
            } else {
                let total = remaining_amount as u128;
                let swap = total * post_fee_rate as u128 / 10_000u128;
                let fee = total - swap;
                (total, swap, fee)
            };
    }
    Ok(())
}
