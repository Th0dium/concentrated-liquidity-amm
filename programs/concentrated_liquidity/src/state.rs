use anchor_lang::prelude::*;

pub const DEFAULT_TICK_SPACING_BPS: u16 = 100;

#[account]
#[derive(InitSpace)]
pub struct PoolState {
    pub bump: u8,
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub token_a_vault: Pubkey,
    pub token_b_vault: Pubkey,
    pub fee_bps: u16,
    pub current_tick: i32,
    pub total_liquidity: u128,
    pub tick_spacing_bps: u16,
}

#[account]
#[derive(InitSpace)]
pub struct Position {
    pub bump: u8,
    pub owner: Pubkey,
    pub pool: Pubkey,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub liquidity_amount: u128,
    pub fees_a_owed: u128,
    pub fees_b_owed: u128,
}