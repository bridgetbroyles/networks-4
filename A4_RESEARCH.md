# A4 Design Research and Evidence

This file records why the competitive program differs from a typical static
starter or one-pass generated solution. The game simulator remains the final
authority: published networking methods were adapted only where its API and
measured utility support them.

## Routing policy

The program retains Gao-Rexford customer/peer/provider preferences and
valley-free export. Gao and Rexford show that these commercial-policy
constraints provide stable routing without global coordination:

- [Stable Internet Routing Without Global Coordination](https://www.cs.princeton.edu/courses/archive/fall14/cos561/papers/StableBGP01.pdf)

RFC 4271 says MED compares entry points offered by the same neighbouring AS.
The implementation uses MED only after internal distance, because the A4
landslide benchmark showed that MED-first routing overused the crossing.
Hot-potato/MED-tie-break routing improved failure utility by `$9.259266` with
no normal-round regression:

- [RFC 4271, BGP-4](https://www.rfc-editor.org/rfc/rfc4271.html)
- [Geographic Properties of Internet Routing](https://static.usenix.org/publications/library/proceedings/usenix02/subramanian.html)

## Failure detection and convergence

RFC 5880's BFD model derives failure time from a transmission interval and a
detection multiplier. A4 sends a 20-byte hello every 100 ms and declares an
internal link down after three missed intervals (300 ms), with a 500 ms startup
grace period:

- [RFC 5880, Bidirectional Forwarding Detection](https://www.rfc-editor.org/rfc/rfc5880.html)

Route replacement is make-before-break: a higher-priority new entry is
installed before the obsolete entry is deleted. This avoids the transient
default-route hole created by generated delete-then-install code. The approach
matches the graceful-shutdown goal of installing alternate paths before
removing nominal ones:

- [RFC 6198, Graceful Shutdown of BGP Sessions](https://www.rfc-editor.org/rfc/rfc6198.html)

Signed backup routes act like precomputed fast-reroute alternates. RFC 5286
similarly emphasizes precalculated, loop-free next hops to reduce loss during
convergence:

- [RFC 5286, IP Fast Reroute](https://www.rfc-editor.org/rfc/rfc5286.html)

## Queueing

Strict priority is appropriate here because Calls and Trading are low-volume
but deadline-sensitive. The implementation prevents a common starvation and
buffer-partitioning mistake by:

- reserving finite queue capacity for every class;
- enabling priority only when a signed low-latency path makes deadlines
  attainable;
- configuring only active borders and the slow crossing;
- leaving high-speed access and application ports in their original FIFO mode.
- returning prioritized bottlenecks to one full FIFO queue when an interval
  reports eight or more drops.

Deficit round robin was considered. It provides efficient fairness, but its
basic form does not itself provide the same latency preference; the A4 classes
have equal packet sizes and explicit deadline/value ordering, so strict
priority is the better fit:

- [Efficient Fair Queuing Using Deficit Round-Robin](https://web.stanford.edu/class/ee384x/EE384X/papers/DRR.pdf)

## Rejected experiments

These results are intentionally documented so they are not reintroduced:

| Experiment | Normal delta | Landslide delta | Decision |
|---|---:|---:|---|
| Send only Calls/Trading through Tier 1 after a split | `$0.00` | `-$12.04` | Reject: deadlines still missed, fees rise. |
| Freeze all external routes during a split | `$0.00` | `-$91.35` | Reject: reachable exits recover substantial value. |
| MED-first to hot-potato/MED-tie-break | `$0.00` | `+$9.26` | Keep hot-potato. |
| Queue every port vs only bottlenecks | `$0.00` | `$0.00` in practice | Keep bottleneck-only: same utility, less disturbance and state. |
| Three vs roughly four missed hellos | `$0.00` | `$0.00` in practice | Keep three: research-backed faster detection, no regression. |
| Static priority at 2× load | `-$191.24` | `-$191.24` | Add feedback: adaptive FIFO recovered `$191.16`. |

The benchmark command is `scripts/benchmark_a4.sh`. It creates temporary
agreement worlds, compares two WASM files, and removes all generated worlds on
exit. `LOAD_SCALE`, `WITH_BACKUP`, `SKIP_PEER3`, and `SEEDS` select stress and
agreement cases.

## Final comparison with the starter

| Scenario | Starter | Competitive | Delta |
|---|---:|---:|---:|
| Three peers, normal | `1824.00` | `1825.08` | `+$1.08` |
| Three peers, landslide | `1561.32` | `1562.40` | `+$1.08` |
| Backup-only neighbour, normal | `1260.42` | `1262.00` | `+$1.58` |
| Backup-only neighbour, landslide | `1018.81` | `1020.43` | `+$1.62` |
| Three peers, 2× load, normal | `3041.08` | `3041.00` | `-$0.08` |
| Three peers, 2× load, landslide | `2601.01` | `2600.93` | `-$0.08` |
| Full 32-AS world, Tier-1 only | `1.20345` | `1.20345` | `$0.00` |

The 2× result includes one control interval before overload fallback; the
remaining `$0.08` cost is the price of detecting rather than assuming overload.
