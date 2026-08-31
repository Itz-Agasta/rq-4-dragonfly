//! What a healthy engine would be doing right now.
//!
//! # Why an adapted twin cannot produce the residual an operator needs
//!
//! An estimator that carries health parameters does its job by making the residual
//! go away: it moves the parameters until the model agrees with the machine, and
//! once it has, the innovation is back at zero whether or not anything is wrong.
//! That is the right behaviour for estimating condition and the wrong quantity
//! entirely for showing it. A screen fed the filter's innovation would show a
//! coked injector as nothing at all, which is the failure the whole design exists
//! to avoid.
//!
//! So there are two questions and two numbers. *Is the twin synchronised* is
//! answered by the filter's own innovation. *What is this engine doing that a
//! healthy one would not* is answered here, by a second evaluation of the same
//! model with every health parameter held at nominal.
//!
//! # A residual generator never consumes the channel it predicts
//!
//! This is the rule that makes the residual real. The states this takes from the
//! filter are the ones the fault does not bias, which is manifold pressures, the
//! two shaft speeds, coolant and oil temperature. Everything the fault does move is
//! computed forward from the fuelling command instead: cylinder metal temperature
//! is integrated here rather than read, and both instrument lags are driven by this
//! model's own predictions rather than by what the instruments said. Feeding a
//! measurement into the prediction of itself produces a residual that is structurally
//! incapable of being large, which is the classical mistake in model-based diagnosis.
//!
//! Frisk, Krysander & Eriksson's structured residual formulation is the source of
//! the rule: each residual is generated from a subset of the measurements chosen so
//! that different faults make different subsets inconsistent.

use engine_model::{CYLINDERS, EngineParams, Inputs, State, thermal};

use crate::channels::{CHANNELS, index};

/// Exhaust thermocouple time constant, s. Matches the filter's; see `twin`.
const EGT_TAU_S: f64 = 2.0;
/// Head sensor time constant, s.
const CHT_TAU_S: f64 = 0.5;

/// A healthy-engine predictor running alongside the filter.
#[derive(Clone, Debug)]
pub struct Nominal {
    params: EngineParams,
    t_cht: [f64; CYLINDERS],
    egt_lag: [f64; CYLINDERS],
    cht_lag: [f64; CYLINDERS],
    seeded: bool,
}

impl Nominal {
    /// A predictor of an engine with the given as-new parameters.
    #[must_use]
    pub fn new(params: EngineParams) -> Self {
        Self {
            params,
            t_cht: [0.0; CYLINDERS],
            egt_lag: [0.0; CYLINDERS],
            cht_lag: [0.0; CYLINDERS],
            seeded: false,
        }
    }

    /// Forget the integrated states, so the next call re-seeds from the estimate.
    pub fn reset(&mut self) {
        self.seeded = false;
    }

    /// What every instrument would read on a healthy engine at this operating point.
    ///
    /// `state` is the filter's estimate. Its cylinder metal temperatures are
    /// ignored in favour of the ones integrated here, for the reason in the module
    /// header: a metal temperature pulled down by its own measurement cannot report
    /// that it is lower than it should be.
    pub fn predict(&mut self, state: &State, u: &Inputs, dt: f64) -> [f64; CHANNELS] {
        let mut x = *state;
        if !self.seeded {
            self.t_cht = state.t_cht;
        }
        x.t_cht = self.t_cht;

        let o = engine_model::evaluate(&self.params, &x, u);
        if !self.seeded {
            self.egt_lag = o.t_egt;
            self.cht_lag = self.t_cht;
            self.seeded = true;
        }

        // Explicit Euler on the metal nodes. Their time constant is tens of seconds
        // against a frame of fifty milliseconds, so the step is three orders of
        // magnitude inside stability and a higher-order integrator would buy nothing.
        for i in 0..CYLINDERS {
            let rate = thermal::head_temperature_rate(
                &self.params,
                self.t_cht[i],
                x.t_coolant,
                o.w_fuel_cylinder[i],
                x.omega_e,
            );
            self.t_cht[i] = (self.t_cht[i] + rate * dt).max(1.0);
        }

        let egt_alpha = 1.0 - (-dt / EGT_TAU_S).exp();
        let cht_alpha = 1.0 - (-dt / CHT_TAU_S).exp();
        for i in 0..CYLINDERS {
            self.egt_lag[i] = egt_alpha.mul_add(o.t_egt[i] - self.egt_lag[i], self.egt_lag[i]);
            self.cht_lag[i] = cht_alpha.mul_add(self.t_cht[i] - self.cht_lag[i], self.cht_lag[i]);
        }

        let mut z = [0.0; CHANNELS];
        z[index::RPM] = x.rpm();
        z[index::MAP] = x.p_im;
        z[index::MAT] = o.t_intake;
        z[index::MAF] = o.w_air;
        z[index::TURBO] = x.turbo_rpm();
        z[index::TORQUE] = o.torque_brake;
        z[index::FUEL_FLOW] = o.w_fuel * 3600.0;
        z[index::OIL_PRESSURE] = o.p_oil;
        z[index::OIL_TEMPERATURE] = x.t_oil;
        z[index::COOLANT] = x.t_coolant;
        for i in 0..CYLINDERS {
            z[index::EGT + i] = self.egt_lag[i];
            z[index::CHT + i] = self.cht_lag[i];
            z[index::LAMBDA + i] = o.lambda_cylinder[i].clamp(0.5, 20.0);
        }
        z
    }
}
