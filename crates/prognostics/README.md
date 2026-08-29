# prognostics

Degradation models, remaining useful life, and ONNX inference through `ort`.

RUL is hybrid: physics extrapolates the health-parameter trajectory to its failure threshold, and a learned model supplies only the correction. Every estimate is an interval with conformal coverage, never a point.
