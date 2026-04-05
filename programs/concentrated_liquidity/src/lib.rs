use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;
pub mod math;
pub mod state;

pub use instructions::*;

declare_id!("FkK3SxxHftx9TyVF7Xei362Hi55YQkjuGsE8yKXn4Sxv");

#[program]
pub mod concentrated_liquidity {
    use super::*;

    /// Initialize a new concentrated liquidity pool for a token pair.
    ///
    /// Creates the pool state PDA and two token vault PDAs to hold liquidity.
    /// Each token pair can only have one pool (enforced by PDA seeds).
    ///
    /// # Arguments
    /// * `fee_bps` - Trading fee in basis points (e.g., 30 = 0.30%)
    /// * `tick_spacing_bps` - Minimum tick spacing in basis points (e.g., 100 = 1%)
    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        fee_bps: u16,
        tick_spacing_bps: u16,
    ) -> Result<()> {
        instructions::initialize_pool::handler(ctx, fee_bps, tick_spacing_bps)
    }

    /// Create a new liquidity position within a specific tick range.
    ///
    /// Deposits tokens into the pool and mints a unique NFT-like position token
    /// to represent ownership. The position earns fees when swaps occur within
    /// its tick range.
    ///
    /// # Arguments
    /// * `tick_lower` - Lower bound of the price range (inclusive)
    /// * `tick_upper` - Upper bound of the price range (exclusive)
    /// * `amount_a` - Amount of token A to deposit
    /// * `amount_b` - Amount of token B to deposit
    pub fn create_position(
        ctx: Context<CreatePosition>,
        tick_lower: i32,
        tick_upper: i32,
        amount_a: u64,
        amount_b: u64,
    ) -> Result<()> {
        instructions::create_position::handler(ctx, tick_lower, tick_upper, amount_a, amount_b)
    }

    /// Initialize one tick-array account for a pool.
    pub fn initialize_tick_array(
        ctx: Context<InitializeTickArray>,
        start_tick_index: i32,
    ) -> Result<()> {
        instructions::initialize_tick_array::handler(ctx, start_tick_index)
    }
}
