//! The simulated engine, as a library.
//!
//! The binary is the interesting artifact; this exists so the fault library is
//! importable. `twin-core`'s acceptance tests inject faults through
//! [`fault::Faults::apply`] rather than reimplementing the severity curves, because
//! a test that reimplements the fault it is testing tests the engine model and not
//! the injection path, and the two can drift apart silently.
//!
//! Nothing here talks to a bus. The CAN socket lives in `main.rs` alone.

pub mod fault;
pub mod mission;
pub mod plant;
pub mod publish;
pub mod sensors;
