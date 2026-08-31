//! An unscented Kalman filter over a dense state, with nothing in it about engines.
//!
//! The unscented transform propagates a deterministic set of sample points through
//! the true nonlinear map rather than through a linearisation of it, which is what
//! makes it usable here: the plant this filter is wrapped around is a stiff
//! thermodynamic model with no analytic Jacobian, and computing one numerically
//! would cost the same sample points without the accuracy.
//!
//! The filter is deliberately generic over the propagation and measurement maps.
//! Keeping it free of the model is what allows it to be tested against a linear
//! system, where the transform is exact and the answer has to agree with an
//! ordinary Kalman filter to the last bit. A filter tested only against the plant
//! it was written for cannot distinguish its own bugs from the plant's.
//!
//! # References
//!
//! Julier & Uhlmann, "Unscented Filtering and Nonlinear Estimation", Proceedings of
//! the IEEE 92(3), 2004. The scaled transform and the weight derivation.
//! <https://doi.org/10.1109/JPROC.2003.823141>
//!
//! van der Merwe & Wan, "The square-root unscented Kalman filter for state and
//! parameter-estimation", ICASSP 2001, for the joint state-and-parameter
//! formulation an augmented state implements.
//! <https://doi.org/10.1109/ICASSP.2001.940586>

use nalgebra::{DMatrix, DVector};

/// What can go wrong in a step.
#[derive(Debug, thiserror::Error)]
pub enum FilterError {
    /// The covariance stopped being positive definite and jitter did not recover it.
    #[error("covariance is not positive definite after {attempts} attempts at regularisation")]
    NotPositiveDefinite {
        /// How many times a jittered factorisation was tried.
        attempts: usize,
    },
    /// The innovation covariance could not be inverted.
    #[error("innovation covariance is singular")]
    SingularInnovation,
    /// A sigma point or a measurement carried a non-finite value.
    #[error("non-finite value in {what}")]
    NonFinite {
        /// Which quantity was non-finite.
        what: &'static str,
    },
}

/// Sigma point spread and weighting.
///
/// The textbook default of `alpha = 1e-3` is written for state dimensions in the
/// single digits and is actively harmful above about ten. It puts the centre weight
/// at `lambda / (n + lambda)`, which for `n = 27` is roughly `-1e6`: the mean
/// becomes a difference of enormous opposing terms and the covariance loses its
/// positive definiteness within a few steps. Any `alpha` below one gives a negative
/// centre weight at large `n`, so the default here is `alpha = 1`, which puts every
/// weight at or above zero and spreads the points at `sqrt(n)` standard deviations.
#[derive(Clone, Copy, Debug)]
pub struct Spread {
    /// Point spread. One keeps every weight non-negative at any dimension.
    pub alpha: f64,
    /// Prior distribution knowledge. Two is optimal for a Gaussian.
    pub beta: f64,
    /// Secondary scaling. Zero, so the spread is set by `alpha` alone.
    pub kappa: f64,
}

impl Default for Spread {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            beta: 2.0,
            kappa: 0.0,
        }
    }
}

/// The innovation of an update: what the measurement said the model got wrong.
///
/// This is the residual the rest of the system is built on, and it is a filter
/// output rather than a separate comparison. Its covariance is what a residual has
/// to be judged against: the same excursion means something different when the
/// filter is confident than when it is not.
#[derive(Clone, Debug)]
pub struct Innovation {
    /// Measurement less predicted measurement, in measurement units.
    pub residual: DVector<f64>,
    /// Innovation covariance, `H P H' + R` in the linear case.
    pub covariance: DMatrix<f64>,
}

impl Innovation {
    /// Residual divided by its own standard deviation, per channel.
    ///
    /// The diagonal alone, deliberately. A full whitening by the inverse Cholesky
    /// factor mixes channels together, and a residual that cannot be named after one
    /// measurement is useless for saying which measurement moved.
    #[must_use]
    pub fn normalised(&self) -> DVector<f64> {
        DVector::from_iterator(
            self.residual.len(),
            self.residual.iter().enumerate().map(|(i, r)| {
                let sigma = self.covariance[(i, i)].max(0.0).sqrt();
                if sigma > 0.0 { r / sigma } else { 0.0 }
            }),
        )
    }
}

/// Mean and covariance, with the machinery to move them through a nonlinear map.
#[derive(Clone, Debug)]
pub struct Ukf {
    mean: DVector<f64>,
    covariance: DMatrix<f64>,
    lambda: f64,
    weights_mean: Vec<f64>,
    weights_covariance: Vec<f64>,
}

