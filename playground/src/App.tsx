import { useMemo, useState } from "react";
import { buildQuote, type ClmmQuote, validateInputs } from "./clmmMath";

type InputState = {
  currentPrice: string;
  lowerPrice: string;
  upperPrice: string;
  amountA: string;
  amountB: string;
  decimalsA: string;
  decimalsB: string;
  tickSpacing: string;
};

type PriceInputMode = "price" | "sqrt";
type PlaygroundTab = "ratio" | "fee-growth";

type FeeLog = {
  text: string;
  type?: "highlight" | "flip";
};

type FeeSimState = {
  fgg: number;
  fgo: number[];
  currentTick: number;
  step: number;
  logs: FeeLog[];
};

const initialInputs: InputState = {
  currentPrice: "83",
  lowerPrice: "75",
  upperPrice: "95",
  amountA: "100",
  amountB: "6280",
  decimalsA: "6",
  decimalsB: "6",
  tickSpacing: "100",
};

const numFeeSimTicks = 6;
const feeGrowthPerStep = 10;
const initialFeeSimState: FeeSimState = {
  fgg: 0,
  fgo: new Array(numFeeSimTicks).fill(0),
  currentTick: 0,
  step: 0,
  logs: [{ text: "Initialized: price at tick 0, FGG = 0, all FGO values = 0." }],
};

function App() {
  const [activeTab, setActiveTab] = useState<PlaygroundTab>("ratio");
  const [inputs, setInputs] = useState<InputState>(initialInputs);
  const [priceInputMode, setPriceInputMode] = useState<PriceInputMode>("price");

  const parsed = useMemo(
    () => ({
      displayCurrentPrice: priceInputToDisplayPrice(inputs.currentPrice, priceInputMode),
      displayLowerPrice: priceInputToDisplayPrice(inputs.lowerPrice, priceInputMode),
      displayUpperPrice: priceInputToDisplayPrice(inputs.upperPrice, priceInputMode),
      amountADisplay: Number(inputs.amountA),
      amountBDisplay: Number(inputs.amountB),
      decimalsA: Number(inputs.decimalsA),
      decimalsB: Number(inputs.decimalsB),
      tickSpacing: Number(inputs.tickSpacing),
    }),
    [inputs, priceInputMode],
  );

  const validation = useMemo(() => validateInputs(parsed), [parsed]);
  const quotes = useMemo(() => {
    if (!validation.ok) {
      return null;
    }

    return [
      buildQuote({ ...parsed, mode: "continuous" }),
      buildQuote({ ...parsed, mode: "tick-snapped" }),
    ];
  }, [parsed, validation.ok]);

  function updateInput(key: keyof InputState, value: string) {
    setInputs((current) => ({ ...current, [key]: value }));
  }

  function togglePriceInputMode() {
    const nextMode: PriceInputMode = priceInputMode === "price" ? "sqrt" : "price";
    setInputs((current) => ({
      ...current,
      currentPrice: convertPriceInput(current.currentPrice, priceInputMode, nextMode),
      lowerPrice: convertPriceInput(current.lowerPrice, priceInputMode, nextMode),
      upperPrice: convertPriceInput(current.upperPrice, priceInputMode, nextMode),
    }));
    setPriceInputMode(nextMode);
  }

  function resetInputs() {
    setInputs(initialInputs);
    setPriceInputMode("price");
  }

  return (
    <main className="app-shell">
      <section className="hero">
        <div>
          <p className="eyebrow">Concentrated Liquidity Math</p>
          <h1>CLMM Ratio Playground</h1>
        </div>
        <button
          className="ghost-button"
          type="button"
          onClick={activeTab === "ratio" ? resetInputs : undefined}
          disabled={activeTab !== "ratio"}
        >
          Reset
        </button>
      </section>

      <nav className="tab-bar" aria-label="Playground tools">
        <button
          className={activeTab === "ratio" ? "active" : ""}
          type="button"
          onClick={() => setActiveTab("ratio")}
        >
          Ratio
        </button>
        <button
          className={activeTab === "fee-growth" ? "active" : ""}
          type="button"
          onClick={() => setActiveTab("fee-growth")}
        >
          Fee Growth
        </button>
      </nav>

      {activeTab === "ratio" ? (
        <section className="workspace">
          <form className="input-panel">
            <div className="mode-row">
              <span>Price input</span>
              <button
                className={`mode-toggle ${priceInputMode === "sqrt" ? "on" : ""}`}
                type="button"
                aria-pressed={priceInputMode === "sqrt"}
                onClick={togglePriceInputMode}
              >
                <span>Price</span>
                <span>Sqrt</span>
              </button>
            </div>
            <Field
              label={priceInputMode === "price" ? "Current price" : "Current sqrt price"}
              value={inputs.currentPrice}
              onChange={(value) => updateInput("currentPrice", value)}
            />
            <Field
              label={priceInputMode === "price" ? "Lower price" : "Lower sqrt price"}
              value={inputs.lowerPrice}
              onChange={(value) => updateInput("lowerPrice", value)}
            />
            <Field
              label={priceInputMode === "price" ? "Upper price" : "Upper sqrt price"}
              value={inputs.upperPrice}
              onChange={(value) => updateInput("upperPrice", value)}
            />
            <Field label="Amount A" value={inputs.amountA} onChange={(value) => updateInput("amountA", value)} />
            <Field label="Amount B" value={inputs.amountB} onChange={(value) => updateInput("amountB", value)} />
            <Field
              label="Token A decimals"
              value={inputs.decimalsA}
              step="1"
              onChange={(value) => updateInput("decimalsA", value)}
            />
            <Field
              label="Token B decimals"
              value={inputs.decimalsB}
              step="1"
              onChange={(value) => updateInput("decimalsB", value)}
            />
            <Field
              label="Tick spacing"
              value={inputs.tickSpacing}
              step="1"
              onChange={(value) => updateInput("tickSpacing", value)}
            />
          </form>

          <section className="results-panel">
            {!validation.ok && <div className="validation-banner">{validation.message}</div>}
            {quotes && (
              <>
                <div className="quote-grid">
                  {quotes.map((quote) => (
                    <QuoteCard key={quote.mode} quote={quote} />
                  ))}
                </div>
                <ComparisonTable continuous={quotes[0]} snapped={quotes[1]} />
              </>
            )}
          </section>
        </section>
      ) : (
        <FeeGrowthSimulator />
      )}
    </main>
  );
}

