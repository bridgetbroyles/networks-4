//! Competitive A4 switch program.
//!
//! It keeps the safe Gao-Rexford routing policy from the starter, but adds
//! the three pieces the starter intentionally omits:
//!
//! - exit selection uses the nearest reachable exit, with a directly
//!   connected neighbour's MED as the tie-breaker;
//! - a signed non-Tier-1 path activates separate strict-priority queues for
//!   Calls, Trading, and bulk traffic, where deadlines can actually be met;
//! - internal links are probed every 100 ms and routes are rebuilt after a
//!   bounded silence, using a signed backup route when the crossing splits
//!   the AS without sending fee-negative bulk traffic through the Tier-1.
mod policy;
mod router;

use std::collections::{BTreeMap, BTreeSet};
use switch_program_sdk::*;

const HELLO_INTERVAL_NS: u64 = 100_000_000;
const DOWN_AFTER_NS: u64 = 3 * HELLO_INTERVAL_NS;
const STARTUP_GRACE_NS: u64 = 500_000_000;
const OVERLOAD_DROPS_PER_INTERVAL: u32 = 8;

pub struct Competitive {
    info: wire::A4InfoW,
    me: u32,
    internal_ports: Vec<u16>,
    queue_ports: Vec<u16>,
    last_heard_ns: BTreeMap<u16, u64>,
    down: BTreeSet<(u32, u16)>,
    installed: Vec<(u64, u32, u8, u16, i32)>,
    next_entry_id: u64,
    next_priority: i32,
    priority_worthwhile: bool,
    queues_configured: bool,
    priority_abandoned: bool,
}

/// Rank two physical exits to the neighbour already chosen by routing
/// policy. Backup routes are usable only when the normal internal path to an
/// own prefix has disappeared. Normal traffic uses hot-potato routing to
/// minimize dependence on our crossing; MED breaks ties between equally near
/// exits to the same directly connected neighbour.
fn exit_rank(route: &wire::A4RouteW, inside_hops: u32) -> Option<u64> {
    if route.rel == wire::A4_BACKUP {
        return Some(0);
    }
    Some(u64::from(inside_hops) * 1_000 + u64::from(route.med))
}

/// Divide one port's real buffer between Calls, Trading, and Web/Video.
/// Keeping the sum at the physical capacity prevents bulk traffic from
/// consuming the slots reserved for the two deadline-sensitive classes.
fn queue_caps(queue_bytes: u64) -> [u32; 3] {
    let packets = (queue_bytes / 1_000).max(3).min(u64::from(u32::MAX)) as u32;
    let calls = (packets / 4).max(1);
    let trading = (packets / 6).max(1);
    let bulk = packets.saturating_sub(calls + trading).max(1);
    [calls, trading, bulk]
}

fn queue_packets(queue_bytes: u64) -> u32 {
    (queue_bytes / 1_000).max(1).min(u64::from(u32::MAX)) as u32
}

fn has_signed_non_tier1_path(info: &wire::A4InfoW) -> bool {
    info.borders.iter().any(|border| {
        matches!(
            border.rel,
            wire::A4_CUSTOMER | wire::A4_PEER | wire::A4_PROVIDER | wire::A4_BACKUP
        )
    })
}

/// Queue only the network bottlenecks: active borders and the slowest of a
/// multi-link switch's internal ports (normally the crossing). Ports left at
/// the default one-queue configuration safely treat every queue id as queue 0.
fn select_queue_ports(
    info: &wire::A4InfoW,
    me: u32,
    local_ports: &[u16],
    capacities_bps: &[u64],
) -> Vec<u16> {
    let capacity: BTreeMap<u16, u64> = local_ports
        .iter()
        .copied()
        .zip(capacities_bps.iter().copied())
        .collect();
    let mut selected: Vec<u16> = info
        .borders
        .iter()
        .filter(|border| border.switch == me && border.rel != 0)
        .map(|border| border.port)
        .collect();
    let internal = router::internal_ports(info, me);
    if capacity.len() != local_ports.len() {
        selected.extend(internal);
    } else if internal.len() > 1 {
        let slowest = internal
            .iter()
            .filter_map(|port| capacity.get(port))
            .min()
            .copied();
        if let Some(slowest) = slowest {
            selected.extend(
                internal
                    .into_iter()
                    .filter(|port| capacity.get(port) == Some(&slowest)),
            );
        }
    }
    selected.sort_unstable();
    selected.dedup();
    selected
}

fn link_timed_out(now_ns: u64, last_heard_ns: u64) -> bool {
    now_ns.saturating_sub(last_heard_ns) >= DOWN_AFTER_NS
}

