//! Pure, deterministic observability model.
//!
//! This is the simulation half of Phase A7. Its single job is to make the
//! input/simulation boundary concrete and testable:
//!
//! - [`LaunchManifest`] is the *only* input that determines the event stream.
//!   It models the "recorded launch manifest" the roadmap calls out: anything
//!   that may legitimately change deterministic state lives here.
//! - [`simulate`] turns a manifest into a fixed-length stream of semantic
//!   [`TracedEvent`]s drawn from the same vocabulary the real game emits
//!   (doors, observation, interaction, match rounds, pickups, reroutes).
//! - Debug knobs (fog, bloom, overlays, color-vision preview, trace verbosity)
//!   are deliberately **not** parameters here, so it is structurally impossible
//!   for a knob to perturb the simulation. The ECS layer enforces the same rule
//!   live, and the lab tests prove it.

/// Number of fixed ticks in one looped simulation run.
pub const TICKS_PER_RUN: u32 = 48;
/// Maximum random events emitted on a single tick (excludes the scripted round
/// cadence event, so the true per-tick maximum is `MAX_EVENTS_PER_TICK + 1`).
pub const MAX_EVENTS_PER_TICK: usize = 2;
/// Number of rooms in the model facility (matches the 8-room discovery layout).
pub const ROOM_COUNT: u8 = 8;
/// Ticks between scripted match-round advances.
pub const TICKS_PER_ROUND: u32 = 12;

/// The deterministic launch manifest: the recorded inputs that fully determine
/// a run. A config knob only influences the simulation once it is *committed*
/// into a manifest (see the lab's `Enter` action); until then it is pending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaunchManifest {
    /// Seed for the deterministic event generator.
    pub seed: u32,
}

impl LaunchManifest {
    /// Builds a manifest for `seed`.
    pub const fn new(seed: u32) -> Self {
        Self { seed }
    }
}

/// The default launch seed used at startup and on scene reset.
pub const DEFAULT_SEED: u32 = 1975;

/// A coarse semantic class for an event, used for trace filtering, overlay
/// colour selection, and verbosity gating. Kept free of any presentation type
/// so the model stays render-agnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// A door opened or slammed.
    Door,
    /// A room was observed (frozen).
    Observation,
    /// A control station was operated.
    Interaction,
    /// A competitive match round advanced.
    Round,
    /// A keystone pickup was collected.
    Pickup,
    /// An unobserved corridor rerouted.
    Reroute,
}

impl EventKind {
    /// Every kind, in a stable order.
    pub const ALL: [EventKind; 6] = [
        EventKind::Door,
        EventKind::Observation,
        EventKind::Interaction,
        EventKind::Round,
        EventKind::Pickup,
        EventKind::Reroute,
    ];

    /// A short label for overlays and traces.
    pub fn label(self) -> &'static str {
        match self {
            EventKind::Door => "door",
            EventKind::Observation => "observe",
            EventKind::Interaction => "interact",
            EventKind::Round => "round",
            EventKind::Pickup => "pickup",
            EventKind::Reroute => "reroute",
        }
    }
}

/// One semantic event the facility can emit, with its concrete subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TracedEvent {
    /// A door in `room` opened, freeing traversal.
    DoorOpened { room: u8 },
    /// A door in `room` slammed shut.
    DoorSlammed { room: u8 },
    /// `room` became observed and froze.
    RoomObserved { room: u8 },
    /// A control `station` was operated.
    Interacted { station: u8 },
    /// The match advanced to `round`.
    RoundAdvanced { round: u8 },
    /// A `keystone` pickup was collected.
    PickupCollected { keystone: u8 },
    /// An unobserved corridor rerouted `from` one room `to` another.
    Rerouted { from: u8, to: u8 },
}

