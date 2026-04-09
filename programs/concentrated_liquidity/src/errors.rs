use anchor_lang::prelude::*;

#[error_code]
pub enum ConcentratedLiquidityError {
    #[msg("Token mints must be different.")]
    IdenticalMints,
    #[msg("Fee basis points must be at most 10_000.")]
    InvalidFeeBps,
    #[msg("Tick lower must be strictly less than tick upper.")]
    InvalidTickRange,
    #[msg("At least one deposit amount must be greater than zero.")]
    ZeroLiquidityDeposit,
    #[msg("Deposit token amounts do not match the current price relative to the position range.")]
    InvalidPositionTokenAmounts,
    #[msg("Tick index must align to pool tick spacing.")]
    TickNotAligned,
    #[msg("Tick array start index is invalid for this pool spacing.")]
    InvalidTickArrayStart,
    #[msg("Tick index is out of bounds for the provided tick array.")]
    TickIndexOutOfBounds,
    #[msg("Tick math overflow.")]
    TickMathOverflow,
}
