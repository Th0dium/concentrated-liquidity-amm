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

function App() {
  const [inputs, setInputs] = useState<InputState>(initialInputs);

  const parsed = useMemo(
    () => ({
      displayCurrentPrice: Number(inputs.currentPrice),
      displayLowerPrice: Number(inputs.lowerPrice),
      displayUpperPrice: Number(inputs.upperPrice),
      amountADisplay: Number(inputs.amountA),
      amountBDisplay: Number(inputs.amountB),
      decimalsA: Number(inputs.decimalsA),
      decimalsB: Number(inputs.decimalsB),
      tickSpacing: Number(inputs.tickSpacing),
    }),
    [inputs],
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

  return (
    <main className="app-shell">
      <section className="hero">
        <div>
          <p className="eyebrow">Concentrated Liquidity Math</p>
          <h1>CLMM Ratio Playground</h1>
        </div>
        <button className="ghost-button" type="button" onClick={() => setInputs(initialInputs)}>
          Reset
        </button>
      </section>

      <section className="workspace">
        <form className="input-panel">
          <Field
            label="Current price"
            value={inputs.currentPrice}
            onChange={(value) => updateInput("currentPrice", value)}
          />
          <Field
            label="Lower price"
            value={inputs.lowerPrice}
            onChange={(value) => updateInput("lowerPrice", value)}
          />
          <Field
            label="Upper price"
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

export default App;
