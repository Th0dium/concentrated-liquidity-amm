use anchor_lang::prelude::*;

use crate::{
    errors::ConcentratedLiquidityError,
    state::{FEE_GROWTH_SCALING_FACTOR, PoolState, Position, Tick, TickArray, Q64_64_ONE, TICK_ARRAY_SIZE},
};

const TICK_BASE: f64 = 1.0001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeeSide {
    TokenA,
    TokenB,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FeeGrowthInside {
    pub token_a_x64: u128,
    pub token_b_x64: u128,
}

#[derive(Clone, Copy, Debug)]
pub struct LiquidityQuote {
    pub liquidity_delta: u128,
    pub amount_a: u64,
    pub amount_b: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct NextTickCrossing {
    pub tick_index: i32,
    pub tick_array_list_index: usize,
    pub tick_offset: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct SwapStep {
    pub next_sqrt_price_x64: u128,
    pub amount_in: u64,
    pub amount_out: u64,
    pub fee_amount: u64,
    pub reached_target_tick: bool,
}

/// Calculates the total tick range covered by one tick array (TICK_ARRAY_SIZE * tick_spacing).
pub fn tick_array_span(tick_spacing: u16) -> Result<i32> {
    let spacing = i32::from(tick_spacing);
    let array_span = spacing
        .checked_mul(TICK_ARRAY_SIZE as i32)
        .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
    Ok(array_span)
}

/// Returns the start tick index of the tick array that contains the given tick.
pub fn tick_array_start_index(tick_index: i32, tick_spacing: u16) -> Result<i32> {
    let array_span = tick_array_span(tick_spacing)?;
    let quotient = tick_index.div_euclid(array_span);
    quotient
        .checked_mul(array_span)
        .ok_or(ConcentratedLiquidityError::TickMathOverflow.into())
}

/// Finds the array index offset of a tick within its tick array.
pub fn tick_offset_in_array(
    start_tick_index: i32,
    tick_index: i32,
    tick_spacing: u16,
) -> Result<usize> {
    let spacing = i32::from(tick_spacing);
    let delta = tick_index
        .checked_sub(start_tick_index)
        .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;

    if delta < 0 || delta % spacing != 0 {
        return Err(ConcentratedLiquidityError::InvalidTickArrayStart.into());
    }

    let offset = delta / spacing;
    if offset < 0 || offset >= TICK_ARRAY_SIZE as i32 {
        return Err(ConcentratedLiquidityError::TickIndexOutOfBounds.into());
    }

    Ok(offset as usize)
}

/// Validates that a tick index is aligned with the pool's tick spacing.
pub fn validate_tick_alignment(tick_index: i32, tick_spacing: u16) -> Result<()> {
    let spacing = i32::from(tick_spacing);
    if spacing <= 0 || tick_index % spacing != 0 {
        return Err(ConcentratedLiquidityError::TickNotAligned.into());
    }
    Ok(())
}

/// Validates token deposit amounts match position range relative to current price (below/in/above range).
pub fn validate_position_token_amounts(
    current_tick: i32,
    tick_lower: i32,
    tick_upper: i32,
    amount_a: u64,
    amount_b: u64,
) -> Result<()> {
    if amount_a == 0 && amount_b == 0 {
        return Err(ConcentratedLiquidityError::ZeroLiquidityDeposit.into());
    }

    if tick_lower > current_tick {
        if amount_a == 0 || amount_b > 0 {
            return Err(ConcentratedLiquidityError::InvalidPositionTokenAmounts.into());
        }
    } else if tick_upper <= current_tick {
        if amount_a > 0 || amount_b == 0 {
            return Err(ConcentratedLiquidityError::InvalidPositionTokenAmounts.into());
        }
    } else if amount_a == 0 || amount_b == 0 {
        return Err(ConcentratedLiquidityError::InvalidPositionTokenAmounts.into());
    }

    Ok(())
}

pub fn sqrt_price_x64_to_f64(sqrt_price_x64: u128) -> f64 {
    sqrt_price_x64 as f64 / Q64_64_ONE as f64
}

pub fn sqrt_price_f64_to_x64(sqrt_price: f64) -> Result<u128> {
    if !sqrt_price.is_finite() || sqrt_price <= 0.0 {
        return Err(ConcentratedLiquidityError::MathConversionError.into());
    }

    let scaled = sqrt_price * Q64_64_ONE as f64;
    if !scaled.is_finite() || scaled <= 0.0 || scaled > u128::MAX as f64 {
        return Err(ConcentratedLiquidityError::MathConversionError.into());
    }

    Ok(scaled.round() as u128)
}

/// Converts Q64.64 sqrt price to tick index.
pub fn sqrt_price_x64_to_tick(sqrt_price_x64: u128) -> i32 {
    let sqrt_price = sqrt_price_x64_to_f64(sqrt_price_x64);
    let tick = ((sqrt_price.ln() * 2.0) / TICK_BASE.ln()).floor();
    tick.clamp(i32::MIN as f64, i32::MAX as f64) as i32
}

/// Converts tick index to Q64.64 sqrt price.
pub fn tick_to_sqrt_price_x64(tick: i32) -> u128 {
    let sqrt_price = TICK_BASE.powf(tick as f64 / 2.0);
    sqrt_price_f64_to_x64(sqrt_price).unwrap_or(Q64_64_ONE)
}

pub fn tick_to_sqrt_price_f64(tick: i32) -> f64 {
    sqrt_price_x64_to_f64(tick_to_sqrt_price_x64(tick))
}

fn amount_a_delta_unsigned(liquidity: u128, sqrt_a: f64, sqrt_b: f64, round_up: bool) -> Result<u64> {
    let lower = sqrt_a.min(sqrt_b);
    let upper = sqrt_a.max(sqrt_b);
    let delta = (liquidity as f64) * (upper - lower) / (upper * lower);
    f64_to_token_amount(delta, round_up)
}

fn amount_b_delta_unsigned(liquidity: u128, sqrt_a: f64, sqrt_b: f64, round_up: bool) -> Result<u64> {
    let lower = sqrt_a.min(sqrt_b);
    let upper = sqrt_a.max(sqrt_b);
    let delta = (liquidity as f64) * (upper - lower);
    f64_to_token_amount(delta, round_up)
}

fn f64_to_token_amount(value: f64, round_up: bool) -> Result<u64> {
    if !value.is_finite() || value < 0.0 {
        return Err(ConcentratedLiquidityError::MathConversionError.into());
    }

    let rounded = if round_up { value.ceil() } else { value.floor() };
    if rounded > u64::MAX as f64 {
        return Err(ConcentratedLiquidityError::TickMathOverflow.into());
    }

    Ok(rounded as u64)
}

fn liquidity_from_amount_a(amount_a: u64, sqrt_lower: f64, sqrt_upper: f64) -> Result<u128> {
    let numerator = (amount_a as f64) * sqrt_lower * sqrt_upper;
    let denominator = sqrt_upper - sqrt_lower;
    f64_to_liquidity(numerator / denominator)
}

fn liquidity_from_amount_b(amount_b: u64, sqrt_lower: f64, sqrt_upper: f64) -> Result<u128> {
    let denominator = sqrt_upper - sqrt_lower;
    f64_to_liquidity((amount_b as f64) / denominator)
}

fn f64_to_liquidity(value: f64) -> Result<u128> {
    if !value.is_finite() || value <= 0.0 {
        return Err(ConcentratedLiquidityError::ZeroLiquidity.into());
    }
    if value > u128::MAX as f64 {
        return Err(ConcentratedLiquidityError::TickMathOverflow.into());
    }
    Ok(value.floor() as u128)
}

pub fn liquidity_quote(
    amount_a_max: u64,
    amount_b_max: u64,
    tick_lower: i32,
    tick_upper: i32,
    sqrt_price_x64: u128,
) -> Result<LiquidityQuote> {
    let sqrt_lower = tick_to_sqrt_price_f64(tick_lower);
    let sqrt_upper = tick_to_sqrt_price_f64(tick_upper);
    let sqrt_current = sqrt_price_x64_to_f64(sqrt_price_x64);

    let liquidity_delta = if sqrt_current <= sqrt_lower {
        liquidity_from_amount_a(amount_a_max, sqrt_lower, sqrt_upper)?
    } else if sqrt_current >= sqrt_upper {
        liquidity_from_amount_b(amount_b_max, sqrt_lower, sqrt_upper)?
    } else {
        let liquidity_from_a = liquidity_from_amount_a(amount_a_max, sqrt_current, sqrt_upper)?;
        let liquidity_from_b = liquidity_from_amount_b(amount_b_max, sqrt_lower, sqrt_current)?;
        liquidity_from_a.min(liquidity_from_b)
    };

    if liquidity_delta == 0 {
        return Err(ConcentratedLiquidityError::ZeroLiquidity.into());
    }

    let (amount_a, amount_b) =
        amounts_for_liquidity(liquidity_delta, tick_lower, tick_upper, sqrt_price_x64)?;

    Ok(LiquidityQuote {
        liquidity_delta,
        amount_a,
        amount_b,
    })
}

pub fn amounts_for_liquidity(
    liquidity: u128,
    tick_lower: i32,
    tick_upper: i32,
    sqrt_price_x64: u128,
) -> Result<(u64, u64)> {
    let sqrt_lower = tick_to_sqrt_price_f64(tick_lower);
    let sqrt_upper = tick_to_sqrt_price_f64(tick_upper);
    let sqrt_current = sqrt_price_x64_to_f64(sqrt_price_x64);

    if sqrt_current <= sqrt_lower {
        Ok((amount_a_delta_unsigned(liquidity, sqrt_lower, sqrt_upper, true)?, 0))
    } else if sqrt_current >= sqrt_upper {
        Ok((0, amount_b_delta_unsigned(liquidity, sqrt_lower, sqrt_upper, true)?))
    } else {
        Ok((
            amount_a_delta_unsigned(liquidity, sqrt_current, sqrt_upper, true)?,
            amount_b_delta_unsigned(liquidity, sqrt_lower, sqrt_current, true)?,
        ))
    }
}

pub fn liquidity_from_amounts(
    amount_a: u64,
    amount_b: u64,
    tick_lower: i32,
    tick_upper: i32,
    sqrt_price_x64: u128,
) -> Result<u128> {
    Ok(liquidity_quote(amount_a, amount_b, tick_lower, tick_upper, sqrt_price_x64)?.liquidity_delta)
}

pub fn fee_growth_inside_for_ticks(
    pool: &PoolState,
    tick_lower_index: i32,
    lower_tick: &Tick,
    tick_upper_index: i32,
    upper_tick: &Tick,
) -> FeeGrowthInside {
    let below_a;
    let below_b;
    if pool.current_tick < tick_lower_index {
        below_a = pool
            .fee_growth_global_a_x64
            .wrapping_sub(lower_tick.fee_growth_outside_a_x64);
        below_b = pool
            .fee_growth_global_b_x64
            .wrapping_sub(lower_tick.fee_growth_outside_b_x64);
    } else {
        below_a = lower_tick.fee_growth_outside_a_x64;
        below_b = lower_tick.fee_growth_outside_b_x64;
    }

    let above_a;
    let above_b;
    if pool.current_tick >= tick_upper_index {
        above_a = pool
            .fee_growth_global_a_x64
            .wrapping_sub(upper_tick.fee_growth_outside_a_x64);
        above_b = pool
            .fee_growth_global_b_x64
            .wrapping_sub(upper_tick.fee_growth_outside_b_x64);
    } else {
        above_a = upper_tick.fee_growth_outside_a_x64;
        above_b = upper_tick.fee_growth_outside_b_x64;
    }

    FeeGrowthInside {
        token_a_x64: pool
            .fee_growth_global_a_x64
            .wrapping_sub(below_a)
            .wrapping_sub(above_a),
        token_b_x64: pool
            .fee_growth_global_b_x64
            .wrapping_sub(below_b)
            .wrapping_sub(above_b),
    }
}

pub fn accrue_position_fees(position: &mut Position, fee_growth_inside: FeeGrowthInside) -> Result<()> {
    let growth_delta_a = fee_growth_inside
        .token_a_x64
        .wrapping_sub(position.fee_growth_checkpoint_a_x64);
    let growth_delta_b = fee_growth_inside
        .token_b_x64
        .wrapping_sub(position.fee_growth_checkpoint_b_x64);

    let additional_a = position
        .liquidity_amount
        .checked_mul(growth_delta_a)
        .ok_or(ConcentratedLiquidityError::TickMathOverflow)?
        / FEE_GROWTH_SCALING_FACTOR;
    let additional_b = position
        .liquidity_amount
        .checked_mul(growth_delta_b)
        .ok_or(ConcentratedLiquidityError::TickMathOverflow)?
        / FEE_GROWTH_SCALING_FACTOR;

    position.fees_a_owed = position
        .fees_a_owed
        .checked_add(additional_a)
        .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
    position.fees_b_owed = position
        .fees_b_owed
        .checked_add(additional_b)
        .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
    position.fee_growth_checkpoint_a_x64 = fee_growth_inside.token_a_x64;
    position.fee_growth_checkpoint_b_x64 = fee_growth_inside.token_b_x64;

    Ok(())
}

pub fn initialize_tick_fee_growths(
    tick: &mut Tick,
    tick_index: i32,
    current_tick: i32,
    pool_fee_growth_a_x64: u128,
    pool_fee_growth_b_x64: u128,
) {
    if tick.initialized == 0 {
        tick.initialized = 1;
        if tick_index <= current_tick {
            tick.fee_growth_outside_a_x64 = pool_fee_growth_a_x64;
            tick.fee_growth_outside_b_x64 = pool_fee_growth_b_x64;
        }
    }
}

pub fn update_tick_liquidity(
    tick: &mut Tick,
    liquidity_delta: i128,
    is_upper_tick: bool,
) -> Result<()> {
    if liquidity_delta == 0 {
        return Err(ConcentratedLiquidityError::ZeroLiquidity.into());
    }

    if liquidity_delta > 0 {
        tick.liquidity_gross = tick
            .liquidity_gross
            .checked_add(liquidity_delta as u128)
            .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
    } else {
        let remove_amount = liquidity_delta.unsigned_abs();
        tick.liquidity_gross = tick
            .liquidity_gross
            .checked_sub(remove_amount)
            .ok_or(ConcentratedLiquidityError::NegativeLiquidity)?;
    }

    let signed_net_delta = if is_upper_tick {
        liquidity_delta
            .checked_neg()
            .ok_or(ConcentratedLiquidityError::TickMathOverflow)?
    } else {
        liquidity_delta
    };

    tick.liquidity_net = tick
        .liquidity_net
        .checked_add(signed_net_delta)
        .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;

    if tick.liquidity_gross == 0 {
        tick.initialized = 0;
        tick.liquidity_net = 0;
        tick.fee_growth_outside_a_x64 = 0;
        tick.fee_growth_outside_b_x64 = 0;
    }

    Ok(())
}

pub fn add_fee_growth(pool: &mut PoolState, fee_side: FeeSide, fee_amount: u64) -> Result<()> {
    if fee_amount == 0 || pool.liquidity == 0 {
        return Ok(());
    }

    let growth_delta = (u128::from(fee_amount))
        .checked_mul(FEE_GROWTH_SCALING_FACTOR)
        .ok_or(ConcentratedLiquidityError::TickMathOverflow)?
        / pool.liquidity;

    match fee_side {
        FeeSide::TokenA => {
            pool.fee_growth_global_a_x64 = pool
                .fee_growth_global_a_x64
                .checked_add(growth_delta)
                .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
        }
        FeeSide::TokenB => {
            pool.fee_growth_global_b_x64 = pool
                .fee_growth_global_b_x64
                .checked_add(growth_delta)
                .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
        }
    }

    Ok(())
}

pub fn cross_tick(pool: &PoolState, tick: &mut Tick, a_to_b: bool) -> Result<i128> {
    tick.fee_growth_outside_a_x64 = pool
        .fee_growth_global_a_x64
        .wrapping_sub(tick.fee_growth_outside_a_x64);
    tick.fee_growth_outside_b_x64 = pool
        .fee_growth_global_b_x64
        .wrapping_sub(tick.fee_growth_outside_b_x64);

    if a_to_b {
        tick.liquidity_net
            .checked_neg()
            .ok_or(ConcentratedLiquidityError::TickMathOverflow.into())
    } else {
        Ok(tick.liquidity_net)
    }
}

pub fn apply_liquidity_delta(current_liquidity: u128, liquidity_delta: i128) -> Result<u128> {
    if liquidity_delta >= 0 {
        current_liquidity
            .checked_add(liquidity_delta as u128)
            .ok_or(ConcentratedLiquidityError::TickMathOverflow.into())
    } else {
        current_liquidity
            .checked_sub(liquidity_delta.unsigned_abs())
            .ok_or(ConcentratedLiquidityError::NegativeLiquidity.into())
    }
}

pub fn compute_swap_step(
    sqrt_price_current_x64: u128,
    sqrt_price_target_x64: u128,
    liquidity: u128,
    amount_remaining: u64,
    fee_bps: u16,
    a_to_b: bool,
) -> Result<SwapStep> {
    if liquidity == 0 {
        return Err(ConcentratedLiquidityError::NoActiveLiquidity.into());
    }

    let sqrt_current = sqrt_price_x64_to_f64(sqrt_price_current_x64);
    let sqrt_target = sqrt_price_x64_to_f64(sqrt_price_target_x64);

    let amount_in_to_target = if a_to_b {
        amount_a_delta_unsigned(liquidity, sqrt_target, sqrt_current, true)?
    } else {
        amount_b_delta_unsigned(liquidity, sqrt_current, sqrt_target, true)?
    };

    let fee_denominator = 10_000u64
        .checked_sub(u64::from(fee_bps))
        .ok_or(ConcentratedLiquidityError::InvalidFeeBps)?;
    let amount_remaining_less_fee = (u128::from(amount_remaining) * u128::from(fee_denominator) / 10_000u128)
        as u64;

    let (next_sqrt_price_x64, amount_in, amount_out, fee_amount, reached_target_tick) =
        if amount_remaining_less_fee >= amount_in_to_target {
            let gross_in = if fee_denominator == 0 {
                amount_remaining
            } else {
                let numerator = u128::from(amount_in_to_target) * 10_000u128;
                let gross = numerator
                    .checked_add(u128::from(fee_denominator) - 1)
                    .ok_or(ConcentratedLiquidityError::TickMathOverflow)?
                    / u128::from(fee_denominator);
                if gross > u128::from(amount_remaining) {
                    amount_remaining
                } else {
                    gross as u64
                }
            };
            let fee_amount = gross_in
                .checked_sub(amount_in_to_target)
                .ok_or(ConcentratedLiquidityError::TickMathOverflow)?;
            let amount_out = if a_to_b {
                amount_b_delta_unsigned(liquidity, sqrt_target, sqrt_current, false)?
            } else {
                amount_a_delta_unsigned(liquidity, sqrt_current, sqrt_target, false)?
            };
            (
                sqrt_price_target_x64,
                gross_in,
                amount_out,
                fee_amount,
                true,
            )
        } else {
            let net_input = amount_remaining_less_fee;
            let next_sqrt = if a_to_b {
                let numerator = (liquidity as f64) * sqrt_current;
                let denominator = (liquidity as f64) + (net_input as f64) * sqrt_current;
                numerator / denominator
            } else {
                sqrt_current + (net_input as f64 / liquidity as f64)
            };
            let next_sqrt_x64 = sqrt_price_f64_to_x64(next_sqrt)?;
            let amount_out = if a_to_b {
                amount_b_delta_unsigned(liquidity, next_sqrt, sqrt_current, false)?
            } else {
                amount_a_delta_unsigned(liquidity, sqrt_current, next_sqrt, false)?
            };
            (
                next_sqrt_x64,
                amount_remaining,
                amount_out,
                amount_remaining
                    .checked_sub(net_input)
                    .ok_or(ConcentratedLiquidityError::TickMathOverflow)?,
                false,
            )
        };

    Ok(SwapStep {
        next_sqrt_price_x64,
        amount_in,
        amount_out,
        fee_amount,
        reached_target_tick,
    })
}

pub fn find_next_initialized_tick(
    tick_arrays: &[TickArray],
    current_tick: i32,
    tick_spacing: u16,
    a_to_b: bool,
) -> Option<NextTickCrossing> {
    let mut best: Option<NextTickCrossing> = None;

    for (array_list_index, tick_array) in tick_arrays.iter().enumerate() {
        for (offset, tick) in tick_array.ticks.iter().enumerate() {
            if tick.initialized == 0 {
                continue;
            }

            let tick_index = tick_array.start_tick_index + (offset as i32 * i32::from(tick_spacing));
            let is_candidate = if a_to_b {
                tick_index <= current_tick
            } else {
                tick_index > current_tick
            };

            if !is_candidate {
                continue;
            }

            match best {
                None => {
                    best = Some(NextTickCrossing {
                        tick_index,
                        tick_array_list_index: array_list_index,
                        tick_offset: offset,
                    });
                }
                Some(existing) => {
                    let replace = if a_to_b {
                        tick_index > existing.tick_index
                    } else {
                        tick_index < existing.tick_index
                    };

                    if replace {
                        best = Some(NextTickCrossing {
                            tick_index,
                            tick_array_list_index: array_list_index,
                            tick_offset: offset,
                        });
                    }
                }
            }
        }
    }

    best
}
