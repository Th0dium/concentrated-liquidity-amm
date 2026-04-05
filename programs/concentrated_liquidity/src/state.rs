use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Default tick spacing: 100 basis points = 1% price movement per tick
pub const DEFAULT_TICK_SPACING_BPS: u16 = 100;
pub const TICK_ARRAY_SIZE: usize = 88;
pub const Q64_64_ONE: u128 = 1u128 << 64;

#[zero_copy]
#[derive(Default)]
#[repr(C)]
pub struct Tick {
    pub initialized: u8,
    pub _padding: [u8; 15],
    pub liquidity_net: i128,
    pub liquidity_gross: u128,
}

/// Pool state account storing metadata and aggregate liquidity for a token pair.
///
/// PDA seeds: [b"pool", token_a_mint, token_b_mint]
/// This ensures one pool per token pair.
#[account]
#[derive(InitSpace)]
pub struct PoolState {
    /// PDA bump seed for signing CPIs
    pub bump: u8,
    /// Mint address of token A
    pub token_a_mint: Pubkey,
    /// Mint address of token B
    pub token_b_mint: Pubkey,
    /// Token account holding pool's token A reserves
    pub token_a_vault: Pubkey,
    /// Token account holding pool's token B reserves
    pub token_b_vault: Pubkey,
    /// Trading fee in basis points (e.g., 30 = 0.30%)
    pub fee_bps: u16,
    /// Current sqrt price in Q64.64 fixed-point format
    pub sqrt_price_x64: u128,
    /// Active liquidity at the current pool price
    pub liquidity: u128,
    /// Minimum tick spacing in basis points (e.g., 100 = 1%)
    pub tick_spacing_bps: u16,
}

#[account(zero_copy)]
#[repr(C)]
pub struct TickArray {
    pub pool: Pubkey,
    pub start_tick_index: i32,
    pub bump: u8,
    pub _padding: [u8; 3],
    pub ticks: [Tick; TICK_ARRAY_SIZE],
}

/// Position account representing an LP's liquidity deposit in a specific tick range.
///
/// PDA seeds: [b"position", position_mint]
/// Each position has a unique NFT-like mint for transferable ownership.
#[account]
#[derive(InitSpace)]
pub struct Position {
    /// PDA bump seed
    pub bump: u8,
    /// Unique mint address for this position (NFT-like, decimals=0, supply=1)
    pub position_mint: Pubkey,
    /// Current owner of the position (can change if position token is transferred)
    pub owner: Pubkey,
    /// Pool this position belongs to
    pub pool: Pubkey,
    /// Lower tick bound (inclusive) where this position provides liquidity
    pub tick_lower: i32,
    /// Upper tick bound (exclusive) where this position provides liquidity
    pub tick_upper: i32,
    /// Amount of liquidity provided (calculated from deposited tokens)
    pub liquidity_amount: u128,
    /// Accumulated fees in token A (claimable by owner)
    pub fees_a_owed: u128,
    /// Accumulated fees in token B (claimable by owner)
    pub fees_b_owed: u128,
}
