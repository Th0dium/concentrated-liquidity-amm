use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::errors::ConcentratedLiquidityError;
use crate::math::{tick_array_start_index, tick_offset_in_array};
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
    let mut remaining_amount = amount_in;
    let tick_spacing = ctx.accounts.pool_state.tick_spacing;
    let mut tick_arrays = Vec::with_capacity(ctx.remaining_accounts.len());
    for acc in ctx.remaining_accounts {
        let loader = AccountLoader::<TickArray>::try_from(acc)?;
        tick_arrays.push(*loader.load()?);
    }

    while remaining_amount > 0 {
        let current_tick_index = ctx.accounts.pool_state.current_tick;
        let mut tick_found = false;
        let mut target_tick_index = current_tick_index;

        while !tick_found {
            target_tick_index = if a_to_b {
                target_tick_index - tick_spacing as i32
            } else {
                target_tick_index + tick_spacing as i32
            };
            let array_start_index = tick_array_start_index(target_tick_index, tick_spacing)?;

            let tick_array = tick_arrays
                .iter()
                .find(|arr| arr.start_tick_index == array_start_index)
                .ok_or(ConcentratedLiquidityError::TickArrayNotFound)?;
            let array_offset =
                tick_offset_in_array(array_start_index, target_tick_index, tick_spacing)?;
            let tick = &tick_array.ticks[array_offset];
            if tick.initialized {
                tick_found = true;
            }
        }
    }
}