impl TracedEvent {
    /// The coarse class of this event.
    pub fn kind(self) -> EventKind {
        match self {
            TracedEvent::DoorOpened { .. } | TracedEvent::DoorSlammed { .. } => EventKind::Door,
            TracedEvent::RoomObserved { .. } => EventKind::Observation,
            TracedEvent::Interacted { .. } => EventKind::Interaction,
            TracedEvent::RoundAdvanced { .. } => EventKind::Round,
            TracedEvent::PickupCollected { .. } => EventKind::Pickup,
            TracedEvent::Rerouted { .. } => EventKind::Reroute,
        }
    }

    /// The room this event highlights, if any (used to light the map tiles).
    pub fn room(self) -> Option<u8> {
        match self {
            TracedEvent::DoorOpened { room }
            | TracedEvent::DoorSlammed { room }
            | TracedEvent::RoomObserved { room }
            | TracedEvent::Rerouted { from: room, .. } => Some(room),
            _ => None,
        }
    }

    /// A compact one-line description for the trace overlay and the log line.
    pub fn summary(self) -> String {
        match self {
            TracedEvent::DoorOpened { room } => format!("door opened   | room {room}"),
            TracedEvent::DoorSlammed { room } => format!("door slammed  | room {room}"),
            TracedEvent::RoomObserved { room } => format!("room observed | room {room}"),
            TracedEvent::Interacted { station } => format!("interacted    | station {station}"),
            TracedEvent::RoundAdvanced { round } => format!("round advance | round {round}"),
            TracedEvent::PickupCollected { keystone } => {
                format!("pickup        | keystone {keystone}")
            }
            TracedEvent::Rerouted { from, to } => format!("reroute       | {from} -> {to}"),
        }
    }
}

/// A scheduled event at a fixed tick within a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickEvent {
    /// The tick this event fires on (`0..TICKS_PER_RUN`).
    pub tick: u32,
    /// The event itself.
    pub event: TracedEvent,
}

/// SplitMix64 step — a tiny, dependency-free deterministic generator.
fn split_mix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Decodes one random event from a 64-bit draw.
fn decode_event(r: u64) -> TracedEvent {
    let room = (r % ROOM_COUNT as u64) as u8;
    match (r >> 8) % 6 {
        0 => TracedEvent::DoorOpened { room },
        1 => TracedEvent::DoorSlammed { room },
        2 => TracedEvent::RoomObserved { room },
        3 => TracedEvent::Interacted {
            station: ((r >> 16) % 5) as u8,
        },
        4 => TracedEvent::PickupCollected {
            keystone: ((r >> 16) % 3) as u8,
        },
        _ => {
            let step = 1 + (r >> 24) % (ROOM_COUNT as u64 - 1);
            let to = ((room as u64 + step) % ROOM_COUNT as u64) as u8;
            TracedEvent::Rerouted { from: room, to }
        }
    }
}

/// Produces the deterministic event stream for `manifest`.
///
/// The output depends only on `manifest.seed`. Calling this twice with the same
/// manifest yields byte-identical streams; that is the property the lab leans
/// on for replay-grade determinism.
pub fn simulate(manifest: LaunchManifest) -> Vec<TickEvent> {
    let mut state =
        (manifest.seed as u64).wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0xA076_1D64_78BD_642F;
    let mut out = Vec::new();
    for tick in 0..TICKS_PER_RUN {
        if tick > 0 && tick % TICKS_PER_ROUND == 0 {
            out.push(TickEvent {
                tick,
                event: TracedEvent::RoundAdvanced {
                    round: (tick / TICKS_PER_ROUND) as u8,
                },
            });
        }
        let count = (split_mix(&mut state) % (MAX_EVENTS_PER_TICK as u64 + 1)) as usize;
        for _ in 0..count {
            let draw = split_mix(&mut state);
            out.push(TickEvent {
                tick,
                event: decode_event(draw),
            });
        }
    }
    out
}

