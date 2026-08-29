//! Fixed-step Runge-Kutta integration.
//!
//! Fixed step, not adaptive, for two reasons. The turbocharger shaft is by far the
//! stiffest state and an adaptive stepper collapses its step size during spool,
//! exactly when the model has to run fastest. More importantly a fixed step makes
//! the model bit-reproducible: the same inputs give the same trajectory on every
//! run and every machine, which is what allows a recorded mission to be replayed
//! and re-derived rather than merely played back.

use std::ops::{Add, Mul};

/// Integration step, s. 200 Hz.
///
/// Chosen against the fastest mode in the model, the exhaust manifold, whose
/// filling time constant is a few milliseconds. Publishing telemetry at 20 to 50 Hz
/// then means an integer number of steps per published frame, so frames land on
/// exact state samples rather than interpolations between them.
pub const DT: f64 = 1.0 / 200.0;

/// One classical fourth-order Runge-Kutta step.
///
/// The derivative closure takes the state and returns its time derivative in the
/// same type. Both operations the algorithm needs, addition and scaling, come from
/// the bounds, so adding a state variable to the model is adding a field and an
/// arm to that type's operators, and this function does not change.
pub fn rk4<T, F>(x: T, dt: f64, derivative: F) -> T
where
    T: Copy + Add<Output = T> + Mul<f64, Output = T>,
    F: Fn(T) -> T,
{
    let k1 = derivative(x);
    let k2 = derivative(x + k1 * (dt / 2.0));
    let k3 = derivative(x + k2 * (dt / 2.0));
    let k4 = derivative(x + k3 * dt);
    x + (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (dt / 6.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Debug)]
    struct Scalar(f64);

    impl Add for Scalar {
        type Output = Self;
        fn add(self, o: Self) -> Self {
            Self(self.0 + o.0)
        }
    }
    impl Mul<f64> for Scalar {
        type Output = Self;
        fn mul(self, k: f64) -> Self {
            Self(self.0 * k)
        }
    }

    #[test]
    fn integrates_exponential_decay_to_fourth_order() {
        // dx/dt = -x from x(0) = 1 has the exact solution exp(-t). Halving the step
        // must cut the error by roughly sixteen, which is the property that would be
        // lost if any stage coefficient were wrong.
        let error_at = |dt: f64| {
            let steps = (1.0 / dt).round() as u32;
            let mut x = Scalar(1.0);
            for _ in 0..steps {
                x = rk4(x, dt, |s| s * -1.0);
            }
            (x.0 - (-1.0f64).exp()).abs()
        };
        let coarse = error_at(0.1);
        let fine = error_at(0.05);
        assert!(coarse / fine > 12.0, "order too low: {coarse} then {fine}");
    }

    #[test]
    fn is_exactly_reproducible() {
        let run = || {
            let mut x = Scalar(1.0);
            for _ in 0..1000 {
                x = rk4(x, DT, |s| s * -0.7 + Scalar(0.3));
            }
            x.0
        };
        assert_eq!(run().to_bits(), run().to_bits());
    }
}
