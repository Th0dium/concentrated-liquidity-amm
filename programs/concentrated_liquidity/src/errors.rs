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
    #[msg("Not enough active liquidity to complete the swap.")]
    NoActiveLiquidity,
    #[msg("Not enough initialized ticks were supplied to finish the swap path.")]
    MissingTickArrayForSwap,
    #[msg("Swap output is below the requested minimum amount.")]
    SlippageExceeded,
    #[msg("Math conversion lost precision beyond supported bounds.")]
    MathConversionError,
    #[msg("Position token account does not prove current ownership of the position NFT.")]
    InvalidPositionTokenAccount,
    #[msg("Position does not belong to the supplied pool.")]
    PositionPoolMismatch,
    #[msg("Liquidity amount must be greater than zero.")]
    ZeroLiquidity,
    #[msg("Tick liquidity would become negative.")]
    NegativeLiquidity,
    #[msg("Instruction amount must be greater than zero.")]
    ZeroAmountSpecified,
}
