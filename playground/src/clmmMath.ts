export const TICK_BASE = 1.0001;

export type RangeState = "below" | "inside" | "above";
export type LimitingToken = "A" | "B" | "balanced" | "none";

export type QuoteMode = "continuous" | "tick-snapped";

export interface QuoteInput {
  displayCurrentPrice: number;
  displayLowerPrice: number;
  displayUpperPrice: number;
  amountADisplay: number;
  amountBDisplay: number;
  decimalsA: number;
  decimalsB: number;
  tickSpacing: number;
  mode: QuoteMode;
}

export interface ClmmQuote {
  mode: QuoteMode;
  rangeState: RangeState;
  limitingToken: LimitingToken;
  rawAmountAMax: number;
  rawAmountBMax: number;
  rawCurrentPrice: number;
  rawLowerPrice: number;
  rawUpperPrice: number;
  displayCurrentPrice: number;
  displayLowerPrice: number;
  displayUpperPrice: number;
  sqrtCurrent: number;
  sqrtLower: number;
  sqrtUpper: number;
  currentTick: number;
  lowerTick: number;
  upperTick: number;
  liquidityFromA: number;
  liquidityFromB: number;
  liquidityMinted: number;
  amountAConsumedRaw: number;
  amountBConsumedRaw: number;
  amountAUnusedRaw: number;
  amountBUnusedRaw: number;
  amountAConsumedDisplay: number;
  amountBConsumedDisplay: number;
  amountAUnusedDisplay: number;
  amountBUnusedDisplay: number;
  idealRawBPerRawA: number | null;
  idealDisplayBPerA: number | null;
  warning: string | null;
}

export interface ValidationResult {
  ok: boolean;
  message: string | null;
}

export function validateInputs(input: Omit<QuoteInput, "mode">): ValidationResult {
  const numbers = [
    input.displayCurrentPrice,
    input.displayLowerPrice,
    input.displayUpperPrice,
    input.amountADisplay,
    input.amountBDisplay,
    input.decimalsA,
    input.decimalsB,
    input.tickSpacing,
  ];

  if (numbers.some((value) => !Number.isFinite(value))) {
    return { ok: false, message: "Every input must be a finite number." };
  }

  if (
    input.displayCurrentPrice <= 0 ||
    input.displayLowerPrice <= 0 ||
    input.displayUpperPrice <= 0
  ) {
    return { ok: false, message: "Prices must be greater than zero." };
  }

  if (input.displayLowerPrice >= input.displayUpperPrice) {
    return { ok: false, message: "Lower price must be less than upper price." };
  }

  if (input.amountADisplay < 0 || input.amountBDisplay < 0) {
    return { ok: false, message: "Token amounts cannot be negative." };
  }

  if (input.amountADisplay === 0 && input.amountBDisplay === 0) {
    return { ok: false, message: "At least one token budget must be greater than zero." };
  }

  if (!isIntegerInRange(input.decimalsA, 0, 18) || !isIntegerInRange(input.decimalsB, 0, 18)) {
    return { ok: false, message: "Token decimals must be whole numbers from 0 to 18." };
  }

  if (!isIntegerInRange(input.tickSpacing, 1, 100_000)) {
    return { ok: false, message: "Tick spacing must be a positive whole number." };
  }

  return { ok: true, message: null };
}

