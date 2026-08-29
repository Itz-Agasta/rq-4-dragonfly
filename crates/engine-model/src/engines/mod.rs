//! Shipped engine parameter sets.
//!
//! The parameter files are compiled in rather than read from disk so that a binary
//! carrying this crate has a working engine with no deployment steps. Loading a
//! tuned file from disk is still possible through [`crate::EngineParams::from_toml`].

use crate::EngineParams;

/// Parameter file for the 180 hp heavy-fuel aero diesel, as text.
pub const AE330_TOML: &str = include_str!("ae330.toml");

/// The 180 hp heavy-fuel aero diesel.
///
/// # Panics
/// Panics if the compiled-in parameter file is invalid, which is a build-time
/// error made visible at first use; the test suite calls this on every run.
#[must_use]
pub fn ae330() -> EngineParams {
    EngineParams::from_toml(AE330_TOML).expect("shipped parameter file must be valid")
}
