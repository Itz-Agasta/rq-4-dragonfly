//! The health parameter vector: what the filter estimates about the engine itself.
//!
//! An engine model is a set of equations plus a set of constants fitted to a
//! particular engine when it was new. Degradation is what happens to those
//! constants. Estimating them online rather than assuming them is the difference
//! between a simulator running beside a dashboard and a twin: the estimate is a
//! measurement of the machine's condition expressed in the units the machine is
//! designed in, and its drift over a mission is degradation.
//!
//! Every parameter here is a multiplier on nominal, except the injector discharge
//! coefficients, which are carried as absolute coefficients because that is the
//! number an injector is specified and rejected against.
//!
//! # What is deliberately not estimated
//!
//! Per-cylinder combustion efficiency. It is identifiable in principle, since a
//! restricted injector delivers less fuel and so raises that cylinder's excess air
//! ratio, whereas poor combustion at the same fuelling leaves it unchanged, and
//! per-cylinder excess air ratio is measured, but nothing in the current fault set
//! moves it. Every additional parameter is another way for the filter to explain a
//! residual, so one that no known mechanism drives weakens the isolation of the ones
//! that do.

use engine_model::{CYLINDERS, EngineParams};

/// How many parameters the filter carries.
pub const PARAMS: usize = 10;

/// Nominal injector discharge coefficient. **estimated**: a last-generation
/// common-rail solenoid injector's discharge coefficient rises with pressure drop
/// towards an asymptote near 0.96 while the flow stays attached. A cavitating
/// cylindrical nozzle runs nearer 0.86, so this belongs to the hydroground conical
/// geometry a modern nozzle uses and not to nozzles in general.
/// Payri, Salvador, Carreres & De la Morena, Energy Conversion and Management 114,
/// 2016. <https://doi.org/10.1016/j.enconman.2016.02.043>
pub const INJECTOR_CD_NOMINAL: f64 = 0.966;

/// Index of each parameter in the vector.
///
/// A plain constant rather than an enum: these index into a `nalgebra` vector and
/// every use is arithmetic, so a conversion at each site would be noise.
pub mod index {
    /// Volumetric efficiency multiplier.
    pub const ETA_VOL: usize = 0;
    /// Compressor efficiency multiplier.
    pub const ETA_COMPRESSOR: usize = 1;
    /// Turbine efficiency multiplier.
    pub const ETA_TURBINE: usize = 2;
    /// First injector discharge coefficient; the four are contiguous.
    pub const INJECTOR: usize = 3;
    /// Radiator effectiveness multiplier.
    pub const RADIATOR: usize = 7;
    /// Head-to-coolant conductance multiplier.
    pub const HEAD_CONDUCTANCE: usize = 8;
    /// Oil supply multiplier.
    pub const OIL_SUPPLY: usize = 9;
}

/// Everything the filter and the display need to know about one parameter.
#[derive(Clone, Copy, Debug)]
pub struct Descriptor {
    /// Short name, as it appears in a readout.
    pub name: &'static str,
    /// Value on a healthy engine.
    pub nominal: f64,
    /// Lowest value that is a hypothesis rather than a divergence.
    pub lower: f64,
    /// Highest value that is a hypothesis rather than a divergence.
    pub upper: f64,
    /// Value at which the subsystem no longer meets its duty.
    pub failure: f64,
    /// Standard deviation of the random walk, per filter step.
    pub walk: f64,
    /// Standard deviation of the initial estimate.
    pub initial_sigma: f64,
}