/// A 64-bit FNV-1a checksum over an event stream. Used to assert that the live
/// ECS simulation reproduces the pure model exactly, and to detect any drift.
pub fn checksum(events: &[TickEvent]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut feed = |byte: u8| {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for ev in events {
        for byte in ev.tick.to_le_bytes() {
            feed(byte);
        }
        // A stable discriminant + payload encoding, independent of memory layout.
        let (tag, a, b): (u8, u16, u16) = match ev.event {
            TracedEvent::DoorOpened { room } => (1, room as u16, 0),
            TracedEvent::DoorSlammed { room } => (2, room as u16, 0),
            TracedEvent::RoomObserved { room } => (3, room as u16, 0),
            TracedEvent::Interacted { station } => (4, station as u16, 0),
            TracedEvent::RoundAdvanced { round } => (5, round as u16, 0),
            TracedEvent::PickupCollected { keystone } => (6, keystone as u16, 0),
            TracedEvent::Rerouted { from, to } => (7, from as u16, to as u16),
        };
        feed(tag);
        for byte in a.to_le_bytes() {
            feed(byte);
        }
        for byte in b.to_le_bytes() {
            feed(byte);
        }
    }
    hash
}

/// Whether an event of `kind` is logged at trace `verbosity`.
///
/// - `0` — tracing off; nothing is logged.
/// - `1` — key events only (the dramatic, low-frequency ones).
/// - `2+` — everything.
///
/// This gates *logging only*. It never changes which events the simulation
/// produces, which is the "disable logging without behaviour changes" rule.
pub fn verbosity_logs(kind: EventKind, verbosity: u32) -> bool {
    match verbosity {
        0 => false,
        1 => matches!(
            kind,
            EventKind::Round | EventKind::Pickup | EventKind::Reroute | EventKind::Door
        ),
        _ => true,
    }
}

/// The highest verbosity level the lab cycles through.
pub const MAX_VERBOSITY: u32 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_is_deterministic_for_a_fixed_seed() {
        let manifest = LaunchManifest::new(DEFAULT_SEED);
        let a = simulate(manifest);
        let b = simulate(manifest);
        assert_eq!(a, b);
        assert_eq!(checksum(&a), checksum(&b));
        assert!(!a.is_empty(), "the run produces events");
    }

    #[test]
    fn different_seeds_produce_different_streams() {
        let base = checksum(&simulate(LaunchManifest::new(DEFAULT_SEED)));
        // A handful of nearby and distant seeds should not collide.
        for seed in [0u32, 1, 2, 7, 42, 1976, 99_999] {
            let other = checksum(&simulate(LaunchManifest::new(seed)));
            assert_ne!(
                base, other,
                "seed {seed} collided with the default seed checksum"
            );
        }
    }

    #[test]
    fn stream_stays_within_authored_bounds() {
        for seed in [0u32, 1, 1975, 50_000] {
            let events = simulate(LaunchManifest::new(seed));
            // Per-tick cap is the random max plus the single scripted round event.
            let max_len = TICKS_PER_RUN as usize * (MAX_EVENTS_PER_TICK + 1);
            assert!(events.len() <= max_len);
            for TickEvent { tick, event } in events {
                assert!(tick < TICKS_PER_RUN);
                if let Some(room) = event.room() {
                    assert!(room < ROOM_COUNT, "room {room} out of range");
                }
            }
        }
    }

    #[test]
    fn round_cadence_is_scripted_and_monotonic() {
        let events = simulate(LaunchManifest::new(DEFAULT_SEED));
        let rounds: Vec<(u32, u8)> = events
            .iter()
            .filter_map(|e| match e.event {
                TracedEvent::RoundAdvanced { round } => Some((e.tick, round)),
                _ => None,
            })
            .collect();
        assert_eq!(rounds, vec![(12, 1), (24, 2), (36, 3)]);
    }

    #[test]
    fn verbosity_is_monotonic_and_off_means_silent() {
        for kind in EventKind::ALL {
            assert!(!verbosity_logs(kind, 0), "verbosity 0 logs nothing");
            assert!(verbosity_logs(kind, 2), "verbosity 2 logs everything");
            // Level 1 is a subset of level 2.
            if verbosity_logs(kind, 1) {
                assert!(verbosity_logs(kind, 2));
            }
        }
    }
}
