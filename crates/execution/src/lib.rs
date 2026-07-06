//! Execution router: order signing, transport, and mode selection.
//!
//! Built at M10 (dry-run) then M12 (live). Selects an execution mode per
//! opportunity: parallel IOC submission of all legs (fastest, but the legs are
//! independent orders with no cross-order atomicity, so partial fills must be
//! unwound), sequential with market-order lock-in (no leg-1 inventory risk),
//! or dry-run (log only, the startup default). Signs orders with Ed25519 over
//! pre-warmed keep-alive connections and prefers the WS-API session for the
//! fastest acknowledgement.
//!
//! Deliberately empty at milestone M0. Nothing here is stubbed with fake fills:
//! the dry-run path is real logging, and no code path submits a live order
//! until M12 behind the signed live-mode flag.
#![forbid(unsafe_code)]
