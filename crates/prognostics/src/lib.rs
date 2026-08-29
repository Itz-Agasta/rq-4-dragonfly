//! Degradation models, remaining useful life, and ONNX inference at the edge.
//!
//! RUL is hybrid: physics extrapolates the UKF health-parameter trajectory to its
//! failure threshold, and a learned model supplies only the correction term. Pure ML
//! fails off-design, and the residual-learning split is what generalises there.
//!
//! Every estimate is reported as an interval, never a point. An operator makes a
//! go/no-go call on the lower bound, so conformal prediction supplies distribution-free
//! coverage rather than a number with implied precision it does not have.
//!
//! Inference runs through `ort`. Python trains and exports; it is never in this path.
