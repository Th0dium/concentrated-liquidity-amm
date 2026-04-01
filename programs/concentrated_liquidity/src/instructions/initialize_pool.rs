use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    errors::ConcentratedLiquidityError,
    state::{PoolState, DEFAULT_TICK_SPACING_BPS},
};

#[derive(Accounts)]
#[instruction(fee_bps: u16, tick_spacing_bps: u16)]
pub struct InitializePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = payer,
        space = 8 + PoolState::INIT_SPACE,
        seeds = [b"pool", token_a_mint.key().as_ref(), token_b_mint.key().as_ref()],
        bump
    )]
    pub pool_state: Account<'info, PoolState>,

    #[account(
        init,
        payer = payer,
        token::mint = token_a_mint,
        token::authority = pool_state,
        seeds = [b"vault_a", pool_state.key().as_ref()],
        bump
    )]
    pub token_a_vault: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = payer,
        token::mint = token_b_mint,
        token::authority = pool_state,
        seeds = [b"vault_b", pool_state.key().as_ref()],
        bump
    )]
    pub token_b_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitializePool>, fee_bps: u16, tick_spacing_bps: u16) -> Result<()> {
    require_keys_neq!(
        ctx.accounts.token_a_mint.key(),
        ctx.accounts.token_b_mint.key(),
        ConcentratedLiquidityError::IdenticalMints
    );
    require!(fee_bps <= 10_000, ConcentratedLiquidityError::InvalidFeeBps);

    let pool_state = &mut ctx.accounts.pool_state;
    pool_state.bump = ctx.bumps.pool_state;
    pool_state.token_a_mint = ctx.accounts.token_a_mint.key();
    pool_state.token_b_mint = ctx.accounts.token_b_mint.key();
    pool_state.token_a_vault = ctx.accounts.token_a_vault.key();
    pool_state.token_b_vault = ctx.accounts.token_b_vault.key();
    pool_state.fee_bps = fee_bps;
    pool_state.current_tick = 0;
    pool_state.total_liquidity = 0;
    pool_state.tick_spacing_bps = if tick_spacing_bps == 0 {
        DEFAULT_TICK_SPACING_BPS
    } else {
        tick_spacing_bps
    };
    pool_state.next_position_id = 0;

    Ok(())
}