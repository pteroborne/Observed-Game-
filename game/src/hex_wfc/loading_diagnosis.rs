//! How a facility load reports what went wrong (see `loading.rs`).
//!
//! Split out of the parent module to keep it under the 600-line ratchet, but it is a
//! coherent unit on its own: everything here exists so that a failed load leaves
//! evidence. Backlog #29 is the reason the file exists at all — a Deck-to-Deck
//! playtest lost the game between the solve and a playable map with nothing recorded
//! anywhere, and the missing diagnosis was a second defect inside the first.
//!
//! # Why the load path catches panics
//!
//! `bevy_tasks::TaskPool::spawn` does **not** wrap the spawned future in
//! `catch_unwind`; only `Scope::spawn` does. A panic inside the solve therefore
//! unwinds on a pool thread, where two things happen and neither of them produces a
//! diagnosis a player or a bug report can use:
//!
//! 1. `async_task` cancels the task and the pool thread's own `catch_unwind` swallows
//!    the payload, so the only trace is the default hook's line on stderr — which is
//!    nowhere at all for a packaged build launched from a game library.
//! 2. The next `poll_once` on that cancelled handle panics again (bevy's own words:
//!    "Tasks that panic get immediately canceled. Awaiting a canceled task also
//!    causes a panic") — and that second panic lands on the **main thread**, inside a
//!    Bevy system, so the process dies rather than reaching the loading screen's
//!    error state, which has been sitting there ready to show the reason all along.
//!
//! Both halves are guarded: the solve runs inside [`guard_preparation`] so the payload
//! becomes a reportable [`HexLoadingError`], and the parent guards the poll itself so
//! a handle lost by some other route still fails visibly instead of fatally.

use std::{any::Any, cell::RefCell, panic::AssertUnwindSafe, sync::OnceLock, time::Duration};

use bevy::prelude::*;

use super::super::launch::{HexLaunchError, HexSeedPolicy, PreparedHexLaunch};
use super::{HexLoadingPhase, HexLoadingState};

/// How long a `Preparing` phase may run with no worker handle before it is declared
/// lost. Generous on purpose: `begin_request` inserts the worker through `Commands`,
/// so there is a legitimate window of at most one command flush where the phase has
/// advanced and the resource has not landed yet. A real solve keeps its handle for
/// its whole duration, so nothing but an actual loss reaches this.
pub(super) const WORKER_WATCHDOG_GRACE: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HexLoadingError {
    MissingRequest,
    Preparation(HexLaunchError),
    /// The solve unwound. Carries the panic message and, when the reporter hook is
    /// installed, the source location it came from.
    PreparationPanicked(String),
    /// The worker handle stopped being pollable: it panicked on poll, or it vanished
    /// while the phase still said `Preparing`. Either way the match cannot start, and
    /// saying so is strictly better than the process disappearing.
    WorkerLost(String),
    LanLaunchUnavailable,
    LanLaunchWithdrawn,
    LanTransport(String),
}

impl std::fmt::Display for HexLoadingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingRequest => formatter.write_str(
                "No finalized launch request was provided. Return to Play and try again.",
            ),
            Self::Preparation(error) => error.fmt(formatter),
            Self::PreparationPanicked(detail) => {
                write!(formatter, "the facility solve panicked: {detail}")
            }
            Self::WorkerLost(detail) => {
                write!(formatter, "the facility solve was lost: {detail}")
            }
            Self::LanLaunchUnavailable => formatter
                .write_str("The server launch changed or disappeared before preparation began."),
            Self::LanLaunchWithdrawn => formatter
                .write_str("The server returned to the lobby while players were preparing."),
            Self::LanTransport(error) => write!(formatter, "send launch readiness: {error}"),
        }
    }
}

/// Where the most recent panic on *this* thread came from.
///
/// The hook runs on the panicking thread before unwinding starts, so a thread-local
/// pairs a payload with its location without a lock and without any chance of picking
/// up a different thread's panic.
pub(super) struct PanicSite {
    location: String,
    backtrace: String,
}

thread_local! {
    static LAST_PANIC_SITE: RefCell<Option<PanicSite>> = const { RefCell::new(None) };
}

