//! Twin synchronisation: UKF state estimation, residual generation, health indices.
//!
//! This is the layer that makes the system a twin rather than a simulator sitting
//! beside a dashboard. Each telemetry frame, the model predicts from the same inputs
//! the real engine got, the residual is measurement minus prediction, and a UKF over
//! an augmented state estimates slowly-varying health parameters. Drift in those
//! parameters is degradation, and their covariance is the uncertainty, for free.
//!
//! Health indices are published per subsystem with an explicit printable formula.
//! Never a black-box score: an operator must be able to ask why Lubrication is 61
//! and get an answer in engineering units.
#![forbid(unsafe_code)]

pub mod channels;
pub mod health;
pub mod indices;
pub mod nominal;
pub mod twin;
pub mod ukf;

pub use channels::Measurement;
pub use twin::{Tuning, Twin, TwinOutput};