/// The parameter set.
///
/// Bounds are asymmetric on purpose. A compressor whose efficiency has risen three
/// percent above the map it was fitted to is a modelling error, not a machine that
/// has improved, so the upper bound sits just above nominal and the lower bound is
/// where the range is. Failure thresholds are **estimated** except the injector,
/// which is the coefficient at which the nozzle can no longer deliver the smoke-
/// limited quantity at rated speed.
///
/// Initial standard deviations are tight because an engine is assumed to be as
/// delivered when the monitor is switched on. That is not an optimistic assumption,
/// it is the one that makes the estimates separable: volumetric efficiency and an
/// injector coefficient both reduce the fuel burned per unit of air, so along that
/// direction the measurements are nearly indifferent between them, and what decides
/// which one moves is how confidently each was known beforehand. Loosening the prior
/// on a well-fitted map parameter lets it absorb part of a fault that belongs
/// somewhere else, and the diagnosis then names two subsystems for one problem.
///
/// The random walk standard deviations set how fast the filter is willing to
/// believe the engine has changed. They are deliberately slow: an estimator tuned
/// to track the current state as closely as possible is not the one that
/// extrapolates a degradation trend, because it spends its freedom absorbing noise.
///
/// They also differ by an order of magnitude between the injectors and everything
/// else, and that difference is what isolates a fault rather than merely detecting
/// one. Coking runs over hours; compressor erosion, radiator fouling and bearing
/// wear run over hundreds. Letting the slow parameters move at the fast one's rate
/// lets them absorb part of a fast fault, and the diagnosis then names two
/// subsystems where the machine has one problem.
/// Keizers, Loendersloot & Tinga, International Journal of Prognostics and Health
/// Management 12(2), 2021.
/// <https://doi.org/10.36001/ijphm.2021.v12i2.2943>
pub const DESCRIPTORS: [Descriptor; PARAMS] = [
    Descriptor {
        name: "eta_vol",
        nominal: 1.0,
        lower: 0.75,
        upper: 1.05,
        failure: 0.85,
        walk: 4.0e-6,
        initial_sigma: 0.005,
    },
    Descriptor {
        name: "eta_c",
        nominal: 1.0,
        lower: 0.70,
        upper: 1.03,
        failure: 0.85,
        walk: 4.0e-6,
        initial_sigma: 0.005,
    },
    Descriptor {
        name: "eta_t",
        nominal: 1.0,
        lower: 0.70,
        upper: 1.03,
        failure: 0.85,
        walk: 4.0e-6,
        initial_sigma: 0.005,
    },
    injector(1),
    injector(2),
    injector(3),
    injector(4),
    Descriptor {
        name: "radiator",
        nominal: 1.0,
        lower: 0.50,
        upper: 1.05,
        failure: 0.70,
        walk: 4.0e-6,
        initial_sigma: 0.01,
    },
    Descriptor {
        name: "head_cond",
        nominal: 1.0,
        lower: 0.60,
        upper: 1.05,
        failure: 0.75,
        walk: 4.0e-6,
        initial_sigma: 0.01,
    },
    Descriptor {
        name: "oil_supply",
        nominal: 1.0,
        lower: 0.50,
        upper: 1.05,
        failure: 0.70,
        walk: 4.0e-6,
        initial_sigma: 0.01,
    },
];

/// One injector's descriptor.
///
/// Faster than the rest because coking is the fastest mechanism in the set: it
/// runs over hours where turbine erosion and radiator fouling run over hundreds.
const fn injector(cylinder: u8) -> Descriptor {
    Descriptor {
        name: match cylinder {
            1 => "injector-1 Cd",
            2 => "injector-2 Cd",
            3 => "injector-3 Cd",
            _ => "injector-4 Cd",
        },
        nominal: INJECTOR_CD_NOMINAL,
        lower: 0.35,
        upper: 1.00,
        failure: 0.62,
        walk: 5.0e-5,
        initial_sigma: 0.02,
    }
}

/// The estimate itself.
#[derive(Clone, Copy, Debug)]
pub struct Health {
    /// Current value of each parameter, in the order of [`DESCRIPTORS`].
    pub values: [f64; PARAMS],
}

impl Default for Health {
    fn default() -> Self {
        Self::nominal()
    }
}

impl Health {
    /// A healthy engine.
    #[must_use]
    pub fn nominal() -> Self {
        Self {
            values: std::array::from_fn(|i| DESCRIPTORS[i].nominal),
        }
    }

    /// Read the parameters out of a filter state vector.
    ///
    /// # Panics
    ///
    /// If the slice is shorter than [`PARAMS`].
    #[must_use]
    pub fn from_slice(v: &[f64]) -> Self {
        Self {
            values: std::array::from_fn(|i| v[i]),
        }
    }

    /// How far this parameter has travelled from nominal towards failure, 0 to 1.
    ///
    /// Zero on a healthy engine and one at the threshold. Above one the parameter
    /// has passed its limit, which is reported rather than clamped: an index that
    /// saturates at the limit cannot say how far past it a machine is.
    #[must_use]
    pub fn consumed(&self, i: usize) -> f64 {
        let d = &DESCRIPTORS[i];
        let span = d.nominal - d.failure;
        if span.abs() < f64::EPSILON {
            return 0.0;
        }
        ((d.nominal - self.values[i]) / span).max(0.0)
    }

