# Concentrated Liquidity Pool - Project Log

**Project:** Solana Concentrated Liquidity AMM Smart Contract  
**Owner:** Daniel (Th0dium)  
**Status:** Stage 1 - Project Initialization  
**Last Updated:** 2026-03-30  

---

## Problem Statement

Build a Solana program that implements a concentrated liquidity AMM pool where:
- Users can create liquidity positions within specific price tick ranges
- Swaps route through active ticks using the constant product formula (x*y=k)
- LPs earn fees (0.30%) proportional to their liquidity in active ranges
- Multiple positions can coexist and earn fees independently

### Why This Problem?
- **Real protocol logic** - Uniswap V3 makes billions partly because of concentrated liquidity design
- **Complex state management** - Requires sparse data structures, range queries, position tracking
- **Production Solana patterns** - Uses PDAs, CPIs, SPL tokens, fixed-point math
- **Capital efficiency** - Forces understanding of how modern DeFi protocols maximize returns

---

## Target Specifications

### Scope
- **Token pair:** Any two SPL tokens (e.g., USDC/SOL, ETH/USDC)
- **Fee tier:** 0.30% (hardcoded initially, could be made dynamic)
- **Tick system:** 1 tick = 1% price movement
- **Positions:** LPs specify [tick_lower, tick_upper], deposit tokens, get liquidity rights
- **Swaps:** Route through ticks, collect fees, distribute to active LPs

### Success Criteria
1. ✅ Initialize a pool with two tokens
2. ✅ Create a position (deposit liquidity in a tick range)
3. ✅ Execute a swap that crosses multiple ticks
4. ✅ Withdraw liquidity and claim accumulated fees
5. ✅ Multiple positions coexist and earn fees independently
6. ✅ Constant product formula maintained (x*y=k)

### Non-Goals (Phase 1)
- NFT positions (regular accounts for now)
- Dynamic fee tiers (hardcoded 0.30%)
- Governance or admin functions
- Mainnet deployment

---

## Architecture

### Account Structure

```
Pool State (PDA)
├─ Seed: [b"pool", token_a_mint, token_b_mint]
├─ Stores:
│  ├─ token_a_mint: Pubkey
│  ├─ token_b_mint: Pubkey
│  ├─ token_a_vault: Pubkey
│  ├─ token_b_vault: Pubkey
│  ├─ fee_tier: u64 (basis points, e.g., 30 for 0.30%)
│  ├─ current_tick: i32
│  ├─ total_liquidity: u128 (across all positions)
│  └─ tick_liquidity: BTreeMap<i32, u128>

Token Vault A (PDA)
├─ Seed: [b"vault_a", pool_address]
├─ Authority: Pool PDA (program signs transfers)
└─ Holds: Actual token_a balance (SPL Token Account)

Token Vault B (PDA)
├─ Seed: [b"vault_b", pool_address]
├─ Authority: Pool PDA (program signs transfers)
└─ Holds: Actual token_b balance (SPL Token Account)

Position (PDA per LP)
├─ Seed: [b"position", owner, position_id]
├─ Stores:
│  ├─ owner: Pubkey
│  ├─ pool: Pubkey
│  ├─ tick_lower: i32
│  ├─ tick_upper: i32
│  ├─ liquidity_amount: u128
│  ├─ fees_a: u128 (accumulated)
│  └─ fees_b: u128 (accumulated)
└─ Authority: Owner (only they can close/withdraw)
```

### Why These Choices?

**PDAs for vaults:** Program needs to sign SPL token transfers on behalf of the pool. Keypairs can't do this.

**PDAs for positions:** Each LP's position needs a stable address. Owner can derive their own addresses (like a self-sovereign wallet).

**BTreeMap for tick_liquidity:** Only store ticks with active liquidity (sparse). Efficient for iteration during swaps.

---

## Instructions (Phased)

### Phase 1: Core Infrastructure
- **initialize_pool(token_a, token_b)**
  - Validate tokens exist and are different
  - Create pool state PDA
  - Create token vault PDAs
  - Initialize empty tick_liquidity map
  - **Accounts needed:** TokenMint A, TokenMint B, Pool PDA, Vault A, Vault B, Signer

- **create_position(tick_lower, tick_upper, amount_a, amount_b)**
  - Validate tick range (lower < upper, within bounds)
  - Validate amounts > 0
  - Calculate liquidity from amounts (using current price)
  - Create position PDA
  - Transfer tokens from LP to vaults (via CPI to SPL Token)
  - Update tick_liquidity for all ticks in range
  - **Accounts needed:** LP wallet, Position PDA, Pool PDA, Vault A, Vault B, TokenAccount A, TokenAccount B, Signer

- **swap(amount_in, minimum_amount_out, is_a_to_b)**
  - Calculate input with fees (amount_in * (1 - fee_tier))
  - Route through active ticks using x*y=k
  - Accumulate fees per tick for all active LPs
  - Transfer input to vault, output from vault (via CPI)
  - Validate slippage (output >= minimum_amount_out)
  - **Accounts needed:** Swapper wallet, Pool PDA, Vault A, Vault B, TokenAccount A, TokenAccount B, Signer

- **close_position(position_id)**
  - Validate position exists and owner is signer
  - Calculate LP's share of accumulated fees per tick
  - Transfer liquidity + fees back to LP
  - Delete position PDA
  - Update tick_liquidity
  - **Accounts needed:** LP wallet, Position PDA, Pool PDA, Vault A, Vault B, TokenAccount A, TokenAccount B, Signer

### Phase 2 (Future)
- claim_fees (without closing position)
- adjust_position (rebalance tick range)
- admin functions (change fee tier, pause pool)

---

## Math & Precision

