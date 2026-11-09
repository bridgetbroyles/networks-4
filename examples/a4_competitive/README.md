# Competitive A4 Switch

This is the team's A4 switch program. It is separate from `a4_starter`, so
the supplied baseline remains available for comparisons.

## What it does

- Uses the safe Gao-Rexford routing policy: customer, peer, provider, then
  Tier 1; it exports only valley-free routes.
- Uses hot-potato exit selection to minimize load and failure exposure inside
  this AS; a direct neighbour's MED breaks ties between equally near exits.
- Leaves the starter's single queue unchanged when Tier 1 is the only usable
  path. A signed peer, customer, provider, or backup path enables strict
  priority queues on active borders and the slow crossing: Calls, then
  Trading, then Web/Video. High-speed access/application ports keep FIFO.
- If a prioritized bottleneck drops eight or more packets in one 100 ms
  control interval, it returns to one full FIFO queue. This preserves the
  deadline benefit at normal load without collapsing bulk value under overload.
- Sends a 20-byte hello every 100 ms on internal links. After three missed
  intervals (300 ms) of silence (and a 500 ms startup grace period), it marks
  the link down and rebuilds the route table. Routes are restored when hellos
  return.
- Installs a higher-priority replacement route before removing the old entry,
  avoiding a temporary forwarding hole during convergence.
- Uses a signed backup agreement to reach the other half of this AS after a
  crossing failure. Without a backup, it keeps that traffic off the paid
  Tier-1 default route because home Web/Video earns $1/Mb while Tier 1 costs
  $1.50/Mb.
- Reads the real switch buffer size, so the queue split automatically scales
  when a buffer upgrade is installed.

## Build

```sh
cd examples/a4_competitive
cargo build --release --target wasm32-unknown-unknown
```

The upload file is:

```text
target/wasm32-unknown-unknown/release/a4_competitive.wasm
```

The repository also keeps a ready-to-upload copy at `submission/switch.wasm`.

## Test locally

```sh
cargo test --manifest-path examples/a4_competitive/Cargo.toml

./target/release/competitive_net_sim run-a4 worlds/practice/full.toml \
  --program examples/a4_competitive/target/wasm32-unknown-unknown/release/a4_competitive.wasm \
  --as 1 --json /tmp/a4-statement.json
```

For an honest comparison, run the same world and seed once with the starter
and once with this program. Utility, not packet delivery alone, is the target.

