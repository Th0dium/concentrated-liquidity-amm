use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

use crate::{
    errors::ConcentratedLiquidityError,
    math::{
        add_fee_growth, apply_liquidity_delta, compute_swap_step, cross_tick,
        find_next_initialized_tick, sqrt_price_x64_to_tick, tick_to_sqrt_price_x64, FeeSide,
    },
    state::{PoolState, TickArray},
};

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

/// Execute an exact-input CLMM swap.
///
/// The loop repeatedly finds the next initialized tick in the swap direction,
/// computes one swap step toward that boundary, records input-token fees in the
/// global fee-growth accumulator, and either crosses the tick or stops inside
/// the current liquidity range. Crossing a tick applies `liquidity_net`, which
/// is how only currently in-range positions affect price movement.
///
/// Token movement happens after slippage validation: the full input is
/// transferred from the swapper into the input vault, and the pool PDA signs the
/// output transfer from the opposite vault.
pub fn handler(
    ctx: Context<Swap>,
    amount_in: u64,
    minimum_amount_out: u64,
    a_to_b: bool,
) -> Result<()> {
    // Validate swap input and load the tick arrays used for traversal.
    require!(
        amount_in > 0,
        ConcentratedLiquidityError::ZeroAmountSpecified
    );

    let tick_arrays = load_tick_arrays(&ctx.remaining_accounts)?; // Copy tick arrays for read-only search
    require!(
        !tick_arrays.is_empty(),
        ConcentratedLiquidityError::MissingTickArrayForSwap
    );

    let pool_key = ctx.accounts.pool_state.key();
    for tick_array in &tick_arrays {
        require!(
            tick_array.pool == pool_key,
            ConcentratedLiquidityError::InvalidTickArrayStart
        ); // Prevent wrong pool's tick arrays
    }

    let pool_state = &mut ctx.accounts.pool_state;
    require!(
        pool_state.liquidity > 0,
        ConcentratedLiquidityError::NoActiveLiquidity
    );

    let fee_side = if a_to_b {
        FeeSide::TokenA
    } else {
        FeeSide::TokenB
    }; // Fee collected from input token
    let mut amount_remaining = amount_in; // Track unconsumed input
    let mut amount_out_total = 0u64; // Accumulate output across all steps

    // Walk price through initialized ticks until the exact input is consumed, cross one tick boundary at a time.
    // Active liquidity remains constant between boundaries.
    while amount_remaining > 0 {
        require!(
            pool_state.liquidity > 0,
            ConcentratedLiquidityError::NoActiveLiquidity
        ); // Can become 0 mid-swap if all positions exit

        // Find the next initialized boundary in the swap direction.
        // A-to-B: find greatest tick <= current (move down); B-to-A: find smallest tick > current (move up)
        let next_crossing = find_next_initialized_tick(
            &tick_arrays,
            pool_state.current_tick,
            pool_state.tick_spacing,
            a_to_b,
        )
        .ok_or(ConcentratedLiquidityError::MissingTickArrayForSwap)?; // None = no initialized tick found (missing arrays)

        let target_sqrt_price_x64 = tick_to_sqrt_price_x64(next_crossing.tick_index);
        // Compute one price movement step toward that boundary.
        let step = compute_swap_step(
            pool_state.sqrt_price_x64,
            target_sqrt_price_x64,
            pool_state.liquidity,
            amount_remaining,
            pool_state.fee_bps,
            a_to_b,
        )?; // Returns whether target was reached or input exhausted mid-range

        amount_remaining = amount_remaining
            .checked_sub(step.amount_in)
            .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
        amount_out_total = amount_out_total
            .checked_add(step.amount_out)
            .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;

        pool_state.sqrt_price_x64 = step.next_sqrt_price_x64; // Update price after this step
        add_fee_growth(pool_state, fee_side, step.fee_amount)?; // Accrue fees to global tracker (distributed to LPs)

        // Crossing changes active liquidity; partial steps only update current tick.
        if step.reached_target_tick {
            // Load the original account (not the copy) for mutable access
            let tick_array_info = &ctx.remaining_accounts[next_crossing.tick_array_list_index]; // Index from find_next_initialized_tick
            let tick_array_loader = AccountLoader::try_from(tick_array_info)?;
            let mut tick_array = tick_array_loader.load_mut()?;
            let mut tick = tick_array.ticks[next_crossing.tick_offset]; // Copy tick out
            let liquidity_net = cross_tick(pool_state.as_ref(), &mut tick, a_to_b)?; // Flip fee_growth_outside, return +L or -L
            tick_array.ticks[next_crossing.tick_offset] = tick; // Write back modified tick
            pool_state.liquidity = apply_liquidity_delta(pool_state.liquidity, liquidity_net)?; // Update active liquidity
                                                                                                // Ranges are [lower, upper). After crossing downward through tick t,
                                                                                                // price is on its lower side, so the active tick is t - 1. After an
                                                                                                // upward crossing, tick t itself is active.
            pool_state.current_tick = if a_to_b {
                next_crossing
                    .tick_index
                    .checked_sub(1)
                    .ok_or(ConcentratedLiquidityError::TickMathOverflow)? // A-to-B crosses down: current = tick - 1
            } else {
                next_crossing.tick_index // B-to-A crosses up: current = tick
            };
        } else {
            pool_state.current_tick = sqrt_price_x64_to_tick(pool_state.sqrt_price_x64);
            // Input exhausted mid-range: derive tick from price
        }
    }

    // Enforce the user's slippage limit before moving tokens.
    require!(
        amount_out_total >= minimum_amount_out,
        ConcentratedLiquidityError::SlippageExceeded
    ); // Revert if output < expected (price moved unfavorably)

    // Pull the exact input amount from the swapper.
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
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_in_accounts,
        ),
        amount_in,
    )?; // User authorizes transfer of input token

    // Prepare pool PDA signer seeds for the output vault transfer.
    let token_a_mint_key = ctx.accounts.token_a_mint.key();
    let token_b_mint_key = ctx.accounts.token_b_mint.key();
    let signer_seeds: &[&[u8]] = &[
        b"pool",
        token_a_mint_key.as_ref(),
        token_b_mint_key.as_ref(),
        &[pool_state.bump],
    ]; // Pool PDA is vault authority

    // Send the computed output amount from the opposite vault.
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
        ), // Pool PDA signs via CPI with seeds
        amount_out_total,
    )?;

    Ok(())
}

fn load_tick_arrays(remaining_accounts: &[AccountInfo<'_>]) -> Result<Vec<TickArray>> {
    // Copy the zero-copy accounts for read-only searching. When a boundary is
    // crossed, the handler re-borrows the corresponding original account
    // mutably using the indices returned by `find_next_initialized_tick`.
    let mut tick_arrays = Vec::with_capacity(remaining_accounts.len());
    for account_info in remaining_accounts {
        let loader = AccountLoader::try_from(account_info)?; // Validate account is TickArray type
        tick_arrays.push(*loader.load()?); // Dereference and copy the entire TickArray struct
    }
    Ok(tick_arrays) // Returns owned copies, not references (safe for iteration without borrowing original accounts)
}
