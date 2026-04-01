use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;
pub mod state;

pub use instructions::*;

declare_id!("FkK3SxxHftx9TyVF7Xei362Hi55YQkjuGsE8yKXn4Sxv");

#[program]
pub mod concentrated_liquidity {
    use super::*;

    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        fee_bps: u16,
        tick_spacing_bps: u16,
    ) -> Result<()> {
        instructions::initialize_pool::handler(ctx, fee_bps, tick_spacing_bps)
    }

    pub fn create_position(
        ctx: Context<CreatePosition>,
        position_id: u64,
        tick_lower: i32,
        tick_upper: i32,
        amount_a: u64,
        amount_b: u64,
    ) -> Result<()> {
        instructions::create_position::handler(
            ctx,
            position_id,
            tick_lower,
            tick_upper,
            amount_a,
            amount_b,
        )
    }
}