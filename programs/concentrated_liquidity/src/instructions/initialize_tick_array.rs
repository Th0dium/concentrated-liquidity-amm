use anchor_lang::prelude::*;

use crate::{
    errors::ConcentratedLiquidityError,
    math::{tick_array_span, tick_array_start_index},
    state::PoolState,
    state::TickArray,
};

/// Tick arrays are zero-copy pool-owned accounts that hold fixed-size chunks of
/// tick boundary state. Positions and swaps must pass the arrays containing the
/// ticks they touch, which keeps price traversal deterministic and account
/// bounded on Solana.
#[derive(Accounts)]
#[instruction(start_tick_index: i32)]
pub struct InitializeTickArray<'info> {
    /// Wallet paying rent for the new tick-array account.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Pool that owns this tick array.
    ///
    /// The pool's `tick_spacing` determines the canonical span and alignment for
    /// `start_tick_index`.
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,

    /// Tick-array PDA.
    ///
    /// Seeds: `[b"tick_array", pool_state, start_tick_index]`.
    /// The start index is part of the address so clients can derive which array
    /// covers a requested tick.
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<TickArray>(),
        seeds = [
            b"tick_array",
            pool_state.key().as_ref(),
            &start_tick_index.to_le_bytes(),
        ],
        bump
    )]
    pub tick_array: AccountLoader<'info, TickArray>,

    pub system_program: Program<'info, System>,
}

/// Initialize one tick-array account for a pool.
///
/// The handler verifies that `start_tick_index` is exactly aligned to the
/// pool's tick-array span. This prevents overlapping or shifted arrays from
/// claiming the same region of price space. On success, only array metadata is
/// written; individual tick slots remain zeroed until positions initialize
/// liquidity boundaries.
pub fn handler(ctx: Context<InitializeTickArray>, start_tick_index: i32) -> Result<()> {
    let expected = tick_array_start_index(start_tick_index, ctx.accounts.pool_state.tick_spacing)
        .map_err(|_| ConcentratedLiquidityError::InvalidTickArrayStart)?;
    require!(
        start_tick_index == expected,
        ConcentratedLiquidityError::InvalidTickArrayStart
    );

    let _ = tick_array_span(ctx.accounts.pool_state.tick_spacing)
        .map_err(|_| ConcentratedLiquidityError::TickMathOverflow)?;

    let mut tick_array = ctx.accounts.tick_array.load_init()?;
    tick_array.pool = ctx.accounts.pool_state.key();
    tick_array.start_tick_index = start_tick_index;
    tick_array.bump = ctx.bumps.tick_array;

    Ok(())
}