type FieldProps = {
  label: string;
  value: string;
  step?: string;
  onChange: (value: string) => void;
};

function Field({ label, value, step = "any", onChange }: FieldProps) {
  return (
    <label className="field">
      <span>{label}</span>
      <input type="number" value={value} step={step} onChange={(event) => onChange(event.target.value)} />
    </label>
  );
}

function QuoteCard({ quote }: { quote: ClmmQuote }) {
  return (
    <article className="quote-card">
      <div className="quote-heading">
        <h2>{quote.mode === "continuous" ? "Continuous Price" : "Tick-Snapped"}</h2>
        <span className={`status-pill ${quote.rangeState}`}>{rangeLabel(quote.rangeState)}</span>
      </div>

      {quote.warning && <p className="warning">{quote.warning}</p>}

      <dl className="metric-list">
        <Metric label="Minted liquidity" value={formatNumber(quote.liquidityMinted)} />
        <Metric label="Liquidity from A" value={formatNumber(quote.liquidityFromA)} />
        <Metric label="Liquidity from B" value={formatNumber(quote.liquidityFromB)} />
        <Metric label="Limiting token" value={limitingLabel(quote.limitingToken)} />
        <Metric label="Ideal B per A" value={formatNullable(quote.idealDisplayBPerA)} />
      </dl>

      <div className="token-grid">
        <TokenUsage title="Token A" consumed={quote.amountAConsumedDisplay} unused={quote.amountAUnusedDisplay} raw={quote.amountAConsumedRaw} />
        <TokenUsage title="Token B" consumed={quote.amountBConsumedDisplay} unused={quote.amountBUnusedDisplay} raw={quote.amountBConsumedRaw} />
      </div>
    </article>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function TokenUsage({
  title,
  consumed,
  unused,
  raw,
}: {
  title: string;
  consumed: number;
  unused: number;
  raw: number;
}) {
  return (
    <section className="token-usage">
      <h3>{title}</h3>
      <p>
        <span>Consumed</span>
        <strong>{formatNumber(consumed)}</strong>
      </p>
      <p>
        <span>Unused</span>
        <strong>{formatNumber(unused)}</strong>
      </p>
      <p>
        <span>Raw consumed</span>
        <strong>{formatNumber(raw)}</strong>
      </p>
    </section>
  );
}

function ComparisonTable({ continuous, snapped }: { continuous: ClmmQuote; snapped: ClmmQuote }) {
  const rows = [
    ["Lower sqrt", continuous.sqrtLower, snapped.sqrtLower],
    ["Current sqrt", continuous.sqrtCurrent, snapped.sqrtCurrent],
    ["Upper sqrt", continuous.sqrtUpper, snapped.sqrtUpper],
    ["Lower tick", continuous.lowerTick, snapped.lowerTick],
    ["Current tick", continuous.currentTick, snapped.currentTick],
    ["Upper tick", continuous.upperTick, snapped.upperTick],
    ["Raw lower price", continuous.rawLowerPrice, snapped.rawLowerPrice],
    ["Raw current price", continuous.rawCurrentPrice, snapped.rawCurrentPrice],
    ["Raw upper price", continuous.rawUpperPrice, snapped.rawUpperPrice],
    ["Display lower price", continuous.displayLowerPrice, snapped.displayLowerPrice],
    ["Display current price", continuous.displayCurrentPrice, snapped.displayCurrentPrice],
    ["Display upper price", continuous.displayUpperPrice, snapped.displayUpperPrice],
  ] as const;

  return (
    <section className="comparison">
      <h2>Price And Tick Trace</h2>
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Value</th>
              <th>Continuous</th>
              <th>Tick-snapped</th>
            </tr>
          </thead>
          <tbody>
            {rows.map(([label, continuousValue, snappedValue]) => (
              <tr key={label}>
                <td>{label}</td>
                <td>{formatNumber(continuousValue)}</td>
                <td>{formatNumber(snappedValue)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function FeeGrowthSimulator() {
  const [state, setState] = useState<FeeSimState>(initialFeeSimState);
  const [lowerTick, setLowerTick] = useState("0");
  const [upperTick, setUpperTick] = useState("3");
  const lower = Number(lowerTick);
  const upper = Number(upperTick);
  const lowerOptions = Array.from({ length: numFeeSimTicks - 1 }, (_, index) => index);
  const upperOptions = Array.from({ length: numFeeSimTicks - 1 }, (_, index) => index + 1);
  const fgi = lower < upper ? calcFeeGrowthInside(state, lower, upper) : null;
  const fgoBelowValue = lower < upper ? feeGrowthBelow(state, lower) : null;
  const fgoAboveValue = lower < upper ? feeGrowthAbove(state, upper) : null;

  function movePrice(direction: 1 | -1) {
    setState((current) => {
      const nextFgg = current.fgg + feeGrowthPerStep;
      const nextFgo = [...current.fgo];
      const crossedTick = direction === 1 ? current.currentTick + 1 : current.currentTick;
      const oldFgo = nextFgo[crossedTick];
      nextFgo[crossedTick] = nextFgg - oldFgo;
      const nextCurrentTick = current.currentTick + direction;
      const directionLabel = direction === 1 ? "up" : "down";

      return {
        fgg: nextFgg,
        fgo: nextFgo,
        currentTick: nextCurrentTick,
        step: current.step + 1,
        logs: [
          ...current.logs,
          {
            text: `Step ${current.step + 1}: price moved ${directionLabel}; FGG -> ${nextFgg}.`,
            type: "highlight",
          },
          {
            text: `Cross tick ${crossedTick}: FGO[${crossedTick}] flips ${oldFgo} -> ${nextFgg} - ${oldFgo} = ${nextFgo[crossedTick]}.`,
            type: "flip",
          },
        ],
      };
    });
  }

  function resetSimulator() {
    setState(initialFeeSimState);
    setLowerTick("0");
    setUpperTick("3");
  }

  return (
    <section className="fee-sim">
      <section className="fee-toolbar">
        <div>
          <span>fee_growth_global</span>
          <strong>{state.fgg}</strong>
        </div>
        <div className="fee-step">+{feeGrowthPerStep} fee growth per move</div>
      </section>

      <section className="fee-grid-wrap">
        <div className="fee-tick-grid">
          {state.fgo.map((value, tick) => (
            <article className={`fee-tick-card ${tick === state.currentTick ? "active" : ""}`} key={tick}>
              {tick === state.currentTick && <div className="price-marker">v</div>}
              <span>tick</span>
              <strong>{tick}</strong>
              <p>
                FGO: <b>{value}</b>
              </p>
            </article>
          ))}
        </div>
      </section>

      <div className="fee-status">Price at tick {state.currentTick} - step {state.step}</div>

      <section className="fee-controls">
        <button
          className="ghost-button"
          type="button"
          disabled={state.currentTick >= numFeeSimTicks - 1}
          onClick={() => movePrice(1)}
        >
          Price Up
        </button>
        <button
          className="ghost-button"
          type="button"
          disabled={state.currentTick <= 0}
          onClick={() => movePrice(-1)}
        >
          Price Down
        </button>
        <button className="ghost-button danger" type="button" onClick={resetSimulator}>
          Reset
        </button>
      </section>

      <section className="fgi-panel">
        <div className="fgi-controls">
          <label>
            <span>lower tick</span>
            <select value={lowerTick} onChange={(event) => setLowerTick(event.target.value)}>
              {lowerOptions.map((tick) => (
                <option key={tick} value={tick}>
                  {tick}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>upper tick</span>
            <select value={upperTick} onChange={(event) => setUpperTick(event.target.value)}>
              {upperOptions.map((tick) => (
                <option key={tick} value={tick}>
                  {tick}
                </option>
              ))}
            </select>
          </label>
          <strong>{fgi === null ? "Invalid range" : `FGI = ${fgi}`}</strong>
        </div>
        {fgi !== null && (
          <p>
            = FGG({state.fgg}) - fgo_below[{lower}]({fgoBelowValue}) - fgo_above[{upper}](
            {fgoAboveValue})
          </p>
        )}
      </section>

      <section className="fee-log-panel">
        <h2>Event Log</h2>
        <div className="fee-log">
          {[...state.logs].reverse().map((log, index) => (
            <div className={`fee-log-entry ${log.type ?? ""}`} key={`${log.text}-${index}`}>
              {log.text}
            </div>
          ))}
        </div>
      </section>
    </section>
  );
}

function rangeLabel(state: ClmmQuote["rangeState"]) {
  if (state === "below") {
    return "Below range";
  }

  if (state === "above") {
    return "Above range";
  }

  return "In range";
}

function limitingLabel(token: ClmmQuote["limitingToken"]) {
  if (token === "balanced") {
    return "Balanced";
  }

  if (token === "none") {
    return "None";
  }

  return `Token ${token}`;
}

function formatNullable(value: number | null) {
  return value === null ? "One-sided" : formatNumber(value);
}

function formatNumber(value: number) {
  if (!Number.isFinite(value)) {
    return "Invalid";
  }

  if (value === 0) {
    return "0";
  }

  const absolute = Math.abs(value);
  if (absolute >= 1_000_000_000 || absolute < 0.000001) {
    return value.toExponential(6);
  }

  return new Intl.NumberFormat("en-US", {
    maximumFractionDigits: absolute >= 1 ? 6 : 12,
  }).format(value);
}

function priceInputToDisplayPrice(value: string, mode: PriceInputMode) {
  const parsed = Number(value);
  if (mode === "sqrt" && parsed < 0) {
    return Number.NaN;
  }

  return mode === "sqrt" ? parsed * parsed : parsed;
}

function convertPriceInput(value: string, currentMode: PriceInputMode, nextMode: PriceInputMode) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < 0) {
    return value;
  }

  const converted = currentMode === "price" && nextMode === "sqrt" ? Math.sqrt(parsed) : parsed * parsed;
  return formatInputNumber(converted);
}

function formatInputNumber(value: number) {
  if (!Number.isFinite(value)) {
    return "";
  }

  return Number(value.toPrecision(12)).toString();
}

function feeGrowthBelow(state: FeeSimState, tick: number) {
  return state.currentTick >= tick ? state.fgo[tick] : state.fgg - state.fgo[tick];
}

function feeGrowthAbove(state: FeeSimState, tick: number) {
  return state.currentTick >= tick ? state.fgg - state.fgo[tick] : state.fgo[tick];
}

function calcFeeGrowthInside(state: FeeSimState, lower: number, upper: number) {
  return state.fgg - feeGrowthBelow(state, lower) - feeGrowthAbove(state, upper);
}

export default App;