static PANIC_REPORTER: OnceLock<()> = OnceLock::new();

/// Chain a location-recording hook in front of whatever hook is already installed.
///
/// Installed once, and it never replaces the previous behavior — the default hook
/// still writes its usual line. All this adds is a copy of the location (and a
/// backtrace when `RUST_BACKTRACE` asks for one) that survives `catch_unwind`, which
/// otherwise hands back a payload with no idea where it came from.
pub(super) fn install_panic_reporter() {
    PANIC_REPORTER.get_or_init(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let location = info
                .location()
                .map_or_else(|| "unknown location".to_string(), ToString::to_string);
            // `capture` is a no-op unless RUST_BACKTRACE is set, so this costs nothing
            // in normal play and gives a full stack to anyone chasing a field report.
            let backtrace = std::backtrace::Backtrace::capture().to_string();
            LAST_PANIC_SITE.with(|slot| {
                *slot.borrow_mut() = Some(PanicSite {
                    location,
                    backtrace,
                });
            });
            previous(info);
        }));
    });
}

pub(super) fn take_panic_site() -> Option<PanicSite> {
    LAST_PANIC_SITE.with(|slot| slot.borrow_mut().take())
}

/// Turn an unwind payload into something a bug report can act on.
///
/// `panic!`, `assert!`, `unwrap` and `expect` all produce a `String` or `&'static str`
/// payload; anything else at least gets named rather than reported as an empty error.
pub(super) fn describe_panic(payload: &(dyn Any + Send), site: Option<PanicSite>) -> String {
    let message = payload.downcast_ref::<String>().map_or_else(
        || {
            payload
                .downcast_ref::<&'static str>()
                .map_or("a non-string panic payload", |message| *message)
                .to_string()
        },
        Clone::clone,
    );
    match site {
        Some(site) => format!("{message} (at {})", site.location),
        None => message,
    }
}

/// Run one preparation attempt so that an unwind becomes a value instead of killing
/// the pool thread and, one poll later, the process.
///
/// `AssertUnwindSafe` is deliberate and safe here: the closure borrows nothing that
/// outlives it, and the only shared state `prepare` touches is the corpus `OnceLock`,
/// which is simply left uninitialized by a panic during `get_or_init` and can be
/// retried. Nothing observable is left half-written for a later reader.
pub(super) fn guard_preparation<F>(run: F) -> Result<PreparedHexLaunch, HexLoadingError>
where
    F: FnOnce() -> Result<PreparedHexLaunch, HexLaunchError>,
{
    install_panic_reporter();
    // Discard any earlier report so a panic-free run cannot inherit a stale location.
    drop(take_panic_site());
    match std::panic::catch_unwind(AssertUnwindSafe(run)) {
        Ok(Ok(prepared)) => Ok(prepared),
        Ok(Err(error)) => Err(HexLoadingError::Preparation(error)),
        Err(payload) => {
            let site = take_panic_site();
            let backtrace = site
                .as_ref()
                .map_or_else(String::new, |site| site.backtrace.clone());
            let detail = describe_panic(payload.as_ref(), site);
            error!("hex facility preparation panicked: {detail}\n{backtrace}");
            Err(HexLoadingError::PreparationPanicked(detail))
        }
    }
}

/// Short seed-policy name for the start-of-load line. The full expected hash is not
/// wanted here — a mismatch reports both hashes through the failure path instead.
pub(super) const fn seed_policy_label(policy: HexSeedPolicy) -> &'static str {
    match policy {
        HexSeedPolicy::Nearby => "(nearby seeds allowed)",
        HexSeedPolicy::Exact { .. } => "(exact, content-hash locked)",
    }
}

/// A `Preparing` phase whose worker handle has gone missing is a load that will never
/// finish. Report it once the command-flush window has safely passed.
pub(super) fn worker_watchdog(state: &HexLoadingState) -> Option<HexLoadingError> {
    (state.phase == HexLoadingPhase::Preparing && state.elapsed >= WORKER_WATCHDOG_GRACE).then(
        || {
            HexLoadingError::WorkerLost(format!(
                "no preparation worker after {:.1} s on attempt {}",
                state.elapsed.as_secs_f32(),
                state.attempt
            ))
        },
    )
}