/// Diagonal jitter added to a covariance whose factorisation failed, relative to
/// its own trace. Six attempts span twelve orders of magnitude, which is far more
/// than a recoverable loss of definiteness needs and still finite.
const JITTER_ATTEMPTS: usize = 6;

impl Ukf {
    /// A filter at a given mean and covariance.
    ///
    /// # Panics
    ///
    /// If the covariance is not square or does not match the mean's dimension.
    #[must_use]
    pub fn new(mean: DVector<f64>, covariance: DMatrix<f64>, spread: Spread) -> Self {
        assert_eq!(
            covariance.nrows(),
            covariance.ncols(),
            "covariance is square"
        );
        assert_eq!(
            covariance.nrows(),
            mean.len(),
            "covariance matches the mean"
        );

        let n = mean.len() as f64;
        let lambda = spread.alpha.powi(2) * (n + spread.kappa) - n;
        let denominator = 2.0 * (n + lambda);
        let mut weights_mean = vec![1.0 / denominator; 2 * mean.len() + 1];
        let mut weights_covariance = weights_mean.clone();
        weights_mean[0] = lambda / (n + lambda);
        weights_covariance[0] = weights_mean[0] + 1.0 - spread.alpha.powi(2) + spread.beta;

        Self {
            mean,
            covariance,
            lambda,
            weights_mean,
            weights_covariance,
        }
    }

    /// The current estimate.
    #[must_use]
    pub fn mean(&self) -> &DVector<f64> {
        &self.mean
    }

    /// The current covariance.
    #[must_use]
    pub fn covariance(&self) -> &DMatrix<f64> {
        &self.covariance
    }

    /// Standard deviation of one element of the state.
    #[must_use]
    pub fn sigma(&self, i: usize) -> f64 {
        self.covariance[(i, i)].max(0.0).sqrt()
    }

    /// Overwrite one element of the mean, leaving the covariance alone.
    ///
    /// Used to hold an estimate inside physical bounds after an update. Clipping the
    /// mean without touching the covariance keeps the filter's own uncertainty
    /// honest: the estimate is being constrained, not the evidence.
    pub fn clamp_element(&mut self, i: usize, lo: f64, hi: f64) {
        self.mean[i] = self.mean[i].clamp(lo, hi);
    }

    /// Sigma points of the current distribution, `2n + 1` of them.
    fn sigma_points(&self) -> Result<Vec<DVector<f64>>, FilterError> {
        let n = self.mean.len();
        let scale = n as f64 + self.lambda;
        let root = cholesky_lower(&self.covariance)? * scale.sqrt();

        let mut points = Vec::with_capacity(2 * n + 1);
        points.push(self.mean.clone());
        for i in 0..n {
            let column = root.column(i);
            points.push(&self.mean + column);
            points.push(&self.mean - column);
        }
        Ok(points)
    }

    /// Weighted mean and covariance of a propagated point set.
    fn recombine(
        &self,
        points: &[DVector<f64>],
        noise: &DMatrix<f64>,
    ) -> (DVector<f64>, DMatrix<f64>) {
        let dim = points[0].len();
        let mut mean = DVector::zeros(dim);
        for (w, p) in self.weights_mean.iter().zip(points) {
            mean += p * *w;
        }

        let mut covariance = noise.clone();
        for (w, p) in self.weights_covariance.iter().zip(points) {
            let d = p - &mean;
            covariance += (&d * d.transpose()) * *w;
        }
        (mean, covariance)
    }

    /// Advance the state through `f` and add the process noise `q`.
    ///
    /// # Errors
    ///
    /// If the covariance cannot be factorised or `f` returns a non-finite value.
    pub fn predict<F>(&mut self, f: F, q: &DMatrix<f64>) -> Result<(), FilterError>
    where
        F: Fn(&DVector<f64>) -> DVector<f64>,
    {
        let propagated: Vec<DVector<f64>> = self.sigma_points()?.iter().map(&f).collect();
        for p in &propagated {
            if p.iter().any(|v| !v.is_finite()) {
                return Err(FilterError::NonFinite {
                    what: "propagated sigma point",
                });
            }
        }

        let (mean, covariance) = self.recombine(&propagated, q);
        self.mean = mean;
        self.covariance = symmetrise(covariance);
        Ok(())
    }

