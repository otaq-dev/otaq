//! Attention (SDD §12; d0016, d0042): what needs the user and when to tell them. The needs-you family itself is
//! [`crate::signals`]; the needs-you items are [`crate::board::needs_you`].

pub mod focus;
pub mod inbox;
pub mod limit_stops;
pub mod notify;

pub use focus::{ClientId, Focus, FocusChange, Released, Side};
pub use inbox::Inbox;
pub use limit_stops::{LimitStops, Suppression};
