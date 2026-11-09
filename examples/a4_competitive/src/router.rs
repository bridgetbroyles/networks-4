//! Helpers for an A4 switch program: turn the A4 init information into a
//! forwarding table for one switch, given how to rank offered routes.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use switch_program_sdk::wire::{A4InfoW, A4RouteW};
use switch_program_sdk::*;

pub const T_ROUTE: u32 = 1;
pub const T_CLASS: u32 = 2;
pub const T_PROTO: u32 = 3;
pub const PROTO_HELLO: u8 = 89;
pub const CTRL_IP: u32 = 0xe000_0005;

/// The port at `me` toward `to` inside the AS, avoiding `down` links
/// (given as (switch, port) at either end).
pub fn next_hop(info: &A4InfoW, me: u32, to: u32, down: &BTreeSet<(u32, u16)>) -> Option<u16> {
    if me == to {
        return None;
    }
    // BFS from `to` back to `me`; the first hop is the neighbour of `me`
    // that is one step closer.
    let mut dist: BTreeMap<u32, u32> = BTreeMap::from([(to, 0)]);
    let mut q = VecDeque::from([to]);
    while let Some(s) = q.pop_front() {
        for l in &info.internal {
            if down.contains(&(l.a, l.a_port)) || down.contains(&(l.b, l.b_port)) {
                continue;
            }
            let n = if l.a == s {
                l.b
            } else if l.b == s {
                l.a
            } else {
                continue;
            };
            if !dist.contains_key(&n) {
                dist.insert(n, dist[&s] + 1);
                q.push_back(n);
            }
        }
    }
    let mine = *dist.get(&me)?;
    info.internal
        .iter()
        .filter(|l| !down.contains(&(l.a, l.a_port)) && !down.contains(&(l.b, l.b_port)))
        .filter_map(|l| {
            if l.a == me {
                Some((l.b, l.a_port))
            } else if l.b == me {
                Some((l.a, l.b_port))
            } else {
                None
            }
        })
        .filter(|(n, _)| dist.get(n).is_some_and(|d| *d + 1 == mine))
        .map(|(_, p)| p)
        .min()
}

/// Hop distance inside the AS from `me` to `to`, or None.
pub fn inside(info: &A4InfoW, me: u32, to: u32, down: &BTreeSet<(u32, u16)>) -> Option<u32> {
    let mut cur = me;
    let mut n = 0;
    while cur != to {
        let p = next_hop(info, cur, to, down)?;
        let l = info
            .internal
            .iter()
            .find(|l| (l.a == cur && l.a_port == p) || (l.b == cur && l.b_port == p))?;
        cur = if l.a == cur { l.b } else { l.a };
        n += 1;
        if n > 64 {
            return None;
        }
    }
    Some(n)
}

