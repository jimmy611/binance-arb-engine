//! The engine's master trading mode.
//!
//! This is the fail-safe switch mandated by the specification: the engine
//! starts in [`EngineMode::DryRun`], and reaching [`EngineMode::Live`] requires
//! an explicit signed configuration flag plus a UI confirmation elsewhere in
//! the system. This enum is the single shared representation of that state; the
//! per-opportunity execution style (parallel IOC, sequential lock-in) is a
//! separate concept owned by the execution crate.

use core::fmt;

use serde::{Deserialize, Serialize};

/// Whether the engine may send real orders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineMode {
    /// Every decision and would-be order is logged; nothing is sent. This is
    /// the startup default and the safe state the risk manager forces on any
    /// hard breach.
    #[default]
    DryRun,
    /// Real orders may be submitted, subject to risk limits.
    Live,
}

impl EngineMode {
    /// Returns `true` only in [`EngineMode::Live`].
    #[inline]
    #[must_use]
    pub const fn is_live(self) -> bool {
        matches!(self, EngineMode::Live)
    }
}

impl fmt::Display for EngineMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineMode::DryRun => f.write_str("dry-run"),
            EngineMode::Live => f.write_str("live"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_dry_run() {
        assert_eq!(EngineMode::default(), EngineMode::DryRun);
        assert!(!EngineMode::default().is_live());
        assert!(EngineMode::Live.is_live());
    }

    #[test]
    fn serde_is_snake_case() {
        assert_eq!(
            serde_json::to_string(&EngineMode::DryRun).unwrap(),
            "\"dry_run\""
        );
        assert_eq!(
            serde_json::to_string(&EngineMode::Live).unwrap(),
            "\"live\""
        );
    }
}
