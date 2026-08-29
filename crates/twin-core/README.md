# twin-core

Twin synchronisation: UKF state estimation over an augmented state that includes the slowly-varying health parameters, residual generation, and the per-subsystem health indices.

This is the layer that makes the system a twin rather than a simulator beside a dashboard. Drift in the estimated health parameters is degradation; their covariance is the uncertainty.
