use crate::state::TICK_ARRAY_SIZE;

/// Calculates the total tick range covered by one tick array (TICK_ARRAY_SIZE * tick_spacing).
pub fn tick_array_span(tick_spacing: u16) -> Result<i32, crate::errors::ConcentratedLiquidityError> {
    let spacing = i32::from(tick_spacing);
    let array_span = spacing
        .checked_mul(TICK_ARRAY_SIZE as i32)
        .ok_or(crate::errors::ConcentratedLiquidityError::TickMathOverflow)?;
    Ok(array_span)
}

/// Returns the start tick index of the tick array that contains the given tick.
pub fn tick_array_start_index(
    tick_index: i32,
    tick_spacing: u16,
) -> Result<i32, crate::errors::ConcentratedLiquidityError> {
    let array_span = tick_array_span(tick_spacing)?;
    let quotient = tick_index.div_euclid(array_span);
    quotient
        .checked_mul(array_span)
        .ok_or(crate::errors::ConcentratedLiquidityError::TickMathOverflow)
}

/// Finds the array index offset of a tick within its tick array.
pub fn tick_offset_in_array(
    start_tick_index: i32,
    tick_index: i32,
    tick_spacing: u16,
) -> Result<usize, crate::errors::ConcentratedLiquidityError> {
    let spacing = i32::from(tick_spacing);
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

/// Validates that a tick index is aligned with the pool's tick spacing.
pub fn validate_tick_alignment(
    tick_index: i32,
    tick_spacing: u16,
) -> Result<(), crate::errors::ConcentratedLiquidityError> {
    let spacing = i32::from(tick_spacing);
    if tick_index % spacing != 0 {
        return Err(crate::errors::ConcentratedLiquidityError::TickNotAligned);
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
) -> Result<(), crate::errors::ConcentratedLiquidityError> {
    if amount_a == 0 && amount_b == 0 {
        return Err(crate::errors::ConcentratedLiquidityError::ZeroLiquidityDeposit);
    }

    if tick_lower > current_tick {
        if amount_a == 0 || amount_b > 0 {
            return Err(crate::errors::ConcentratedLiquidityError::InvalidPositionTokenAmounts);
        }
    } else if tick_upper <= current_tick  {
        if amount_a > 0 || amount_b == 0 {
            return Err(crate::errors::ConcentratedLiquidityError::InvalidPositionTokenAmounts);
        }
    } else if amount_a == 0 || amount_b == 0 {
        return Err(crate::errors::ConcentratedLiquidityError::InvalidPositionTokenAmounts);
    }

    Ok(())
}

/// Converts Q64.64 sqrt price to tick index (placeholder returns 0 until swap math is implemented).
pub fn sqrt_price_x64_to_tick(_sqrt_price_x64: u128) -> i32 {
    // Placeholder conversion for MVP. Full logarithmic conversion comes with swap math.
    
    // Real Implementation (Future):
    // Input: sqrt_price_x64 = sqrt(price_ratio) in Q64.64 format
    // Output: tick_index where price = 1.0001^tick
    
    // Example 1: Equal price (1:1 ratio)
    // sqrt_price_x64 = 18446744073709551616  // sqrt(1.0) in Q64.64 = 2^64
    // tick = 0
    
    // Example 2: Token B is 2x more expensive than Token A
    // price_ratio = 2.0 (need 2 token_a to get 1 token_b)
    // sqrt_price = sqrt(2.0) ≈ 1.414213562
    // sqrt_price_x64 = 1.414213562 * 2^64 ≈ 26087635650665564160
    // tick = log(2.0) / log(1.0001) ≈ 6931
    
    // Example 3: USDC/SOL pool where SOL = $100, USDC = $1
    // price_ratio = 100.0 (need 100 USDC to get 1 SOL)
    // sqrt_price = sqrt(100.0) = 10.0
    // sqrt_price_x64 = 10.0 * 2^64 ≈ 184467440737095516160
    // tick = log(100.0) / log(1.0001) ≈ 46051
    
    // Formula:
    // tick = floor(log(price) / log(1.0001))
    //      = floor(log((sqrt_price_x64 / 2^64)^2) / log(1.0001))
    //      = floor(2 * log(sqrt_price_x64 / 2^64) / log(1.0001))
    
    // Implementation approach:
    // 1. Convert Q64.64 to floating point or use fixed-point log
    // 2. Compute log2(sqrt_price_x64) - 64 (since we stored * 2^64)
    // 3. Multiply by 2 (to square the sqrt)
    // 4. Divide by log2(1.0001) ≈ 0.00014426950408889634
    // 5. Floor to get integer tick
    
    0
}

/// Converts tick index to Q64.64 sqrt price (placeholder returns Q64_64_ONE until swap math is implemented).
pub fn tick_to_sqrt_price_x64(_tick: i32) -> u128 {
    // Placeholder conversion for MVP. Full power-series conversion comes with swap math.
    
    // Real Implementation (Future):
    // Input: tick_index (integer)
    // Output: sqrt_price_x64 = sqrt(1.0001^tick) in Q64.64 format
    
    // Example 1: Tick 0 (equal price)
    // tick = 0
    // price = 1.0001^0 = 1.0
    // sqrt_price = sqrt(1.0) = 1.0
    // sqrt_price_x64 = 1.0 * 2^64 = 18446744073709551616
    
    // Example 2: Tick 6931 (2x price)
    // tick = 6931
    // price = 1.0001^6931 ≈ 2.0
    // sqrt_price = sqrt(2.0) ≈ 1.414213562
    // sqrt_price_x64 = 1.414213562 * 2^64 ≈ 26087635650665564160
    
    // Example 3: Tick 46051 (100x price, like $100 SOL vs $1 USDC)
    // tick = 46051
    // price = 1.0001^46051 ≈ 100.0
    // sqrt_price = sqrt(100.0) = 10.0
    // sqrt_price_x64 = 10.0 * 2^64 = 184467440737095516160
    
    // Example 4: Tick -6931 (0.5x price, token A is 2x more expensive)
    // tick = -6931
    // price = 1.0001^(-6931) ≈ 0.5
    // sqrt_price = sqrt(0.5) ≈ 0.707106781
    // sqrt_price_x64 = 0.707106781 * 2^64 ≈ 13043817825332782080
    
    // Formula:
    // sqrt_price_x64 = sqrt(1.0001^tick) * 2^64
    
    // Implementation approach:
    // 1. Compute 1.0001^tick using power series or lookup table
    // 2. Take square root
    // 3. Multiply by 2^64 to convert to Q64.64 format
    // 
    // Optimization: Use precomputed lookup table for common tick ranges
    // and interpolate for precision, or use Taylor series expansion
    
    crate::state::Q64_64_ONE
}

/// Calculates liquidity amount from t
    pub fn liquidity_from_amounts(
    amount_a: u64,
    amount_b: u64,
    tick_lower: i32,
    tick_upper: i32,
    sqrt_price_x64: u128,
) -> u128 {
    // Placeholder until full concentrated-liquidity math is added.
    // The branch structure still matches CLMM token-side semantics.
    
    // Real Implementation (Future):
    // Liquidity (L) represents the "virtual reserves" in a concentrated range.
    // It's the constant that maintains x*y=k within the tick range.
    
    // Formulas (from Uniswap V3):
    // When current_tick < tick_lower (position is all token A):
    //   L = amount_a / (1/sqrt(P_lower) - 1/sqrt(P_upper))
    //   L = amount_a * (sqrt(P_upper) * sqrt(P_lower)) / (sqrt(P_upper) - sqrt(P_lower))
    
    // When current_tick >= tick_upper (position is all token B):
    //   L = amount_b / (sqrt(P_upper) - sqrt(P_lower))
    
    // When tick_lower <= current_tick < tick_upper (position has both tokens):
    //   L_a = amount_a / (1/sqrt(P_current) - 1/sqrt(P_upper))
    //   L_b = amount_b / (sqrt(P_current) - sqrt(P_lower))
    //   L = min(L_a, L_b)  // Take the limiting token
    
    // Example 1: Position below current price (only token A)
    // tick_lower = -1000, tick_upper = 0, current_tick = 1000
    // amount_a = 1_000_000 (1 token with 6 decimals, like USDC)
    // amount_b = 0
    // sqrt_price_lower = sqrt(1.0001^-1000) ≈ 0.9048
    // sqrt_price_upper = sqrt(1.0001^0) = 1.0
    // L = 1_000_000 * (1.0 * 0.9048) / (1.0 - 0.9048)
    //   = 1_000_000 * 0.9048 / 0.0952
    //   ≈ 9_504_201
    
    // Example 2: Position above current price (only token B)
    // tick_lower = 1000, tick_upper = 2000, current_tick = 0
    // amount_a = 0
    // amount_b = 2_000_000_000 (2 tokens with 9 decimals, like SOL)
    // sqrt_price_lower = sqrt(1.0001^1000) ≈ 1.0513
    // sqrt_price_upper = sqrt(1.0001^2000) ≈ 1.1052
    // L = 2_000_000_000 / (1.1052 - 1.0513)
    //   = 2_000_000_000 / 0.0539
    //   ≈ 37_105_381_076
    
    // Example 3: Position in range (both tokens)
    // tick_lower = -500, tick_upper = 500, current_tick = 0
    // amount_a = 1_000_000 (USDC)
    // amount_b = 1_000_000_000 (SOL)
    // sqrt_price_current = 1.0
    // sqrt_price_lower = sqrt(1.0001^-500) ≈ 0.9512
    // sqrt_price_upper = sqrt(1.0001^500) ≈ 1.0513
    // L_a = 1_000_000 / (1/1.0 - 1/1.0513) = 1_000_000 / (1.0 - 0.9512) ≈ 20_491_803
    // L_b = 1_000_000_000 / (1.0 - 0.9512) ≈ 20_491_803_278
    // L = min(L_a, L_b) = 20_491_803
    
    // Note: All calculations use Q64.64 fixed-point to avoid floating point errors
    // sqrt_price values are stored as (actual_value * 2^64)
    
    let current_tick = sqrt_price_x64_to_tick(sqrt_price_x64);
    if current_tick < tick_lower {
        u128::from(amount_a)
    } else if current_tick >= tick_upper {
        u128::from(amount_b)
    } else {
        u128::from(amount_a.min(amount_b))
    }
}
