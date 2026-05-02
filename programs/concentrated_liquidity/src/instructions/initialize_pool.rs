use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    errors::ConcentratedLiquidityError,
    state::{PoolState, DEFAULT_TICK_SPACING, Q64_64_ONE},
};

/// This creates the durable pool state account and the two SPL token vaults for
/// one ordered token pair. The pool state PDA is the authority of both vaults,
/// so future swaps and withdrawals can be signed by the program through PDA
/// signer seeds.
#[derive(Accounts)]
pub struct InitializePool<'info> {
    /// Wallet paying rent for the pool state and both vault token accounts.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// SPL mint for token A.
    ///
    /// The mint address is part of the pool PDA seed, so token order matters:
    /// `(A, B)` and `(B, A)` are different pools.
    pub token_a_mint: Account<'info, Mint>,

    /// SPL mint for token B.
    ///
    /// The handler rejects identical mints because a pool must trade between
    /// two different assets.
    pub token_b_mint: Account<'info, Mint>,

    /// Pool state PDA.
    ///
    /// Seeds: `[b"pool", token_a_mint, token_b_mint]`.
    /// Stores pair metadata, vault addresses, current price, active liquidity,
    /// fee-growth accumulators, fee tier, and tick spacing.
    #[account(
        init,
        payer = payer,
        space = 8 + PoolState::INIT_SPACE,
        seeds = [b"pool", token_a_mint.key().as_ref(), token_b_mint.key().as_ref()],
        bump
    )]
    pub pool_state: Account<'info, PoolState>,

    /// Pool-owned token A vault.
    ///
    /// Seeds: `[b"vault_a", pool_state]`.
    /// Liquidity deposits and token-A swap inputs accumulate here. The pool PDA
    /// is the SPL token account authority.
    #[account(
        init,
        payer = payer,
        token::mint = token_a_mint,
        token::authority = pool_state,
        seeds = [b"vault_a", pool_state.key().as_ref()],
        bump
    )]
    pub token_a_vault: Account<'info, TokenAccount>,

    /// Pool-owned token B vault.
    ///
    /// Seeds: `[b"vault_b", pool_state]`.
    /// Liquidity deposits and token-B swap inputs accumulate here. The pool PDA
    /// is the SPL token account authority.
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

/// Initialize a concentrated-liquidity pool for a token pair.
///
/// This writes only pool-level bootstrap state. No positions, ticks, liquidity,
/// or fees exist yet. The pool starts at sqrt price `1.0` (`Q64_64_ONE`),
/// `current_tick = 0`, zero active liquidity, and zero global fee growth.
///
/// `fee_bps` is stored directly after validating it is not above 100%.
/// `tick_spacing = 0` means the program uses `DEFAULT_TICK_SPACING`; otherwise
/// the provided raw tick spacing controls which ticks are valid boundaries.
pub fn handler(ctx: Context<InitializePool>, fee_bps: u16, tick_spacing: u16) -> Result<()> {
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
    pool_state.current_tick = 0;
    pool_state.liquidity = 0;
    pool_state.fee_growth_global_a_x64 = 0;
    pool_state.fee_growth_global_b_x64 = 0;

    // Use default tick spacing if not provided
    pool_state.tick_spacing = if tick_spacing == 0 {
        DEFAULT_TICK_SPACING
    } else {
        tick_spacing
    };

    Ok(())
}
