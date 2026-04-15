use anchor_lang::prelude::*;

use crate::{
    errors::ConcentratedLiquidityError,
    math::{tick_array_start_index, tick_array_span},
    state::TickArray,
    state::PoolState,
};

#[derive(Accounts)]
#[instruction(start_tick_index: i32)]
pub struct InitializeTickArray<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,

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

pub fn handler(ctx: Context<InitializeTickArray>, start_tick_index: i32) -> Result<()> {
    let expected = tick_array_start_index(start_tick_index, ctx.accounts.pool_state.tick_spacing)
        .map_err(|_| ConcentratedLiquidityError::InvalidTickArrayStart)?;
    require!(start_tick_index == expected, ConcentratedLiquidityError::InvalidTickArrayStart);

    let _ = tick_array_span(ctx.accounts.pool_state.tick_spacing)
        .map_err(|_| ConcentratedLiquidityError::TickMathOverflow)?;

    let mut tick_array = ctx.accounts.tick_array.load_init()?;
    tick_array.pool = ctx.accounts.pool_state.key();
    tick_array.start_tick_index = start_tick_index;
    tick_array.bump = ctx.bumps.tick_array;

    Ok(())
}