export function buildQuote(input: QuoteInput): ClmmQuote {
  const rawAmountAMax = displayAmountToRaw(input.amountADisplay, input.decimalsA);
  const rawAmountBMax = displayAmountToRaw(input.amountBDisplay, input.decimalsB);
  const decimalPriceScale = 10 ** (input.decimalsB - input.decimalsA);

  const rawCurrentInput = input.displayCurrentPrice * decimalPriceScale;
  const rawLowerInput = input.displayLowerPrice * decimalPriceScale;
  const rawUpperInput = input.displayUpperPrice * decimalPriceScale;

  const currentTick = priceToTick(rawCurrentInput);
  const lowerTick = floorToSpacing(priceToTick(rawLowerInput), input.tickSpacing);
  const upperTick = ceilToSpacing(priceToTick(rawUpperInput), input.tickSpacing);

  const rawCurrentPrice =
    input.mode === "tick-snapped" ? tickToPrice(currentTick) : rawCurrentInput;
  const rawLowerPrice = input.mode === "tick-snapped" ? tickToPrice(lowerTick) : rawLowerInput;
  const rawUpperPrice = input.mode === "tick-snapped" ? tickToPrice(upperTick) : rawUpperInput;

  const sqrtCurrent = Math.sqrt(rawCurrentPrice);
  const sqrtLower = Math.sqrt(rawLowerPrice);
  const sqrtUpper = Math.sqrt(rawUpperPrice);
  const rangeState = getRangeState(sqrtCurrent, sqrtLower, sqrtUpper);

  const liquidityFromA =
    rangeState === "above"
      ? 0
      : liquidityFromAmountA(
          rawAmountAMax,
          rangeState === "below" ? sqrtLower : sqrtCurrent,
          sqrtUpper,
        );
  const liquidityFromB =
    rangeState === "below"
      ? 0
      : liquidityFromAmountB(
          rawAmountBMax,
          sqrtLower,
          rangeState === "above" ? sqrtUpper : sqrtCurrent,
        );

  const liquidityMinted = chooseLiquidity(rangeState, liquidityFromA, liquidityFromB);
  const { amountAConsumedRaw, amountBConsumedRaw } = amountsForLiquidity(
    liquidityMinted,
    sqrtLower,
    sqrtCurrent,
    sqrtUpper,
    rangeState,
  );

  const amountAUnusedRaw = Math.max(rawAmountAMax - amountAConsumedRaw, 0);
  const amountBUnusedRaw = Math.max(rawAmountBMax - amountBConsumedRaw, 0);
  const limitingToken = findLimitingToken(rangeState, liquidityFromA, liquidityFromB, liquidityMinted);
  const { idealRawBPerRawA, idealDisplayBPerA } = idealDepositRatio(
    rangeState,
    sqrtLower,
    sqrtCurrent,
    sqrtUpper,
    input.decimalsA,
    input.decimalsB,
  );

  return {
    mode: input.mode,
    rangeState,
    limitingToken,
    rawAmountAMax,
    rawAmountBMax,
    rawCurrentPrice,
    rawLowerPrice,
    rawUpperPrice,
    displayCurrentPrice: rawPriceToDisplay(rawCurrentPrice, input.decimalsA, input.decimalsB),
    displayLowerPrice: rawPriceToDisplay(rawLowerPrice, input.decimalsA, input.decimalsB),
    displayUpperPrice: rawPriceToDisplay(rawUpperPrice, input.decimalsA, input.decimalsB),
    sqrtCurrent,
    sqrtLower,
    sqrtUpper,
    currentTick,
    lowerTick,
    upperTick,
    liquidityFromA,
    liquidityFromB,
    liquidityMinted,
    amountAConsumedRaw,
    amountBConsumedRaw,
    amountAUnusedRaw,
    amountBUnusedRaw,
    amountAConsumedDisplay: rawAmountToDisplay(amountAConsumedRaw, input.decimalsA),
    amountBConsumedDisplay: rawAmountToDisplay(amountBConsumedRaw, input.decimalsB),
    amountAUnusedDisplay: rawAmountToDisplay(amountAUnusedRaw, input.decimalsA),
    amountBUnusedDisplay: rawAmountToDisplay(amountBUnusedRaw, input.decimalsB),
    idealRawBPerRawA,
    idealDisplayBPerA,
    warning:
      liquidityMinted > 0
        ? null
        : "This token composition mints zero liquidity for the current range state.",
  };
}

export function displayAmountToRaw(amount: number, decimals: number): number {
  return Math.floor(amount * 10 ** decimals);
}

export function rawAmountToDisplay(amount: number, decimals: number): number {
  return amount / 10 ** decimals;
}

export function rawPriceToDisplay(rawPrice: number, decimalsA: number, decimalsB: number): number {
  return rawPrice / 10 ** (decimalsB - decimalsA);
}

export function priceToTick(price: number): number {
  return Math.floor(Math.log(price) / Math.log(TICK_BASE));
}

export function tickToPrice(tick: number): number {
  return TICK_BASE ** tick;
}

