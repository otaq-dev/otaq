//! Attention (SDD §12; d0016, d0042): what needs the user and when to tell them. The orange family itself is
//! [`crate::signals`]; the needs-you items are [`crate::board::needs_you`].

pub mod focus;

pub use focus::{ClientId, Focus, FocusChange, Released};