    /// Apply the estimate to a set of engine parameters.
    ///
    /// The base parameters are the engine as it was fitted when new; the result is
    /// the engine as the filter currently believes it to be. Every sigma point is
    /// propagated through its own copy of this, which is why it takes a reference
    /// and returns an owned value rather than mutating in place.
    #[must_use]
    pub fn apply(&self, base: &EngineParams) -> EngineParams {
        let mut p = base.clone();
        p.cylinder.eta_vol_max *= self.values[index::ETA_VOL];
        p.compressor.eta_max *= self.values[index::ETA_COMPRESSOR];
        p.turbine.eta_max *= self.values[index::ETA_TURBINE];
        for i in 0..CYLINDERS {
            p.cylinder.injector_scale[i] *= self.values[index::INJECTOR + i] / INJECTOR_CD_NOMINAL;
        }
        p.cooling.radiator_effectiveness *= self.values[index::RADIATOR];
        p.thermal.head_conductance_w_per_k *= self.values[index::HEAD_CONDUCTANCE];
        // Pump wear, bearing clearance and a viscosity shift all land on the same
        // coefficient and none of them is separable from one gallery pressure, so
        // the lubrication circuit carries one lumped parameter rather than three
        // that could not be told apart. See `engine_model::oil`.
        p.oil.pressure_coefficient *= self.values[index::OIL_SUPPLY];
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nominal_parameters_leave_the_engine_untouched() {
        let base = engine_model::engines::ae330();
        let applied = Health::nominal().apply(&base);
        assert!((applied.cylinder.eta_vol_max - base.cylinder.eta_vol_max).abs() < 1e-15);
        assert!((applied.compressor.eta_max - base.compressor.eta_max).abs() < 1e-15);
        for i in 0..CYLINDERS {
            assert!(
                (applied.cylinder.injector_scale[i] - base.cylinder.injector_scale[i]).abs()
                    < 1e-15
            );
        }
        assert!((applied.oil.pressure_coefficient - base.oil.pressure_coefficient).abs() < 1e-15);
    }

    /// The simulator settles a coked injector at 0.84 of nominal flow. Expressed as
    /// a discharge coefficient that is 0.81, and the estimate on the wire has to be
    /// the same number as the flow scale in the plant, or the two halves of the
    /// system are describing the same fault with different arithmetic.
    #[test]
    fn a_coked_injector_reads_as_the_flow_scale_the_plant_applied() {
        let base = engine_model::engines::ae330();
        let mut health = Health::nominal();
        health.values[index::INJECTOR + 2] = 0.811;
        let applied = health.apply(&base);

        let scale = applied.cylinder.injector_scale[2] / base.cylinder.injector_scale[2];
        assert!((scale - 0.84).abs() < 2e-3, "flow scale {scale}");
        for i in [0, 1, 3] {
            assert!(
                (applied.cylinder.injector_scale[i] - base.cylinder.injector_scale[i]).abs()
                    < 1e-12,
                "cylinder {i} moved"
            );
        }
    }

    #[test]
    fn life_consumed_runs_from_nothing_at_nominal_to_all_of_it_at_the_threshold() {
        let i = index::INJECTOR + 2;
        let mut health = Health::nominal();
        assert!(health.consumed(i).abs() < 1e-12);

        health.values[i] = DESCRIPTORS[i].failure;
        assert!((health.consumed(i) - 1.0).abs() < 1e-12);

        health.values[i] = 0.811;
        let consumed = health.consumed(i);
        assert!((0.4..0.5).contains(&consumed), "{consumed}");
    }

    /// A parameter above its nominal is a modelling error rather than a machine in
    /// better condition than new, so the upper bounds sit at or just above nominal.
    #[test]
    fn every_bound_brackets_its_nominal_and_its_failure_threshold() {
        for d in &DESCRIPTORS {
            assert!(d.lower < d.failure, "{}", d.name);
            assert!(d.failure < d.nominal, "{}", d.name);
            assert!(d.nominal <= d.upper, "{}", d.name);
            assert!(d.upper - d.nominal < 0.06 * d.nominal, "{}", d.name);
        }
    }
}