/// The forwarding table at `me`: (addr, len, port) for every own prefix
/// and every prefix some route offers. `rank` orders a prefix's offered
/// routes (lower first) and may refuse one with None; the first that can
/// be reached inside the AS wins.
pub fn table(
    info: &A4InfoW,
    me: u32,
    down: &BTreeSet<(u32, u16)>,
    rank: impl Fn(&A4RouteW, u32) -> Option<u64>,
) -> Vec<(u32, u8, u16)> {
    let mut out = Vec::new();
    let no_failed_links = BTreeSet::new();
    for p in &info.own {
        if p.switch == me {
            out.push((p.addr, p.len, p.port));
        } else if let Some(port) = next_hop(info, me, p.switch, down) {
            out.push((p.addr, p.len, port));
        } else {
            // A split AS may have a signed backup route to this own prefix.
            // Leave the prefix open for the route loop below only when such
            // a backup exit is actually reachable from this half.
            let reachable_backup = info.routes.iter().any(|r| {
                r.addr == p.addr
                    && r.len == p.len
                    && r.rel == wire::A4_BACKUP
                    && !down.contains(&(r.switch, r.port))
                    && inside(info, me, r.switch, down).is_some()
            });
            if !reachable_backup {
                // Falling through to 0/0 would send every class through the
                // Tier-1. Web/Video costs more than it earns there, while
                // Calls/Trading cannot meet their deadlines on that long
                // path. Keep the specific route aimed at the failed crossing
                // so the packet is dropped; useful fallback should be bought
                // explicitly as a low-latency backup agreement.
                if let Some(port) = next_hop(info, me, p.switch, &no_failed_links) {
                    out.push((p.addr, p.len, port));
                }
            }
        }
    }
    let mut by_prefix: BTreeMap<(u32, u8), Vec<&A4RouteW>> = BTreeMap::new();
    for r in &info.routes {
        if down.contains(&(r.switch, r.port)) {
            continue;
        }
        by_prefix.entry((r.addr, r.len)).or_default().push(r);
    }
    for ((addr, len), rs) in by_prefix {
        if out.iter().any(|&(a, l, _)| a == addr && l == len) {
            continue;
        }
        let mut ranked: Vec<(u64, u32, u16, &A4RouteW)> = rs
            .iter()
            .copied()
            .filter_map(|r| {
                let d = inside(info, me, r.switch, down)?;
                Some((rank(r, d)?, r.switch, r.port, r))
            })
            .collect();
        ranked.sort_by_key(|(k, s, p, _)| (*k, *s, *p));
        if let Some(&(_, s, p, _)) = ranked.first() {
            let port = if s == me {
                p
            } else {
                next_hop(info, me, s, down).unwrap()
            };
            out.push((addr, len, port));
        } else {
            // Every specific exit is on the disconnected half. Preserve the
            // best original specific route toward the dead crossing instead
            // of deleting it and accidentally letting traffic fall through
            // to the paid Tier-1 default route.
            let mut original: Vec<(u64, u32, u16)> = rs
                .iter()
                .filter_map(|r| {
                    let d = inside(info, me, r.switch, &no_failed_links)?;
                    Some((rank(r, d)?, r.switch, r.port))
                })
                .collect();
            original.sort_unstable();
            if let Some(&(_, s, p)) = original.first() {
                let port = if s == me {
                    p
                } else {
                    next_hop(info, me, s, &no_failed_links).unwrap()
                };
                out.push((addr, len, port));
            }
        }
    }
    out
}

/// The usual pipeline: control packets punted by protocol, then a route by
/// destination, then a queue by class.
pub fn setup(routes: &[(u32, u8, u16)], prio: bool) -> ProgramSetup {
    let mut setup = ProgramSetup::new();
    setup.declare_table(T_PROTO, MatchKind::Exact, 8);
    setup.declare_table(T_ROUTE, MatchKind::Lpm, 4096);
    setup.declare_table(T_CLASS, MatchKind::Exact, 8);
    setup.set_program(
        text::parse_tiny_program(
            "
            stage alu=4 mem=1
                load    r0, ip_proto
                table   t3, r0 -> m0
            stage alu=4 mem=1
                load    r1, ip_dst
                table   t1, r1 -> m1
            stage alu=4 mem=1
                load    r2, ip_dscp
                table   t2, r2 -> m2
            ",
        )
        .unwrap(),
    );
    setup.add_entry(
        T_PROTO,
        TableEntry {
            id: wire::EntryIdW(1),
            key: PROTO_HELLO as u64,
            prefix_len: 8,
            priority: 0,
            action: TableAction::Punt {
                reason: PuntReason::Custom(PROTO_HELLO as u32),
            },
        },
    );
    for (i, &(addr, len, port)) in routes.iter().enumerate() {
        setup.add_entry(T_ROUTE, route_entry(1000 + i as u64, addr, len, port, 0));
    }
    if prio {
        for entry in priority_entries() {
            setup.add_entry(T_CLASS, entry);
        }
    }
    setup
}

/// Class-table entries for strict priority. They are also installable at
/// runtime, after the controller observes that a real queue is dropping.
pub fn priority_entries() -> Vec<TableEntry> {
    let mut entries = vec![
        TableEntry {
            id: wire::EntryIdW(10),
            key: wire::A4_CALLS as u64,
            prefix_len: 8,
            priority: 0,
            action: TableAction::SetQueue {
                queue: wire::QueueIdW(0),
            },
        },
        TableEntry {
            id: wire::EntryIdW(11),
            key: wire::A4_TRADING as u64,
            prefix_len: 8,
            priority: 0,
            action: TableAction::SetQueue {
                queue: wire::QueueIdW(1),
            },
        },
    ];
    for (i, class) in [0u64, 1, 2].iter().enumerate() {
        entries.push(TableEntry {
            id: wire::EntryIdW(20 + i as u64),
            key: *class,
            prefix_len: 8,
            priority: 0,
            action: TableAction::SetQueue {
                queue: wire::QueueIdW(2),
            },
        });
    }
    entries
}

