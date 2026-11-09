# A4 Competitive Playbook

## Current program

The implementation is in `examples/a4_competitive`. The built upload is
`submission/switch.wasm`.

The program makes four decisions:

1. **Routing policy:** safe Gao-Rexford ranking and valley-free exports avoid
   becoming free transit between other networks.
2. **Exit selection:** use the closest reachable exit inside our AS, reducing
   crossing load and exposure; use a direct neighbour's MED to break ties.
3. **Queueing:** once we have a signed non-Tier-1 path, strict priority on
   active borders and the slow crossing serves Calls, then Trading, then
   Web/Video. High-speed access ports remain FIFO. With only Tier 1, the
   original single queue is safer because propagation delay already defeats
   urgent deadlines. Under sustained overload, observed drop feedback returns
   prioritized bottlenecks to a full FIFO queue.
4. **Failure response:** hello packets detect an internal failure, the route
   table is rebuilt, and a signed backup route can carry traffic around a
   failed crossing.

## Verified behavior

- The WebAssembly build succeeds and all program unit tests pass.
- With no agreements, the final program exactly matches the starter in both
  supplied normal practice topologies. It does not trade away utility merely
  to look more sophisticated.
- In the three-peer benchmark, the program improved normal utility by about
  $1 (`1825.08` versus `1824.00`). Hot-potato exit selection also improved the
  published landslide result to `1562.40`, about $1 above the starter and
  $9.26 above the earlier MED-first version.
- Replay inspection shows three-missed-hello failure detection, route-table
  replacement during the outage, and restoration after the link returns.
- A true backup-only-neighbour benchmark beat the starter by about $1.58
  normally and $1.62 during the landslide. A real backup fee must still be
  smaller than its measured recovered value.
- At 2× traffic, static priority lost about $191 versus FIFO. Adaptive overload
  fallback recovered $191.16 while retaining the normal-load priority gain.
- The starter directory was not modified.

These are practice results, not a guarantee for a graded world. Traffic seeds,
agreements, seat position, hardware, failures, and settlement terms change the
best decision.

## Economic priorities

The objective is utility, not throughput:

```text
utility = customer value + settlements received
          - settlements paid - Tier-1 fees
```

- Home Calls are extremely valuable at $35/Mb and have an 80 ms target.
- Trading has a 30 ms target and affects the Jane Street contract, but home
  Trading pays nothing.
- Home Web and Video pay $1/Mb, less than the $1.50/Mb Tier-1 charge. Sending
  one more Mb through Tier 1 can therefore reduce utility unless it preserves
  enough company-contract value.
- Peering can save Tier-1 fees and reduce delay, but only if it attracts useful
  traffic and its settlement terms are acceptable.
- A technically successful failover can still be economically wrong. Always
  compare the extra customer/contract value with backup and transit costs.

## Agreement plan

1. Secure at least one settlement-free peer in each world before optimizing
   hardware around that path.
2. Prefer peers that cover destinations with high Calls/Trading value or large
   Video volume and that are physically close to useful IXPs.
3. Keep Gao-Rexford unless a proposed exception has been simulated; unsafe
   export can make us carry two other teams' traffic without compensation.
4. Price backup service from the measured value recovered during failures. Do
   not pay a flat fee greater than the expected avoided loss.
5. Use custom settlement code only after the actual commercial terms are
   known. The supplied peering, paid-peering, transit, and backup templates are
   safer starting points than an invented contract.

## Hardware plan

- **Cross-ring link:** highest upside when a far-IXP agreement is already
  available. Microwave is especially valuable for the 30 ms Trading target;
  an unpaired link earns nothing.
- **Private interconnect:** cheap and useful for a known high-volume partner,
  but requires an agreement and a compatible location.
- **Second crossing cable:** buy when replays show crossing congestion. It adds
  capacity but shares the crossing's failure risk.
- **Repair crew:** useful when outage duration is the measured problem. It does
  not prevent a failure.
- **Buffer upgrade:** buy only when replay/statement evidence shows local queue
  loss. The program will automatically use the larger buffer.
- **Program memory:** the current program is far below the base allowance, so
  extra memory has no operational value at present.

Do not bid from reserve prices alone. Opportunity cost matters: credits spent
on marginal hardware cannot be used for a cross-ring link or a later auction.

## What is needed for per-world optimization

Provide these after the game opens or after each round:

1. The final `rules.json` downloaded from the website. The repository copy is
   still marked provisional.
2. Each world's seat map: our AS, sector, slot, pass-mate, possible neighbours,
   AI seats, and available IXPs.
3. Every offered agreement and its exact terms, including direction (buyer and
   seller), price, cap, refunds, and minimum service level.
4. Current credits and the live auction lots, minimum bids, and leading bids.
5. Our full statement JSON, replay `.simlog`, and any failure schedule or
   settlement output from each completed round.
6. Which teams are willing to peer, transit, sell backup, or coordinate a
   cross-ring/private link.

Do not provide credentials. A human must make all game-site clicks; the A4 rule
book allows AI advice but forbids a game API or scripted website control.

With those inputs, the next optimization pass can calculate, per world:

- marginal utility of each agreement;
- whether a cross-ring or private link reaches valuable destinations;
- the maximum rational backup fee and auction bid;
- which routes or queue allocations caused losses;
- a round-specific upload, deal list, and bidding ceiling.

