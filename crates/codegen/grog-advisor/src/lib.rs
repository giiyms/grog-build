//! Session-scoped sidecar reviewer for grog (`/advisor`).
//!
//! Protocol is inspired by oh-my-pi's advisor/watchdog (severity, silence
//! default, one note per update, no self-review). Reimplemented in Rust.
//! This is not a wrap of oh-my-pi, pi npm packages, or `/council`.

pub mod command;
pub mod delta;
pub mod guard;
pub mod note;
pub mod persist;
pub mod prompt;
pub mod seats;
pub mod state;

pub use command::{AdvisorVerb, ModelSpec, parse_verb};
pub use delta::{TranscriptItem, is_advisor_injected, render_delta};
pub use guard::EmissionGuard;
pub use note::{AcceptedNote, AdvisorNote, Delivery, Severity, parse_advisor_output};
pub use persist::{SeatSource, prefer_config_or_complement, seat_from_config};
pub use prompt::{build_review_prompt, system_prompt};
pub use seats::{
    AdvisorSeat, ResolveError, complement_seat, cycle_seat, display_name_for, resolve_spec,
    resolve_short_name, seat_readiness,
};
pub use state::{AdvisorState, ReviewJob};
