# prognostics

Degradation models, remaining useful life, and ONNX inference through `ort`.

RUL is hybrid: physics extrapolates the health-parameter trajectory to its failure threshold, and a learned model supplies only the correction. Every estimate is an interval, never a point. The interval today is **covariance-derived**, not conformal: conformal prediction earns its coverage from a calibration set of past run-to-failure trajectories, and the recorder that produces one arrives at D12.
