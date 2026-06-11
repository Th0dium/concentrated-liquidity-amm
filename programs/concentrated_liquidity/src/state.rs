use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Default tick spacing: 100 ticks, roughly 1% price distance between initialized ticks.
pub const DEFAULT_TICK_SPACING: u16 = 100;
/// Number of initializable tick boundaries stored in one `TickArray` account.
pub const TICK_ARRAY_SIZE: usize = 88;
/// Q64.64 representation of `1.0`.
pub const Q64_64_ONE: u128 = 1u128 << 64;
/// Scale used to store cumulative fee growth as token amount per unit of liquidity.
pub const FEE_GROWTH_SCALING_FACTOR: u128 = Q64_64_ONE;

/// One potential liquidity boundary in a `TickArray`.
///
/// A tick does not store liquidity that is currently active at its price.
/// Instead, it stores how the pool's active liquidity must change when price
/// crosses this boundary and the fee-growth checkpoint needed to separate fees
/// earned on the two sides of the boundary.
#[zero_copy]
#[derive(Default, Copy, Clone)]
#[repr(C)]
pub struct Tick {
    /// Byte flag used instead of `bool` so the zero-copy layout remains POD-safe.
    ///
    /// A tick is initialized while at least one position uses it as a boundary.
    pub initialized: u8,
    /// Explicit alignment padding required by the zero-copy account layout.
    pub _padding: [u8; 7],
    /// Signed active-liquidity change when crossing this tick toward higher prices.
    ///
    /// Lower position boundaries contribute `+L`; upper boundaries contribute
    /// `-L`. A downward crossing applies the opposite sign.
    pub liquidity_net: i128,
    /// Sum of the absolute liquidity amounts whose lower or upper boundary is this tick.
    ///
    /// Unlike `liquidity_net`, gross liquidity does not cancel between positions.
    /// It determines whether the tick must remain initialized.
    pub liquidity_gross: u128,
    /// Token A fee growth on the side of this boundary currently considered outside.
    ///
    /// Crossing flips this checkpoint to `global - outside`, changing which side
    /// of the tick the stored value represents.
    pub fee_growth_outside_a_x64: u128,
    /// Token B equivalent of `fee_growth_outside_a_x64`.
    pub fee_growth_outside_b_x64: u128,
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
    /// Discrete tick used to decide which half-open liquidity ranges are active.
    ///
    /// It normally matches the tick derived from `sqrt_price_x64`. At an exact
    /// boundary reached by crossing downward through tick `t`, it is `t - 1`
    /// so `[lower, upper)` membership selects liquidity on the lower side.
    pub current_tick: i32,
    /// Sum of liquidity from positions satisfying `lower <= current_tick < upper`.
    ///
    /// This is neither total deposited liquidity nor either vault's token balance.
    pub liquidity: u128,
    /// Cumulative token A fees per unit of active liquidity, scaled by Q64.64.
    ///
    /// This accumulator is lazy accounting state, not a token amount held in a
    /// separate account.
    pub fee_growth_global_a_x64: u128,
    /// Token B equivalent of `fee_growth_global_a_x64`.
    pub fee_growth_global_b_x64: u128,
    /// Minimum spacing between valid ticks, in raw tick units (e.g., 100 ticks ~= 1%)
    pub tick_spacing: u16,
}

/// Fixed-size, zero-copy account containing consecutive initializable ticks.
///
/// The account covers ticks
/// `start_tick_index + offset * pool.tick_spacing` for offsets `0..88`.
/// Keeping ticks in explicit accounts bounds the state a Solana transaction
/// can inspect and mutate during position updates and swaps.
#[account(zero_copy)]
#[derive(Copy, Clone)]
#[repr(C)]
pub struct TickArray {
    /// Pool whose spacing and price state give these ticks meaning.
    pub pool: Pubkey,
    /// Tick index represented by `ticks[0]`.
    pub start_tick_index: i32,
    /// PDA bump for `[b"tick_array", pool, start_tick_index]`.
    pub bump: u8,
    /// Explicit alignment padding required by the zero-copy account layout.
    pub _padding: [u8; 3],
    /// Consecutive tick boundary records separated by the pool's tick spacing.
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
    /// Owner recorded when the position was created; NFT token ownership is the authoritative source
    pub owner: Pubkey,
    /// Pool this position belongs to
    pub pool: Pubkey,
    /// Lower tick bound (inclusive) where this position provides liquidity
    pub tick_lower: i32,
    /// Upper tick bound (exclusive) where this position provides liquidity
    pub tick_upper: i32,
    /// Amount of liquidity provided (calculated from deposited tokens)
    pub liquidity_amount: u128,
    /// Fee growth inside this range at the position's last accrual for token A.
    ///
    /// New fees are `liquidity * (current_inside - checkpoint) / Q64.64`.
    pub fee_growth_checkpoint_a_x64: u128,
    /// Token B equivalent of `fee_growth_checkpoint_a_x64`.
    pub fee_growth_checkpoint_b_x64: u128,
    /// Token A fees already materialized by lazy accrual but not yet withdrawn.
    pub fees_a_owed: u128,
    /// Token B fees already materialized by lazy accrual but not yet withdrawn.
    pub fees_b_owed: u128,
}
