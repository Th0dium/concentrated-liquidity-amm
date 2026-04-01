# Learning Log

## 2026-04-02

Studied:
- Anchor project scaffold and how `lib.rs` routes instructions into per-file handlers.
- Why `PoolState` and `Position` should be separate accounts, and why vault balances should live in SPL token accounts instead of duplicated reserve fields in pool state.
- Why position PDAs need a `position_id` and why the pool tracks `next_position_id` to support many positions per owner.
- How `initialize_pool` creates the protocol boundary: pool PDA, vault PDAs, fee config, and initial tick state.
- How `create_position` flows through account validation -> position creation -> SPL Token CPIs -> pool liquidity update.

Key files:
- [Project.md](C:/Dev/concentrated_liquidity/Project.md)
- [AGENTS.md](C:/Dev/concentrated_liquidity/AGENTS.md)
- [lib.rs](C:/Dev/concentrated_liquidity/programs/concentrated_liquidity/src/lib.rs)
- [state.rs](C:/Dev/concentrated_liquidity/programs/concentrated_liquidity/src/state.rs)
- [initialize_pool.rs](C:/Dev/concentrated_liquidity/programs/concentrated_liquidity/src/instructions/initialize_pool.rs)
- [create_position.rs](C:/Dev/concentrated_liquidity/programs/concentrated_liquidity/src/instructions/create_position.rs)

Open questions:
- How should tick-level aggregate liquidity be stored: inline sparse map, separate tick PDAs, or tick arrays?
- What liquidity formula should replace the current placeholder in `create_position`?
- How should swap fee growth be tracked so positions can claim fees without iterating every position on each swap?
