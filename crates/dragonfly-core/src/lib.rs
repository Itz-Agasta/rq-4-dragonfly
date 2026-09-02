//! The daemon's internals, as a library.
//!
//! The binary in `main.rs` is the interesting artifact; this exists so the
//! recorded-mission format and the ingest path are importable. `mission-gen`
//! drives [`telemetry::Fusion`] and [`ingest::Ingest`] directly to build a
//! recording offline, which is what keeps a recorded mission and a live one the
//! same data produced by the same code rather than two formats that agree until
//! one of them changes.
//!
//! Nothing here opens a socket. The CAN socket and the runtime live in the
//! binary alone, which is what lets the generator run with no bus and no clock.

pub mod ingest;
pub mod record;
pub mod replay;
pub mod server;
pub mod telemetry;