    /// Correct against a measurement, returning the innovation.
    ///
    /// The sigma points are regenerated from the predicted distribution rather than
    /// reused from the prediction step. Reusing them is a common shortcut and it is
    /// wrong once process noise has been added: the points then describe the
    /// distribution before the noise widened it, and the filter becomes
    /// overconfident in exactly the way that makes a residual band too tight.
    ///
    /// # Errors
    ///
    /// If the covariance cannot be factorised, the innovation covariance is
    /// singular, or either map returns a non-finite value.
    pub fn update<H>(
        &mut self,
        measurement: &DVector<f64>,
        h: H,
        r: &DMatrix<f64>,
    ) -> Result<Innovation, FilterError>
    where
        H: Fn(&DVector<f64>) -> DVector<f64>,
    {
        if measurement.iter().any(|v| !v.is_finite()) {
            return Err(FilterError::NonFinite {
                what: "measurement",
            });
        }

        let points = self.sigma_points()?;
        let predicted: Vec<DVector<f64>> = points.iter().map(&h).collect();
        for p in &predicted {
            if p.iter().any(|v| !v.is_finite()) {
                return Err(FilterError::NonFinite {
                    what: "predicted measurement",
                });
            }
        }

        let (predicted_mean, innovation_covariance) = self.recombine(&predicted, r);

        let mut cross = DMatrix::zeros(self.mean.len(), predicted_mean.len());
        for ((w, x), z) in self.weights_covariance.iter().zip(&points).zip(&predicted) {
            cross += ((x - &self.mean) * (z - &predicted_mean).transpose()) * *w;
        }

        let inverse = innovation_covariance
            .clone()
            .try_inverse()
            .ok_or(FilterError::SingularInnovation)?;
        let gain = &cross * &inverse;
        let residual = measurement - &predicted_mean;

        self.mean += &gain * &residual;
        self.covariance =
            symmetrise(&self.covariance - &gain * &innovation_covariance * gain.transpose());

        Ok(Innovation {
            residual,
            covariance: innovation_covariance,
        })
    }
}

/// Force exact symmetry.
///
/// The covariance is symmetric in exact arithmetic and drifts away from it in
/// floating point. The drift is tiny and it compounds: an asymmetric matrix fails
/// Cholesky long before it stops being positive definite in any meaningful sense.
fn symmetrise(m: DMatrix<f64>) -> DMatrix<f64> {
    let t = m.transpose();
    (m + t) * 0.5
}