fn prioritized_port_dropped(ev: &TimerEvent, queue_ports: &[u16]) -> bool {
    ev.port_drops.iter().any(|drops| {
        drops.dropped >= OVERLOAD_DROPS_PER_INTERVAL && queue_ports.contains(&drops.port)
    })
}

impl Competitive {
    fn current_table(&self) -> Vec<(u32, u8, u16)> {
        router::table(&self.info, self.me, &self.down, exit_rank)
    }

    fn reinstall_table(&mut self) -> Vec<Action> {
        let routes = self.current_table();
        router::reinstall(
            &mut self.installed,
            &routes,
            &mut self.next_entry_id,
            &mut self.next_priority,
        )
    }

    fn queue_actions(&self) -> Vec<Action> {
        let [calls, trading, bulk] = queue_caps(self.info.queue_bytes);
        self.queue_ports
            .iter()
            .map(|&port| {
                actions::set_queues(
                    port,
                    Discipline::StrictPriority,
                    vec![
                        QueueSpec::new(calls),
                        QueueSpec::new(trading),
                        QueueSpec::new(bulk),
                    ],
                )
            })
            .collect()
    }

    fn fifo_actions(&self) -> Vec<Action> {
        let packets = queue_packets(self.info.queue_bytes);
        self.queue_ports
            .iter()
            .map(|&port| actions::set_queues(port, Discipline::Fifo, vec![QueueSpec::new(packets)]))
            .collect()
    }
}

impl SwitchProgram for Competitive {
    fn init(_: u32, _: Vec<u16>) -> (Self, ProgramSetup) {
        panic!("a4_competitive requires an A4 world")
    }

    fn init_with(input: InitInput) -> (Self, ProgramSetup) {
        let info = input.a4.clone().expect("a4_competitive requires input.a4");
        let me = input.switch_id;
        let internal_ports = router::internal_ports(&info, me);
        let queue_ports =
            select_queue_ports(&info, me, &input.local_ports, &input.port_capacity_bps);
        let no_failed_links = BTreeSet::new();
        let routes = router::table(&info, me, &no_failed_links, exit_rank);
        let installed = routes
            .iter()
            .enumerate()
            .map(|(i, &(addr, len, port))| (1_000 + i as u64, addr, len, port, 0))
            .collect();
        let next_entry_id = 1_000 + routes.len() as u64;
        let last_heard_ns = internal_ports.iter().map(|&port| (port, 0)).collect();
        let priority_worthwhile = has_signed_non_tier1_path(&info);

        let program = Self {
            info,
            me,
            internal_ports,
            queue_ports,
            last_heard_ns,
            down: BTreeSet::new(),
            installed,
            next_entry_id,
            next_priority: 0,
            priority_worthwhile,
            queues_configured: false,
            priority_abandoned: false,
        };
        (program, router::setup(&routes, false))
    }

    fn on_punt(&mut self, ev: PuntEvent) -> Vec<Action> {
        if ev.ip_proto != router::PROTO_HELLO || !self.internal_ports.contains(&ev.ingress_port) {
            return Vec::new();
        }

        self.last_heard_ns.insert(ev.ingress_port, ev.now_ns);
        if self.down.remove(&(self.me, ev.ingress_port)) {
            let mut actions = vec![actions::link_up(ev.ingress_port)];
            actions.extend(self.reinstall_table());
            actions
        } else {
            Vec::new()
        }
    }

    fn on_timer(&mut self, ev: TimerEvent) -> Vec<Action> {
        let mut out = Vec::new();

        // With only Tier-1 connectivity, the propagation delay already
        // misses the urgent deadlines, and splitting buffers can reduce
        // profitable Video delivery. A direct agreement creates a path on
        // which queueing delay is worth controlling.
        if self.priority_worthwhile && !self.queues_configured && !self.priority_abandoned {
            out.extend(self.queue_actions());
            out.extend(
                router::priority_entries()
                    .into_iter()
                    .map(|entry| actions::install_entry(router::T_CLASS, entry)),
            );
            self.queues_configured = true;
        }

        // Static priority is valuable while the urgent queues are sparse,
        // but sustained offered load can make its smaller bulk queue destroy
        // more Video/Web value than priority recovers. The timer reports real
        // admission drops, so fall back to the full FIFO buffer as soon as a
        // prioritized bottleneck proves overloaded.
        if self.queues_configured && prioritized_port_dropped(&ev, &self.queue_ports) {
            out.extend(self.fifo_actions());
            for id in [10, 11, 20, 21, 22] {
                out.push(actions::delete_entry(router::T_CLASS, id));
            }
            self.queues_configured = false;
            self.priority_abandoned = true;
        }

        // A hello is only 20 bytes, so even a switch with several internal
        // ports remains far below A4's 10,000-byte/s injection limit.
        for &port in &self.internal_ports {
            out.push(actions::in_queue(
                0,
                actions::inject_packet(
                    port,
                    router::CTRL_IP | self.me,
                    router::CTRL_IP,
                    router::PROTO_HELLO,
                    1,
                    Vec::new(),
                ),
            ));
        }

        let mut changed = false;
        if ev.now_ns >= STARTUP_GRACE_NS {
            for &port in &self.internal_ports {
                let heard = self.last_heard_ns.get(&port).copied().unwrap_or(0);
                if link_timed_out(ev.now_ns, heard) && self.down.insert((self.me, port)) {
                    out.push(actions::link_down(port));
                    changed = true;
                }
            }
        }
        if changed {
            out.extend(self.reinstall_table());
        }

        out.push(actions::schedule_timer(HELLO_INTERVAL_NS));
        out
    }
}