pub fn route_entry(id: u64, addr: u32, len: u8, port: u16, priority: i32) -> TableEntry {
    TableEntry {
        id: wire::EntryIdW(id),
        key: addr as u64,
        prefix_len: len,
        priority,
        action: TableAction::SetEgress {
            port: wire::PortIdW(port),
        },
    }
}

/// Replace the installed table `old` (with entry ids) by `new`.
pub fn reinstall(
    old: &mut Vec<(u64, u32, u8, u16, i32)>,
    new: &[(u32, u8, u16)],
    next_id: &mut u64,
    next_priority: &mut i32,
) -> Vec<Action> {
    let mut out = Vec::new();
    let want: BTreeMap<(u32, u8), u16> = new.iter().map(|&(a, l, p)| ((a, l), p)).collect();
    let mut keep = Vec::new();
    let mut obsolete = Vec::new();
    for &(id, a, l, p, priority) in old.iter() {
        if want.get(&(a, l)) == Some(&p) {
            keep.push((id, a, l, p, priority));
        } else {
            obsolete.push(id);
        }
    }
    for (&(a, l), &p) in &want {
        if !keep.iter().any(|&(_, ka, kl, _, _)| ka == a && kl == l) {
            *next_id += 1;
            *next_priority = next_priority.saturating_add(1);
            out.push(actions::install_entry(
                T_ROUTE,
                route_entry(*next_id, a, l, p, *next_priority),
            ));
            keep.push((*next_id, a, l, p, *next_priority));
        }
    }
    // Install higher-priority replacements before deleting old entries so
    // packets never see a temporary hole in the longest-prefix route table.
    out.extend(
        obsolete
            .into_iter()
            .map(|id| actions::delete_entry(T_ROUTE, id)),
    );
    *old = keep;
    out
}

/// This switch's internal ports: the ones whose link stays inside the AS.
pub fn internal_ports(info: &A4InfoW, me: u32) -> Vec<u16> {
    let mut v: Vec<u16> = info
        .internal
        .iter()
        .filter_map(|l| {
            if l.a == me {
                Some(l.a_port)
            } else if l.b == me {
                Some(l.b_port)
            } else {
                None
            }
        })
        .collect();
    v.sort_unstable();
    v
}

/// Ports that leave the AS or reach an app: every port that is not internal.
pub fn edge_ports(info: &A4InfoW, me: u32) -> Vec<u16> {
    let mut v: Vec<u16> = info
        .borders
        .iter()
        .filter(|b| b.switch == me)
        .map(|b| b.port)
        .collect();
    v.extend(info.own.iter().filter(|p| p.switch == me).map(|p| p.port));
    v.sort_unstable();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreachable_specific_route_does_not_fall_through_to_tier1() {
        let destination = 0x0a02_0000;
        let info = A4InfoW {
            switches: vec![1, 2],
            internal: vec![wire::A4LinkW {
                a: 1,
                a_port: 10,
                b: 2,
                b_port: 20,
            }],
            routes: vec![
                wire::A4RouteW {
                    addr: destination,
                    len: 17,
                    switch: 2,
                    port: 30,
                    rel: wire::A4_PEER,
                    as_path: vec![2],
                    med: 1,
                },
                wire::A4RouteW {
                    addr: 0,
                    len: 0,
                    switch: 1,
                    port: 40,
                    rel: wire::A4_TIER1,
                    as_path: vec![100],
                    med: 0,
                },
            ],
            ..Default::default()
        };
        let routes = table(&info, 1, &BTreeSet::from([(1, 10)]), |route, hops| {
            Some(u64::from(route.med) * 1_000 + u64::from(hops))
        });

        assert!(routes.contains(&(destination, 17, 10)));
        assert!(routes.contains(&(0, 0, 40)));
    }

    #[test]
    fn route_replacement_is_make_before_break() {
        let mut installed = vec![(1, 0x0a00_0000, 24, 10, 0)];
        let mut next_id = 1;
        let mut next_priority = 0;
        let actions = reinstall(
            &mut installed,
            &[(0x0a00_0000, 24, 20)],
            &mut next_id,
            &mut next_priority,
        );

        let Action::InstallTableEntry { entry, .. } = &actions[0] else {
            panic!("replacement must be installed first");
        };
        assert_eq!(entry.priority, 1);
        assert!(matches!(
            &entry.action,
            TableAction::SetEgress { port } if port.0 == 20
        ));
        assert!(matches!(
            &actions[1],
            Action::DeleteTableEntry { entry_id, .. } if entry_id.0 == 1
        ));
    }
}
