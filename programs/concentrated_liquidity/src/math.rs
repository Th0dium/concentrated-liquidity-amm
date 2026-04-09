use crate::state::TICK_ARRAY_SIZE;

pub fn tick_array_span(tick_spacing_bps: u16) -> Result<i32, crate::errors::ConcentratedLiquidityError> {
    let spacing = i32::from(tick_spacing_bps);
    let array_span = spacing
        .checked_mul(TICK_ARRAY_SIZE as i32)
        .ok_or(crate::errors::ConcentratedLiquidityError::TickMathOverflow)?;
    Ok(array_span)
}

pub fn tick_array_start_index(
    tick_index: i32,
    tick_spacing_bps: u16,
) -> Result<i32, crate::errors::ConcentratedLiquidityError> {
    let array_span = tick_array_span(tick_spacing_bps)?;
    let quotient = tick_index.div_euclid(array_span);
    quotient
        .checked_mul(array_span)
        .ok_or(crate::errors::ConcentratedLiquidityError::TickMathOverflow)
}

pub fn tick_offset_in_array(
    start_tick_index: i32,
    tick_index: i32,
    tick_spacing_bps: u16,
) -> Result<usize, crate::errors::ConcentratedLiquidityError> {
    let spacing = i32::from(tick_spacing_bps);
    let delta = tick_index
        .checked_sub(start_tick_index)
        .ok_or(crate::errors::ConcentratedLiquidityError::TickMathOverflow)?;

    if delta < 0 || delta % spacing != 0 {
        return Err(crate::errors::ConcentratedLiquidityError::InvalidTickArrayStart);
    }

    let offset = delta / spacing;
    if offset < 0 || offset >= TICK_ARRAY_SIZE as i32 {
        return Err(crate::errors::ConcentratedLiquidityError::TickIndexOutOfBounds);
    }

    Ok(offset as usize)
}

pub fn validate_tick_alignment(
    tick_index: i32,
    tick_spacing_bps: u16,
) -> Result<(), crate::errors::ConcentratedLiquidityError> {
    let spacing = i32::from(tick_spacing_bps);
    if tick_index % spacing != 0 {
        return Err(crate::errors::ConcentratedLiquidityError::TickNotAligned);
    }
    Ok(())
}

pub fn validate_position_token_amounts(
    current_tick: i32,
    tick_lower: i32,
    tick_upper: i32,
    amount_a: u64,
    amount_b: u64,
) -> Result<(), crate::errors::ConcentratedLiquidityError> {
    if amount_a == 0 && amount_b == 0 {
        return Err(crate::errors::ConcentratedLiquidityError::ZeroLiquidityDeposit);
    }

    if current_tick < tick_lower {
        if amount_a == 0 || amount_b != 0 {
            return Err(crate::errors::ConcentratedLiquidityError::InvalidPositionTokenAmounts);
        }
    } else if current_tick >= tick_upper {
        if amount_b == 0 || amount_a != 0 {
            return Err(crate::errors::ConcentratedLiquidityError::InvalidPositionTokenAmounts);
        }
    } else if amount_a == 0 || amount_b == 0 {
        return Err(crate::errors::ConcentratedLiquidityError::InvalidPositionTokenAmounts);
    }

    Ok(())
}

pub fn sqrt_price_x64_to_tick(_sqrt_price_x64: u128) -> i32 {
    // Placeholder conversion for MVP. Full logarithmic conversion comes with swap math.
    0
}

pub fn tick_to_sqrt_price_x64(_tick: i32) -> u128 {
    // Placeholder conversion for MVP. Full power-series conversion comes with swap math.
    crate::state::Q64_64_ONE
}

pub fn liquidity_from_amounts(
    amount_a: u64,
    amount_b: u64,
    tick_lower: i32,
    tick_upper: i32,
    sqrt_price_x64: u128,
) -> u128 {
    // Placeholder until full concentrated-liquidity math is added.
    // The branch structure still matches CLMM token-side semantics.
    let current_tick = sqrt_price_x64_to_tick(sqrt_price_x64);
    if current_tick < tick_lower {
        u128::from(amount_a)
    } else if current_tick >= tick_upper {
        u128::from(amount_b)
    } else {
        u128::from(amount_a.min(amount_b))
    }
}