switch_program!(Competitive);
switch_program_sdk::policy_program!(policy::rank, policy::announce);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_buffer_reserves_space_for_deadline_traffic() {
        assert_eq!(queue_caps(12_500), [3, 2, 7]);
        assert_eq!(queue_packets(12_500), 12);
    }

    #[test]
    fn buffer_upgrade_is_used_without_hard_coding_it() {
        assert_eq!(queue_caps(25_000), [6, 4, 15]);
    }

    #[test]
    fn unused_physical_links_do_not_enable_priority() {
        let mut info = wire::A4InfoW::default();
        info.borders.push(wire::A4BorderW {
            switch: 1,
            port: 1,
            peer_as: 2,
            rel: 0,
        });
        info.borders.push(wire::A4BorderW {
            switch: 1,
            port: 2,
            peer_as: 100,
            rel: wire::A4_TIER1,
        });
        assert!(!has_signed_non_tier1_path(&info));

        info.borders[0].rel = wire::A4_PEER;
        assert!(has_signed_non_tier1_path(&info));
    }

    #[test]
    fn three_missed_hello_intervals_trigger_failure() {
        assert!(!link_timed_out(DOWN_AFTER_NS - 1, 0));
        assert!(link_timed_out(DOWN_AFTER_NS, 0));
    }

    #[test]
    fn queues_only_active_borders_and_the_slow_crossing() {
        let info = wire::A4InfoW {
            internal: vec![
                wire::A4LinkW {
                    a: 1,
                    a_port: 1,
                    b: 2,
                    b_port: 1,
                },
                wire::A4LinkW {
                    a: 1,
                    a_port: 2,
                    b: 3,
                    b_port: 1,
                },
            ],
            borders: vec![
                wire::A4BorderW {
                    switch: 1,
                    port: 3,
                    peer_as: 100,
                    rel: wire::A4_TIER1,
                },
                wire::A4BorderW {
                    switch: 1,
                    port: 4,
                    peer_as: 4,
                    rel: 0,
                },
            ],
            ..Default::default()
        };

        assert_eq!(
            select_queue_ports(&info, 1, &[1, 2, 3, 4, 5], &[10, 100, 40, 40, 1_000]),
            vec![1, 3]
        );
    }

    #[test]
    fn only_drops_on_prioritized_ports_trigger_fifo_fallback() {
        let event = TimerEvent {
            now_ns: 0,
            port_drops: vec![wire::PortDropsW {
                port: 3,
                dropped: OVERLOAD_DROPS_PER_INTERVAL,
                policy: 0,
                queues: vec![0, 0, 1],
            }],
            counters: Vec::new(),
        };
        assert!(prioritized_port_dropped(&event, &[3]));
        assert!(!prioritized_port_dropped(&event, &[4]));

        let mut small_burst = event;
        small_burst.port_drops[0].dropped = OVERLOAD_DROPS_PER_INTERVAL - 1;
        assert!(!prioritized_port_dropped(&small_burst, &[3]));
    }

    #[test]
    fn med_breaks_ties_between_equally_near_exits() {
        let route = wire::A4RouteW {
            addr: 0,
            len: 24,
            switch: 1,
            port: 1,
            rel: wire::A4_PEER,
            as_path: vec![2],
            med: 2,
        };
        assert!(
            exit_rank(&route, 0)
                < exit_rank(
                    &wire::A4RouteW {
                        med: 3,
                        ..route.clone()
                    },
                    0
                )
        );
    }

    #[test]
    fn nearest_exit_precedes_med() {
        let route = wire::A4RouteW {
            addr: 0,
            len: 24,
            switch: 1,
            port: 1,
            rel: wire::A4_PEER,
            as_path: vec![2],
            med: 5,
        };
        assert!(
            exit_rank(&route, 0)
                < exit_rank(
                    &wire::A4RouteW {
                        med: 1,
                        ..route.clone()
                    },
                    1,
                )
        );
    }
}
