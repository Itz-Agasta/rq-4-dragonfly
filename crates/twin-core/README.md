# twin-core

Twin synchronisation: a joint unscented Kalman filter over an augmented state that carries
the engine's states, the lag states of its slower instruments, and the slowly-varying health
parameters; residual generation against a healthy-engine model; and the per-subsystem health
indices.

This is the layer that makes the system a twin rather than a simulator beside a dashboard.
Drift in the estimated health parameters is degradation, and their covariance is the
uncertainty.

## Two numbers, and they answer different questions

An estimator that carries health parameters does its job by making the residual go away, so
once it has explained a fault its own innovation is back at zero. Feeding that to a display
would show a degraded engine as nothing at all.

- **Innovation**: what the filter failed to predict after estimating the engine's
  condition. This says whether the twin is tracking the machine in front of it.
- **Residual**: the same model evaluated with every health parameter held at nominal,
  compared against the measurement. This says whether that machine is healthy, and it is
  what a display is fed.

A residual generator never consumes the channel it predicts; see the `nominal` module.

## Layout

| module     | what it holds                                                            |
| ---------- | ------------------------------------------------------------------------ |
| `ukf`      | the filter, with nothing in it about engines                             |
| `health`   | the health parameter vector and its mapping onto the engine's parameters |
| `channels` | what is compared, and how much residual is unremarkable                  |
| `nominal`  | what a healthy engine would be reading right now                         |
| `twin`     | the synchronisation loop                                                 |
| `indices`  | the seven subsystem health indices, each with the quantity that set it   |

Pure of I/O and async, like `engine-model`. `tests/tracking.rs` drives it against a plant
built from the engine model with instruments that lag, round and add noise, so the estimator
can be evaluated without a bus, a daemon or any serialisation.
