use anchor_lang::prelude::*;

#[error_code]
pub enum ConcentratedLiquidityError {
    #[msg("Token mints must be different.")]
    IdenticalMints,
    #[msg("Fee basis points must be at most 10_000.")]
    InvalidFeeBps,
}