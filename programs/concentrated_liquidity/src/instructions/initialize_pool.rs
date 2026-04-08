use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    errors::ConcentratedLiquidityError,
    state::{PoolState, DEFAULT_TICK_SPACING_BPS, Q64_64_ONE},
};

#[derive(Accounts)]
pub struct InitializePool<'info> {
    /// Payer for account creation rent
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Mint account for token A (must be different from token B)
    pub token_a_mint: Account<'info, Mint>,
    
    /// Mint account for token B (must be different from token A)
    pub token_b_mint: Account<'info, Mint>,

    /// Pool state PDA
    /// Seeds: [b"pool", token_a_mint, token_b_mint]
    /// Ensures one pool per token pair
    #[account(
        init,
        payer = payer,
        space = 8 + PoolState::INIT_SPACE,
        seeds = [b"pool", token_a_mint.key().as_ref(), token_b_mint.key().as_ref()],
        bump
    )]
    pub pool_state: Account<'info, PoolState>,

    /// Token vault for token A
    /// Seeds: [b"vault_a", pool_state]
    /// Authority: pool_state PDA (program can sign transfers via CPI)
    #[account(
        init,
        payer = payer,
        token::mint = token_a_mint,
        token::authority = pool_state,
        seeds = [b"vault_a", pool_state.key().as_ref()],
        bump
    )]
    pub token_a_vault: Account<'info, TokenAccount>,

    /// Token vault for token B
    /// Seeds: [b"vault_b", pool_state]
    /// Authority: pool_state PDA (program can sign transfers via CPI)
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
    // Validate token mints are different (can't create USDC/USDC pool)
    require_keys_neq!(
        ctx.accounts.token_a_mint.key(),
        ctx.accounts.token_b_mint.key(),
        ConcentratedLiquidityError::IdenticalMints
    );
    
    // Validate fee is at most 100% (10,000 basis points)
    require!(fee_bps <= 10_000, ConcentratedLiquidityError::InvalidFeeBps);

    // Initialize pool state with provided parameters
    let pool_state = &mut ctx.accounts.pool_state;
    pool_state.bump = ctx.bumps.pool_state;
    pool_state.token_a_mint = ctx.accounts.token_a_mint.key();
    pool_state.token_b_mint = ctx.accounts.token_b_mint.key();
    pool_state.token_a_vault = ctx.accounts.token_a_vault.key();
    pool_state.token_b_vault = ctx.accounts.token_b_vault.key();
    pool_state.fee_bps = fee_bps;
    
    // Start at price 1.0 encoded as Q64.64.
    pool_state.sqrt_price_x64 = Q64_64_ONE;
    pool_state.liquidity = 0;
    
    // Use default tick spacing if not provided
    pool_state.tick_spacing_bps = if tick_spacing_bps == 0 {
        DEFAULT_TICK_SPACING_BPS
    } else {
        tick_spacing_bps
    };

    Ok(())
}
