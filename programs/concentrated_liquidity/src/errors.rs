use anchor_lang::prelude::*;

#[error_code]
pub enum ConcentratedLiquidityError {
    #[msg("Token mints must be different.")]
    IdenticalMints,
    #[msg("Fee basis points must be at most 10_000.")]
    InvalidFeeBps,
    #[msg("Tick lower must be strictly less than tick upper.")]
    InvalidTickRange,
    #[msg("Deposit amounts must both be greater than zero.")]
    ZeroLiquidityDeposit,
    #[msg("The requested position id does not match the next expected id for this pool.")]
    InvalidPositionId,
}