### Constant Product Formula
```
Before swap: x * y = k
After swap:  x' * y' >= k (with rounding)

Example:
- Pool has 100 ETH, 300,000 USDC (k = 30,000,000)
- User swaps 30,000 USDC
- New USDC balance: 330,000
- New ETH balance: k / 330,000 = 90.9 ETH
- User gets: 100 - 90.9 = 9.1 ETH
- Price shift: from 3,000 to 3,626 USDC/ETH
```

### Decimal Handling
- USDC: 6 decimals (1 USDC = 10^6 smallest units)
- SOL: 9 decimals (1 SOL = 10^9 lamports)
- Algorithm: All intermediate calculations in smallest units, scale output to token decimals
- **Rule:** Never divide by decimals directly; use fixed-point conversion

### Tick Calculation
```
price_ratio = y / x (token_b per token_a)
tick = log(price_ratio) / log(1.01)  // 1% per tick
// In code: use lookup table or approximate
```

### Fee Distribution
```
// When swap crosses a tick:
fees_this_tick = amount_in * fee_tier
per_lp_share = fees_this_tick * (lp_liquidity / total_liquidity_in_tick)

// Tracked per position:
position.fees_a += share
position.fees_b += share
```

### Rounding Rules
- **Swap outputs:** Round DOWN (1 wei favors pool, prevents exploits)
- **Fee distribution:** Round DOWN per LP (pool keeps remainder)
- **Liquidity calculations:** Use u128 for intermediate steps to avoid overflow

---

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Vault type | PDA | Program needs to sign SPL transfers |
| Position type | PDA | Owner can self-derive, durable address |
| Tick storage | BTreeMap (sparse) | Only store active ticks, save space |
| Fee model | Per-tick accumulation | Efficient for multi-position swaps |
| Rounding | DOWN | Favors pool, prevents dust attacks |
| Initial tick | 0 | Requires external price oracle for real deployment |

---

## Development Progress

### Stage 1: Scaffold (Current)
- [ ] `anchor init concentrated_liquidity`
- [ ] Verify `anchor build` works
- [ ] Add dependencies: anchor-spl, spl-token
- [ ] Create project file structure

**Checkpoint:** Daniel explains what `lib.rs` does and why we need each dependency

---

### Stage 2: State & Accounts (Upcoming)
- [ ] Define PoolState struct
- [ ] Define Position struct
- [ ] Define TickState or use BTreeMap
- [ ] Add account validation macros

**Checkpoint:** Daniel explains why PoolState is a PDA and Position is a PDA

---

### Stage 3: Instructions (Upcoming)
- [ ] Implement initialize_pool
- [ ] Implement create_position
- [ ] Implement swap (hardest part)
- [ ] Implement close_position

**Checkpoint:** Daniel traces through one full swap and explains tick crossing

---

### Stage 4: Integration (Upcoming)
- [ ] Full test suite
- [ ] Edge case handling (empty pools, single-position swaps)
- [ ] Devnet deployment

**Checkpoint:** Daniel debugs a failing test by understanding account state

---

## Known Challenges & Solutions

| Challenge | Why It's Hard | Approach |
|-----------|---------------|----------|
| Tick crossing | Swap must route through multiple ticks with different liquidity | Iterate through active ticks, update price dynamically |
| Fee distribution | Different LPs have different shares of each tick | Track fees per tick, distribute proportionally on close_position |
| Decimal mismatch | Token A has 6 decimals, Token B has 9 | Convert everything to smallest units, scale back for output |
| Overflow | u64 overflows with large amounts | Use u128 for intermediate calculations |
| Price oracle | How to initialize current_tick? | Hardcode to 0 for MVP; real deployment needs Pyth/Chainlink |

---

## Deployment & Testing

### Local Testing
- Anchor test framework (Solana local validator)
- Mock token mints and user wallets
- Test scenarios:
  1. Pool creation with realistic token pairs
  2. Multiple LPs deposit in overlapping tick ranges
  3. Large swap crossing many ticks
  4. Partial position withdrawal

### Devnet (After MVP)
- Deploy to Solana devnet
- Real SPL token mints
- Multiple LPs can interact
- **Hard stop:** No mainnet without explicit Daniel approval

---

## References & Resources

- **Uniswap V3 Whitepaper:** https://uniswap.org/whitepaper-v3.pdf (concentrated liquidity math)
- **Solana Program Library (SPL):** https://github.com/solana-labs/solana-program-library (token standards)
- **Anchor Framework:** https://www.anchor-lang.com (Solana dev framework)
- **Fixed-Point Math:** https://en.wikipedia.org/wiki/Fixed-point_arithmetic (precision handling)

---

## Notes for Agent

- **Daniel's learning style:** Builds → understands → explains back (not lectures first)
- **Daniel's background:** Familiar with Solana basics, vibe coding, smart contract fundamentals
- **Daniel's goal:** Master DeFi protocol architecture for future independent projects
- **Escalation:** If Daniel gets stuck on a concept, pause coding and switch to Socratic Q&A

---

## File Structure (To Be Created)

```
concentrated_liquidity/
├── programs/
│   └── concentrated_liquidity/
│       ├── src/
│       │   ├── lib.rs              // Entry point, instruction routing
│       │   ├── state.rs            // PoolState, Position, TickState
│       │   ├── instructions/
│       │   │   ├── mod.rs
│       │   │   ├── initialize_pool.rs
│       │   │   ├── create_position.rs
│       │   │   ├── swap.rs
│       │   │   └── close_position.rs
│       │   ├── math.rs             // x*y=k, fee calculations, tick logic
│       │   └── errors.rs           // Custom error types
│       └── Cargo.toml
├── tests/
│   └── concentrated_liquidity.ts    // Anchor integration tests
├── AGENTS.md                        // Agent behavior & interaction model
└── PROJECT.md                       // This file
```