function liquidityFromAmountA(amountA: number, sqrtLower: number, sqrtUpper: number): number {
  const denominator = sqrtUpper - sqrtLower;
  if (amountA <= 0 || denominator <= 0) {
    return 0;
  }

  return Math.floor((amountA * sqrtLower * sqrtUpper) / denominator);
}

function liquidityFromAmountB(amountB: number, sqrtLower: number, sqrtUpper: number): number {
  const denominator = sqrtUpper - sqrtLower;
  if (amountB <= 0 || denominator <= 0) {
    return 0;
  }

  return Math.floor(amountB / denominator);
}

function amountAForLiquidity(liquidity: number, sqrtLower: number, sqrtUpper: number): number {
  if (liquidity <= 0) {
    return 0;
  }

  return Math.ceil((liquidity * (sqrtUpper - sqrtLower)) / (sqrtUpper * sqrtLower));
}

function amountBForLiquidity(liquidity: number, sqrtLower: number, sqrtUpper: number): number {
  if (liquidity <= 0) {
    return 0;
  }

  return Math.ceil(liquidity * (sqrtUpper - sqrtLower));
}

function amountsForLiquidity(
  liquidity: number,
  sqrtLower: number,
  sqrtCurrent: number,
  sqrtUpper: number,
  rangeState: RangeState,
): Pick<ClmmQuote, "amountAConsumedRaw" | "amountBConsumedRaw"> {
  if (rangeState === "below") {
    return {
      amountAConsumedRaw: amountAForLiquidity(liquidity, sqrtLower, sqrtUpper),
      amountBConsumedRaw: 0,
    };
  }

  if (rangeState === "above") {
    return {
      amountAConsumedRaw: 0,
      amountBConsumedRaw: amountBForLiquidity(liquidity, sqrtLower, sqrtUpper),
    };
  }

  return {
    amountAConsumedRaw: amountAForLiquidity(liquidity, sqrtCurrent, sqrtUpper),
    amountBConsumedRaw: amountBForLiquidity(liquidity, sqrtLower, sqrtCurrent),
  };
}

function chooseLiquidity(rangeState: RangeState, liquidityFromA: number, liquidityFromB: number): number {
  if (rangeState === "below") {
    return liquidityFromA;
  }

  if (rangeState === "above") {
    return liquidityFromB;
  }

  return Math.min(liquidityFromA, liquidityFromB);
}

function getRangeState(sqrtCurrent: number, sqrtLower: number, sqrtUpper: number): RangeState {
  if (sqrtCurrent <= sqrtLower) {
    return "below";
  }

  if (sqrtCurrent >= sqrtUpper) {
    return "above";
  }

  return "inside";
}

function idealDepositRatio(
  rangeState: RangeState,
  sqrtLower: number,
  sqrtCurrent: number,
  sqrtUpper: number,
  decimalsA: number,
  decimalsB: number,
): Pick<ClmmQuote, "idealRawBPerRawA" | "idealDisplayBPerA"> {
  if (rangeState !== "inside") {
    return { idealRawBPerRawA: null, idealDisplayBPerA: null };
  }

  const idealRawBPerRawA =
    (sqrtCurrent * sqrtUpper * (sqrtCurrent - sqrtLower)) / (sqrtUpper - sqrtCurrent);

  return {
    idealRawBPerRawA,
    idealDisplayBPerA: rawPriceToDisplay(idealRawBPerRawA, decimalsA, decimalsB),
  };
}

function findLimitingToken(
  rangeState: RangeState,
  liquidityFromA: number,
  liquidityFromB: number,
  liquidityMinted: number,
): LimitingToken {
  if (liquidityMinted === 0) {
    return "none";
  }

  if (rangeState === "below") {
    return "A";
  }

  if (rangeState === "above") {
    return "B";
  }

  if (liquidityFromA === liquidityFromB) {
    return "balanced";
  }

  return liquidityFromA < liquidityFromB ? "A" : "B";
}

function floorToSpacing(tick: number, spacing: number): number {
  return Math.floor(tick / spacing) * spacing;
}

function ceilToSpacing(tick: number, spacing: number): number {
  return Math.ceil(tick / spacing) * spacing;
}

function isIntegerInRange(value: number, min: number, max: number): boolean {
  return Number.isInteger(value) && value >= min && value <= max;
}