/// Lower Cholesky factor, with escalating diagonal jitter on failure.
///
/// A failure here means the covariance has lost positive definiteness to rounding,
/// which is recoverable, or that the filter has genuinely diverged, which is not.
/// Jittering distinguishes the two: rounding needs a nudge of order the machine
/// epsilon times the trace, and divergence is not rescued by any amount.
fn cholesky_lower(m: &DMatrix<f64>) -> Result<DMatrix<f64>, FilterError> {
    if let Some(c) = m.clone().cholesky() {
        return Ok(c.l());
    }

    let n = m.nrows();
    let scale = (m.trace().abs() / n as f64).max(1.0);
    let mut jitter = scale * 1e-12;
    for _ in 0..JITTER_ATTEMPTS {
        let mut candidate = m.clone();
        for i in 0..n {
            candidate[(i, i)] += jitter;
        }
        if let Some(c) = candidate.cholesky() {
            return Ok(c.l());
        }
        jitter *= 100.0;
    }
    Err(FilterError::NotPositiveDefinite {
        attempts: JITTER_ATTEMPTS,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector(v: &[f64]) -> DVector<f64> {
        DVector::from_column_slice(v)
    }

    /// The unscented transform is exact for a linear map, so on a linear system the
    /// filter has to reproduce an ordinary Kalman filter to numerical precision.
    ///
    /// This is the test that says the weights, the sigma point set and the
    /// recombination are right. Every other test here would pass with a subtly wrong
    /// weight vector, because a nonlinear filter that is merely close still converges.
    #[test]
    fn on_a_linear_system_it_agrees_with_a_kalman_filter() {
        let f = DMatrix::from_row_slice(2, 2, &[1.0, 0.1, 0.0, 1.0]);
        let h = DMatrix::from_row_slice(1, 2, &[1.0, 0.0]);
        let q = DMatrix::from_diagonal(&vector(&[1e-3, 1e-3]));
        let r = DMatrix::from_diagonal(&vector(&[0.25]));

        let mut ukf = Ukf::new(
            vector(&[0.0, 0.0]),
            DMatrix::from_diagonal(&vector(&[1.0, 1.0])),
            Spread::default(),
        );
        let mut kf_mean = vector(&[0.0, 0.0]);
        let mut kf_cov = DMatrix::from_diagonal(&vector(&[1.0, 1.0]));

        for k in 0..40 {
            let z = vector(&[(f64::from(k) * 0.1).sin()]);

            let fc = f.clone();
            ukf.predict(|x| &fc * x, &q).expect("predict");
            let hc = h.clone();
            ukf.update(&z, |x| &hc * x, &r).expect("update");

            kf_mean = &f * kf_mean;
            kf_cov = &f * kf_cov * f.transpose() + &q;
            let s = &h * &kf_cov * h.transpose() + &r;
            let gain = &kf_cov * h.transpose() * s.try_inverse().expect("invertible");
            kf_mean = &kf_mean + &gain * (&z - &h * &kf_mean);
            kf_cov = &kf_cov - &gain * &h * &kf_cov;
        }

        for i in 0..2 {
            assert!(
                (ukf.mean()[i] - kf_mean[i]).abs() < 1e-9,
                "state {i}: {} against {}",
                ukf.mean()[i],
                kf_mean[i]
            );
            assert!((ukf.covariance()[(i, i)] - kf_cov[(i, i)]).abs() < 1e-9);
        }
    }

    /// A constant hidden through a nonlinear measurement is the shape of the health
    /// parameter problem: the parameter does not move, and the only way to see it is
    /// that it changes what the measurement says.
    #[test]
    fn it_recovers_a_parameter_seen_only_through_a_nonlinear_map() {
        let truth = 0.63;
        let mut ukf = Ukf::new(
            vector(&[1.0]),
            DMatrix::from_diagonal(&vector(&[0.25])),
            Spread::default(),
        );
        let q = DMatrix::from_diagonal(&vector(&[1e-8]));
        let r = DMatrix::from_diagonal(&vector(&[1e-4]));

        for k in 0..60 {
            let t = f64::from(k) * 0.05;
            let z = vector(&[(-truth * t).exp() * (1.0 + 0.5 * t)]);
            ukf.predict(|x| x.clone(), &q).expect("predict");
            ukf.update(&z, |x| vector(&[(-x[0] * t).exp() * (1.0 + 0.5 * t)]), &r)
                .expect("update");
        }

        assert!(
            (ukf.mean()[0] - truth).abs() < 1e-3,
            "estimated {}",
            ukf.mean()[0]
        );
        assert!(ukf.sigma(0) < 0.05, "sigma {}", ukf.sigma(0));
    }

    /// At the dimension this filter actually runs at, the textbook spread makes the
    /// centre weight enormous and negative. Asserting the default is not that is
    /// cheaper than rediscovering it from a covariance that stops factorising.
    #[test]
    fn the_default_spread_keeps_every_weight_non_negative_at_high_dimension() {
        let n = 27;
        let ukf = Ukf::new(
            DVector::zeros(n),
            DMatrix::identity(n, n),
            Spread::default(),
        );
        assert!(ukf.weights_mean.iter().all(|w| *w >= 0.0));
        assert!(ukf.weights_covariance.iter().all(|w| *w >= 0.0));
        assert!((ukf.weights_mean.iter().sum::<f64>() - 1.0).abs() < 1e-12);

        let textbook = Spread {
            alpha: 1e-3,
            ..Spread::default()
        };
        let bad = Ukf::new(DVector::zeros(n), DMatrix::identity(n, n), textbook);
        assert!(bad.weights_mean[0] < -1e5, "{}", bad.weights_mean[0]);
    }

    /// Losing positive definiteness to rounding is recoverable and divergence is
    /// not, and the filter has to tell them apart rather than panicking on either.
    #[test]
    fn a_covariance_that_has_lost_definiteness_to_rounding_is_recovered() {
        let nudged = DMatrix::from_row_slice(2, 2, &[1.0, 1.0, 1.0, 1.0 - 1e-15]);
        assert!(cholesky_lower(&nudged).is_ok());

        let indefinite = DMatrix::from_row_slice(2, 2, &[1.0, 0.0, 0.0, -4.0]);
        assert!(matches!(
            cholesky_lower(&indefinite),
            Err(FilterError::NotPositiveDefinite { .. })
        ));
    }

    #[test]
    fn a_non_finite_measurement_is_refused_rather_than_propagated() {
        let mut ukf = Ukf::new(vector(&[0.0]), DMatrix::identity(1, 1), Spread::default());
        let r = DMatrix::identity(1, 1);
        let out = ukf.update(&vector(&[f64::NAN]), |x| x.clone(), &r);
        assert!(matches!(out, Err(FilterError::NonFinite { .. })));
        assert!(ukf.mean()[0].is_finite());
    }
}
