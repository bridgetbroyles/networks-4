# Project Status

## Objective

- Build and validate a competitive A4 switch program without changing the
  supplied starter.

## Completed

- Added `examples/a4_competitive` with hot-potato/MED-tie-break exits, safe
  routing policy, bottleneck-only priority queues, three-hello failure
  detection, overload-to-FIFO feedback, make-before-break route updates, and
  signed-backup support.
- Added unit tests and a user-facing implementation README.
- Benchmarked normal, peering, full-ring, and landslide scenarios.
- Added `A4_PLAYBOOK.md` with economic, agreement, hardware, and data strategy.
- Added `A4_RESEARCH.md` and `scripts/benchmark_a4.sh` with primary-source
  rationale, rejected experiments, stress cases, and reproducible comparisons.
- Prepared `submission/switch.wasm` for upload.

## In Progress

- None in local code.

## Remaining

- Re-run optimization against the final website rules and actual world state.
- Evaluate real agreement terms, auction prices, statements, and replays after
  they become available.

## Blockers

- `rules.json` is provisional until the game opens.
- Seat maps, counterpart offers, live bids, and graded-round measurements are
  not present in this repository.

## Important Decisions

- Preserve Gao-Rexford policy as the safe default.
- Enable priority queues only after a signed non-Tier-1 path exists; this avoids
  harming utility when urgent deadlines are impossible through Tier 1.
- Never use Tier 1 as an accidental crossing-failure backup for fee-negative
  bulk traffic; require an explicit backup agreement.
- Do not automate or script the game website.

## Verification

- `cargo test --manifest-path examples/a4_competitive/Cargo.toml`
- `cargo test --all` (123 core tests plus integration suites passed)
- Release WebAssembly build for `wasm32-unknown-unknown`
- Simulator comparisons on `practice.toml`, `full.toml`, and the published
  landslide schedule
- Replay inspection of route deletion, installation, and recovery
- `cargo clippy` was not run because the installed Rust toolchain does not
  include the optional Clippy component.

## Next Steps

1. Replace the provisional rules with the final website copy.
2. Collect the per-world inputs listed in `A4_PLAYBOOK.md`.
3. Simulate candidate deals and hardware before making human-entered decisions.

