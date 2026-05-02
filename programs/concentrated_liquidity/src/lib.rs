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

    /// Creates a new concentrated-liquidity pool for an ordered token pair.
    ///
    /// The pool is a PDA derived from the token A and token B mint addresses, so
    /// the same ordered pair can only be initialized once. The instruction also
    /// creates two pool-owned SPL token vaults. The pool PDA is the authority of
    /// those vaults, which lets later instructions move tokens out by signing
    /// with deterministic PDA seeds instead of a private key.
    ///
    /// Initial CLMM state starts at sqrt price `1.0` in Q64.64 form, current
    /// tick `0`, zero active liquidity, and zero global fee growth. Passing
    /// `tick_spacing = 0` chooses the program default spacing.
    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        fee_bps: u16,
        tick_spacing: u16,
    ) -> Result<()> {
        instructions::initialize_pool::handler(ctx, fee_bps, tick_spacing)
    }

    /// Opens one LP position over `[tick_lower, tick_upper)`.
    ///
    /// The caller supplies maximum token A and token B budgets. The program
    /// quotes liquidity at the current pool price, transfers only the consumed
    /// token amounts into the pool vaults, mints one NFT-like position token,
    /// stores fee checkpoints in the position PDA, and updates both tick
    /// boundary liquidity records.
    pub fn create_position(
        ctx: Context<CreatePosition>,
        tick_lower: i32,
        tick_upper: i32,
        amount_a: u64,
        amount_b: u64,
    ) -> Result<()> {
        instructions::create_position::handler(ctx, tick_lower, tick_upper, amount_a, amount_b)
    }

    /// Initializes one fixed-size tick-array account for a pool.
    ///
    /// Tick arrays partition price space into deterministic account-sized
    /// chunks. Positions and swaps must pass the arrays containing the ticks
    /// they touch, keeping Solana account access explicit and bounded.
    pub fn initialize_tick_array(
        ctx: Context<InitializeTickArray>,
        start_tick_index: i32,
    ) -> Result<()> {
        instructions::initialize_tick_array::handler(ctx, start_tick_index)
    }

    /// Executes an exact-input swap through active concentrated liquidity.
    ///
    /// `a_to_b = true` means token A in and token B out; `false` means token B
    /// in and token A out. The swap traverses initialized ticks from
    /// `remaining_accounts`, updates price, current tick, active liquidity, and
    /// fee growth, then transfers input from the swapper and output from the
    /// pool vault after the slippage check passes.
    pub fn swap(
        ctx: Context<Swap>,
        amount_in: u64,
        minimum_amount_out: u64,
        a_to_b: bool,
    ) -> Result<()> {
        instructions::swap::handler(ctx, amount_in, minimum_amount_out, a_to_b)
    }

    /// Closes an LP position and withdraws liquidity plus accrued fees.
    ///
    /// The signer must hold exactly one token from the position mint. The
    /// instruction accrues fees from fee-growth checkpoints, removes liquidity
    /// from the lower and upper ticks, updates pool active liquidity if the
    /// range contains the current price, transfers owed tokens from the vaults,
    /// burns the position token, and closes the position accounts.
    pub fn close_position(ctx: Context<ClosePosition>) -> Result<()> {
        instructions::close_position::handler(ctx)
    }
}
