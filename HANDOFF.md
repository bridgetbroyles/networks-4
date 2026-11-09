# Handoff

## Project Purpose

- A4 is a live network-economics game. One WebAssembly switch program runs on
  every switch in the team's AS; agreements and bids are entered by humans.

## Current Objective

- Maximize utility across four worlds while keeping routing safe and resilient.

## Current State

- `examples/a4_competitive` builds and passes its unit tests.
- The upload artifact is `submission/switch.wasm`.
- The supplied `examples/a4_starter` remains unchanged.
- `A4_PLAYBOOK.md` records the selected strategy and required live inputs.

## Completed Work

- Implemented hot-potato exit selection with MED as a tie-breaker.
- Implemented agreement-aware, bottleneck-only strict-priority queues and
  buffer-upgrade scaling, with measured-drop fallback to full FIFO under
  overload.
- Implemented 100 ms internal hellos, three-missed-hello (300 ms) detection,
  make-before-break table rebuild, signed-backup use, and recovery.
- Kept the supplied Gao-Rexford rank/export policy.
- Tested multiple queue allocations, exit-ranking strategies, fallback
  policies, and failure-update orders; retained only neutral or positive
  changes.

## Remaining Work

1. Obtain final rules and actual per-world topology/seat information.
2. Model concrete agreement offers and auction bids.
3. Analyze each round's statement and replay, then retune if evidence supports
   a change.

## Decisions and Constraints

- Never push or publish changes without the user's explicit permission.
- Never automate the game website; every click must remain human-entered.
- Optimize utility rather than raw delivery.
- Do not activate priority queues for unused physical neighbour links (`rel=0`)
  or Tier-1-only connectivity.

## Known Issues and Blockers

- The repository's `rules.json` is explicitly provisional.
- Practice outcomes vary by topology and traffic seed; no single static result
  proves superiority in graded worlds.
- Backup profitability cannot be selected without real price and failure data.

## Verification Performed

- Ten program unit tests, 123 core repository tests, integration suites, and
  the release WASM build passed.
- No-agreement output exactly matched the starter on both supplied normal
  practice topologies.
- Peering and landslide simulations completed without program failures.
- Replay showed route changes at failure detection and restoration on recovery.
- Clippy was unavailable in the installed toolchain; formatting checks passed.

## Recommended Next Step

- Collect the six inputs under “What is needed for per-world optimization” in
  `A4_PLAYBOOK.md`, then simulate the actual candidate deals before acting.

