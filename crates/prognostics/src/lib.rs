//! Degradation models, remaining useful life, and ONNX inference at the edge.
//!
//! RUL is hybrid: physics extrapolates the UKF health-parameter trajectory to its
//! failure threshold, and a learned model supplies only the correction term. Pure ML
//! fails off-design, and the residual-learning split is what generalises there.
//!
//! Every estimate is reported as an interval, never a point, because an operator
//! makes a go/no-go call on the lower bound. The interval today is
//! **covariance-derived**, not conformal: conformal prediction earns its coverage
//! from a calibration set of past run-to-failure trajectories and the recorder that
//! would produce one arrives at D12. `crate::rul` says so at the point of use, and
//! `design.md`'s word "conformal" is wrong until that set exists.
//!
//! Inference runs through `ort`. Python trains and exports; it is never in this path.
#![forbid(unsafe_code)]

pub mod rul;
pub mod trend;

pub use rul::{Prognosis, Rul, evaluate};
pub use trend::{Trend, Trends};
