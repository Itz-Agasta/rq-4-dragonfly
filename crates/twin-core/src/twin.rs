//! The synchronisation loop: one filter step per telemetry frame.
//!
//! The augmented state carries the engine's own states, the health parameters, and
//! the lag states of the two instruments slow enough to matter. Health parameters
//! are modelled as a random walk, which is the standard way to estimate a constant
//! that is not quite constant: the walk's standard deviation is exactly the
//! statement of how fast the machine is believed to be able to change.
//!
//! # The instruments are part of the model
//!
//! A sheathed exhaust thermocouple lags the gas around it by seconds and a head
//! sensor by a fraction of one. Comparing an instantaneous prediction against a
//! lagged reading makes every transient look like a fault, and an estimator with
//! health parameters available will spend them explaining it. Carrying the lag as
//! filter state rather than filtering the mean prediction afterwards is what keeps
//! each sigma point consistent with its own history.
//!
//! # Transients are model error, not noise
//!
//! Model-plant mismatch grows with how hard the machine is being transiented, and
//! it is systematic rather than zero-mean, so it violates the assumption the filter
//! rests on. The measurement covariance is therefore scheduled against a transient
//! index: during a fuelling step the filter distrusts what it is told, the health
//! parameters barely move, and the residual band widens by exactly as much as the
//! extra uncertainty warrants.
//!
//! Borguet, Dewallef & Leonard, "A Way to Deal With Model-Plant Mismatch for a
//! Reliable Diagnosis in Transient Operation", Journal of Engineering for Gas
//! Turbines and Power 130(3), 2008. <https://doi.org/10.1115/1.2833491>
//!
//! Bounds on the parameters are applied to the estimate and not to the covariance,
//! following the constrained augmented-state treatment in Assfalg, Allgower & Fritz,
//! "Constrained derivative-free augmented state estimation for a diesel engine air
//! path", IFAC Proceedings 39(2), 2006.
//! <https://doi.org/10.3182/20060329-3-au-2901.00224>

use engine_model::{CYLINDERS, EngineParams, Inputs, State, integrator};
use nalgebra::{DMatrix, DVector};
use serde::Serialize;

use crate::channels::{self, CHANNELS, Channel, Measurement, TABLE};
use crate::detect::{Detection, Detector};
use crate::health::{self, DESCRIPTORS, Health, PARAMS};
use crate::indices::{self, INDICES, Scored};
use crate::nominal::Nominal;
use crate::signature::{Diagnosis, Signatures};
use crate::ukf::{FilterError, Spread, Ukf};

/// Layout of the augmented state.
mod slot {
    use super::{CYLINDERS, PARAMS};
    /// Health parameters occupy the first block.
    pub const THETA: usize = 0;
    /// Intake manifold pressure.
    pub const P_IM: usize = PARAMS;
    /// Exhaust manifold pressure.
    pub const P_EM: usize = PARAMS + 1;
    /// Crankshaft speed, rad/s.
    pub const OMEGA_E: usize = PARAMS + 2;
    /// Turbocharger shaft speed, rad/s.
    pub const OMEGA_TC: usize = PARAMS + 3;
    /// Cylinder head metal temperatures.
    pub const T_CHT: usize = PARAMS + 4;
    /// Coolant temperature.
    pub const T_COOLANT: usize = PARAMS + 4 + CYLINDERS;
    /// Oil temperature.
    pub const T_OIL: usize = PARAMS + 5 + CYLINDERS;
    /// Exhaust thermocouple readings.
    pub const EGT_SENSOR: usize = PARAMS + 6 + CYLINDERS;
    /// Head sensor readings.
    pub const CHT_SENSOR: usize = PARAMS + 6 + 2 * CYLINDERS;
    /// Total dimension.
    pub const DIM: usize = PARAMS + 6 + 3 * CYLINDERS;
}

/// Exhaust thermocouple time constant, s. **estimated** for a thin sheathed probe
/// in a small-bore runner. An installation property with a very wide range: one
/// 1.6 mm sheathed rake measures 21 s on a bench and 7 s in an engine, where gas
/// velocity lifts the film coefficient. Whatever the number, the source of the
/// measurements and this filter have to agree on it, or every transient reads as a
/// fault. George & Muthuveerappan, Journal of Aerospace Sciences and Technologies
/// 71(4), 2023. <https://doi.org/10.61653/joast.v71i4.2019.174>
const EGT_TAU_S: f64 = 2.0;

/// Head sensor time constant, s. **estimated**. Small, because the metal it is
/// bonded to is already the slow node.
const CHT_TAU_S: f64 = 0.5;

/// Simulated seconds the model is run before the first estimate is taken.
///
/// Exhaust manifold pressure is the one state no instrument on this engine reports,
/// so it has to be found rather than read. Settling the model at the first observed
/// operating point costs a millisecond and is the difference between a twin that is
/// locked from the first frame and one that spends a minute converging in front of
/// whoever is watching.
const WARMUP_S: f64 = 20.0;

/// Length of the window the synchronisation quality is averaged over, s.
const QUALITY_WINDOW_S: f64 = 10.0;

/// How long the transient index remembers a step, s.
///
/// The measurement covariance has to stay inflated for as long as the model and
/// the plant actually disagree, and that is not the instant of the step: it is the
/// turbocharger's spool, which runs about a second and a half. An index computed
/// only from the current rate of change collapses to zero the moment the lever
/// stops moving, while the manifolds are still filling, and the estimator spends
/// those seconds attributing the mismatch to compressor and turbine efficiency.
const TRANSIENT_MEMORY_S: f64 = 3.0;

/// Relative residual below which the twin is considered locked, percent.
const LOCK_RMS_PCT: f64 = 2.0;

/// Process noise, one standard deviation per filter step.
///
/// These say how much the filter believes each state can change between frames for
/// reasons the model does not describe. They are not physical constants and they
/// are the only genuinely free parameters in the whole estimator.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    /// Manifold pressures, Pa.
    pub pressure_pa: f64,
    /// Crankshaft speed, rad/s. Carries all of the speed variability, because the
    /// twin holds speed rather than predicting it; see [`Twin::propagate`].
    pub omega_e: f64,
    /// Turbocharger speed, rad/s.
    pub omega_tc: f64,
    /// Metal, coolant and oil temperatures, K.
    pub temperature_k: f64,
    /// Instrument lag states, K.
    pub sensor_k: f64,
    /// Transient index above which the measurement covariance starts to inflate.
    pub transient_knee: f64,
    /// How hard the covariance inflates with the transient index.
    pub transient_gain: f64,
    /// Largest factor the measurement covariance may be inflated by.
    ///
    /// Without a ceiling a hard fuelling step inflates it by four orders of
    /// magnitude, which runs the filter open loop for the whole spool and then
    /// corrects the accumulated state error in one step once the index decays.
    /// The correction lands on whichever parameter is cheapest, and it moved the
    /// thermal estimate five percent. A hundred is enough to stop a transient
    /// being read as degradation and small enough that the filter keeps tracking.
    pub transient_ceiling: f64,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            pressure_pa: 250.0,
            omega_e: 0.5,
            omega_tc: 60.0,
            temperature_k: 0.05,
            sensor_k: 0.05,
            transient_knee: 1.0,
            transient_gain: 40.0,
            transient_ceiling: 100.0,
        }
    }
}

/// What one synchronisation step produced.
#[derive(Clone, Debug, Serialize)]
pub struct TwinOutput {
    /// Whether the residual has been small enough for long enough to be trusted.
    pub locked: bool,
    /// Whether lock has ever been held this mission. Latches true, never clears.
    ///
    /// A client attaching mid-mission cannot otherwise tell a twin still
    /// converging from one that held an estimate and lost it, because the edge
    /// between them may predate its socket. The two want opposite things said.
    pub ever_locked: bool,
    /// Root mean square residual across channels, percent of their references.
    pub rms_pct: f64,
    /// How hard the engine is being transiented, 0 at steady state.
    pub transient: f64,
    /// What the model said each channel would read.
    pub predicted: [f64; CHANNELS],
    /// Measurement less prediction, in each channel's own units.
    pub residual: [f64; CHANNELS],
    /// One standard deviation of each residual, from the innovation covariance.
    pub sigma: [f64; CHANNELS],
    /// Residual in standard deviations, which is what a band is drawn against.
    pub normalised: [f64; CHANNELS],
    /// Health parameter estimates.
    pub theta: [f64; PARAMS],
    /// One standard deviation of each health parameter estimate.
    pub theta_sigma: [f64; PARAMS],
    /// Root mean square innovation, percent. How well the filter is tracking.
    pub innovation_pct: f64,
    /// Subsystem health indices, 0 to 100.
    pub health: [f64; INDICES],
    /// Name of the quantity that set each index.
    pub health_driver: [&'static str; INDICES],
    /// Current value of that quantity.
    pub health_driver_value: [f64; INDICES],
    /// Value of that quantity at which the subsystem fails.
    pub health_driver_limit: [f64; INDICES],
    /// What the anomaly tests made of this frame, and how far ahead of the
    /// conventional redline monitor they are.
    pub detection: Detection,
    /// Which fault the residual pattern points at. Only meaningful once
    /// `detection` says there is something to isolate; see `signature`.
    pub diagnosis: Diagnosis,
}

/// The estimator.
#[derive(Debug)]
pub struct Twin {
    params: EngineParams,
    tuning: Tuning,
    filter: Option<Ukf>,
    nominal: Nominal,
    process_noise: DMatrix<f64>,
    last: Option<(f64, f64, f64)>,
    transient: f64,
    quality: Vec<f64>,
    locked_since: Option<f64>,
    detector: Detector,
    signatures: Signatures,
    output: TwinOutput,
}

impl Twin {
    /// A twin of an engine with the given parameters, not yet seeded.
    #[must_use]
    pub fn new(params: EngineParams) -> Self {
        Self::with_tuning(params, Tuning::default())
    }

    /// A twin with explicit process noise.
    #[must_use]
    pub fn with_tuning(params: EngineParams, tuning: Tuning) -> Self {
        let mut q = DVector::zeros(slot::DIM);
        for i in 0..PARAMS {
            q[slot::THETA + i] = DESCRIPTORS[i].walk.powi(2);
        }
        q[slot::P_IM] = tuning.pressure_pa.powi(2);
        q[slot::P_EM] = tuning.pressure_pa.powi(2);
        q[slot::OMEGA_E] = tuning.omega_e.powi(2);
        q[slot::OMEGA_TC] = tuning.omega_tc.powi(2);
        for i in 0..CYLINDERS {
            q[slot::T_CHT + i] = tuning.temperature_k.powi(2);
            q[slot::EGT_SENSOR + i] = tuning.sensor_k.powi(2);
            q[slot::CHT_SENSOR + i] = tuning.sensor_k.powi(2);
        }
        q[slot::T_COOLANT] = tuning.temperature_k.powi(2);
        q[slot::T_OIL] = tuning.temperature_k.powi(2);

        Self {
            nominal: Nominal::new(params.clone()),
            // Generating the signature matrix runs the model to steady state once
            // per hypothesis. In release that is tens of milliseconds at startup;
            // in a debug build it is seconds, which is why `just core` builds
            // release. It is done here rather than lazily so that a matrix and the
            // parameters it was generated from can never disagree.
            signatures: Signatures::generate(&params),
            detector: Detector::new(),
            params,
            tuning,
            filter: None,
            process_noise: DMatrix::from_diagonal(&q),
            last: None,
            transient: 0.0,
            quality: Vec::new(),
            locked_since: None,
            output: blank_output(),
        }
    }

    /// The most recent output, whether or not the last frame was usable.
    #[must_use]
    pub fn output(&self) -> &TwinOutput {
        &self.output
    }

    /// Whether the twin has an estimate at all.
    #[must_use]
    pub fn is_seeded(&self) -> bool {
        self.filter.is_some()
    }

    /// The engine state the filter currently believes in, or `None` before it has
    /// an estimate.
    ///
    /// Deliberately not on [`TwinOutput`]. Every field of that struct is on the
    /// wire twenty times a second, and no display draws a manifold pressure the
    /// filter inferred rather than measured. This exists for the one caller that
    /// needs a starting point to integrate the model forward from, and it hands
    /// back the mean alone: a projection that carried the covariance would be
    /// claiming an interval the model run does not produce.
    #[must_use]
    pub fn state(&self) -> Option<State> {
        self.filter.as_ref().map(|f| unpack(f.mean()))
    }

    /// Advance one telemetry frame.
    ///
    /// `Ok(None)` means the measurement was unusable and nothing advanced, so no
    /// estimate belongs to that frame. Handing the previous output back instead
    /// would let a caller pair an old diagnosis with a new measurement, which is
    /// the one failure a residual display cannot reveal.
    ///
    /// # Errors
    ///
    /// If the filter loses positive definiteness or the model returns a non-finite
    /// value. The estimate is discarded on either, so the next usable measurement
    /// re-seeds rather than continuing from a state known to be wrong.
    pub fn update(&mut self, m: &Measurement) -> Result<Option<&TwinOutput>, FilterError> {
        if !m.is_usable() {
            return Ok(None);
        }
        let dt = self.step_seconds(m);
        if self.filter.is_none() {
            self.seed(m);
        }

        self.transient = self
            .transient_index(m, dt)
            .max(self.transient * (-dt / TRANSIENT_MEMORY_S).exp());
        let transient = self.transient;
        self.last = Some((m.t_s, self.fuel_command(m), m.rpm));

        let inputs = self.inputs(m);
        let base = self.params.clone();
        let result = self.step(m, &base, inputs, dt, transient);

        match result {
            Ok(()) => {
                self.output.transient = transient;
                Ok(Some(&self.output))
            }
            Err(e) => {
                self.filter = None;
                self.nominal.reset();
                self.locked_since = None;
                self.transient = 0.0;
                self.quality.clear();
                // The latch outlives the reset: a filter that failed after
                // holding an estimate is the case `ever_locked` exists to report.
                let held = self.output.ever_locked;
                self.output = blank_output();
                self.output.ever_locked = held;
                Err(e)
            }
        }
    }

    /// Predict, correct, and record the result.
    fn step(
        &mut self,
        m: &Measurement,
        base: &EngineParams,
        u: Inputs,
        dt: f64,
        transient: f64,
    ) -> Result<(), FilterError> {
        let inflation = (1.0
            + self.tuning.transient_gain
                * (transient - self.tuning.transient_knee).max(0.0).powi(2))
        .min(self.tuning.transient_ceiling);
        let z = DVector::from_column_slice(&m.vector());

        let (mean, innovation) = {
            let filter = self.filter.as_mut().expect("seeded above");
            filter.predict(|x| propagate(base, x, &u, dt), &self.process_noise)?;

            let r = DMatrix::from_diagonal(&DVector::from_iterator(
                CHANNELS,
                TABLE.iter().map(|c: &Channel| c.variance() * inflation),
            ));
            let innovation = filter.update(&z, |x| observe(base, x, &u), &r)?;

            for (i, d) in DESCRIPTORS.iter().enumerate() {
                filter.clamp_element(slot::THETA + i, d.lower, d.upper);
            }
            (filter.mean().clone(), innovation)
        };

        let filter = self.filter.as_ref().expect("seeded above");
        for (i, (value, sigma)) in self
            .output
            .theta
            .iter_mut()
            .zip(self.output.theta_sigma.iter_mut())
            .enumerate()
        {
            *value = mean[slot::THETA + i];
            *sigma = filter.sigma(slot::THETA + i);
        }

        // What a healthy engine would read, from the estimated operating point.
        // This and not the innovation is what a residual display is fed; see the
        // `nominal` module for why an adapted twin cannot answer that question.
        let predicted = self.nominal.predict(&unpack(&mean), &u, dt);
        for (i, channel) in TABLE.iter().enumerate() {
            let sigma = (channel.variance() * inflation).sqrt();
            let residual = z[i] - predicted[i];
            self.output.predicted[i] = predicted[i];
            self.output.residual[i] = residual;
            self.output.sigma[i] = sigma;
            self.output.normalised[i] = residual / sigma;
        }

        self.output.detection = self.detector.update(
            m.t_s,
            &self.output.normalised,
            m,
            &self.params.limits.redline,
        );
        // Isolation runs on the residual pattern only once detection says there is
        // something to isolate. Asked "which fault" about a healthy engine it names
        // whichever one the noise happens to lie nearest; see `signature::diagnose`.
        let flagged = self.output.detection.drift || self.output.detection.anomaly;
        self.output.diagnosis = self.signatures.diagnose(&self.output.normalised, flagged);

        let health = Health::from_slice(&self.output.theta);
        let unexplained = innovation.normalised();
        let scored: [Scored; INDICES] = indices::evaluate(
            &health,
            unexplained.as_slice(),
            &m.auxiliary(self.params.limits.rated_power_w),
        );
        for (i, s) in scored.iter().enumerate() {
            self.output.health[i] = s.value;
            self.output.health_driver[i] = s.driver;
            self.output.health_driver_value[i] = s.driver_value;
            self.output.health_driver_limit[i] = s.driver_limit;
        }

        self.record_quality(m.t_s, dt, innovation.residual.as_slice());
        Ok(())
    }

    /// Seconds since the previous frame, floored and capped.
    ///
    /// A gap longer than a second means frames were lost, and propagating the model
    /// across it would put the estimate somewhere the engine never was. The step is
    /// capped instead, which leaves the filter briefly behind rather than wrong.
    fn step_seconds(&self, m: &Measurement) -> f64 {
        match self.last {
            Some((t, _, _)) => (m.t_s - t).clamp(integrator::DT, 1.0),
            None => integrator::DT,
        }
    }

    /// How hard the engine is being transiented.
    ///
    /// Fuelling rate and speed rate, each against a reference rate that puts an
    /// ordinary manoeuvre near one. Squared and summed so a large excursion on
    /// either dominates, which is what the measurement covariance should respond to.
    fn transient_index(&self, m: &Measurement, dt: f64) -> f64 {
        const FUEL_RATE_REF: f64 = 0.20;
        const RPM_RATE_REF: f64 = 120.0;
        let Some((_, fuel, rpm)) = self.last else {
            return 0.0;
        };
        let d_fuel = (self.fuel_command(m) - fuel).abs() / dt / FUEL_RATE_REF;
        let d_rpm = (m.rpm - rpm).abs() / dt / RPM_RATE_REF;
        d_fuel.hypot(d_rpm)
    }

    /// Fuelling command implied by the broadcast injection duration.
    ///
    /// Taken from the duration rather than from the throttle position field, which
    /// is broadcast as a whole percent. At cruise that quantisation is a constant
    /// offset of up to one part in seventy on the load actuator, and a constant
    /// input offset is precisely what an estimator with health parameters absorbs:
    /// it would show as every injector being slightly off nominal, on a healthy
    /// engine, forever.
    fn fuel_command(&self, m: &Measurement) -> f64 {
        let commanded_mg = m.injection_ms * self.params.cylinder.injector_flow_g_per_s;
        (commanded_mg / self.params.cylinder.u_f_max_mg).clamp(0.0, 1.0)
    }

    /// Everything acting on the engine from outside it, from the bus.
    fn inputs(&self, m: &Measurement) -> Inputs {
        Inputs {
            fuel_cmd: self.fuel_command(m),
            wastegate: m.wastegate.clamp(0.0, 1.0),
            p_amb: m.p_amb_pa,
            t_amb: m.oat_k,
            tas_m_s: m.tas_m_s(),
            // The propeller is not the engine's, and modelling it here would make
            // the twin depend on the airframe it happens to be bolted to. Speed is
            // held instead; see `propagate`.
            load_torque: 0.0,
        }
    }

    /// Build the initial estimate from one measurement.
    fn seed(&mut self, m: &Measurement) {
        let mut x = State {
            p_im: m.map_pa,
            p_em: m.map_pa,
            omega_e: m.rpm * std::f64::consts::TAU / 60.0,
            omega_tc: m.turbo_rpm * std::f64::consts::TAU / 60.0,
            t_cht: m.cht_k,
            t_coolant: m.coolant_t_k,
            t_oil: m.oil_t_k,
        };
        let u = self.inputs(m);

        // Settle the states no instrument reports, then put the measured ones back.
        // Only exhaust manifold pressure is actually being solved for; letting the
        // rest drift during the warm-up and then overwriting them keeps the seed
        // anchored to what was observed.
        let steps = (WARMUP_S / integrator::DT) as u32;
        for _ in 0..steps {
            let held = x.omega_e;
            x = engine_model::step(&self.params, &x, &u, integrator::DT);
            x.omega_e = held;
        }
        x.p_im = m.map_pa;
        x.omega_tc = m.turbo_rpm * std::f64::consts::TAU / 60.0;
        x.t_cht = m.cht_k;
        x.t_coolant = m.coolant_t_k;
        x.t_oil = m.oil_t_k;

        let mut mean = DVector::zeros(slot::DIM);
        let mut variance = DVector::zeros(slot::DIM);
        for i in 0..PARAMS {
            mean[slot::THETA + i] = DESCRIPTORS[i].nominal;
            variance[slot::THETA + i] = DESCRIPTORS[i].initial_sigma.powi(2);
        }
        mean[slot::P_IM] = x.p_im;
        mean[slot::P_EM] = x.p_em;
        mean[slot::OMEGA_E] = x.omega_e;
        mean[slot::OMEGA_TC] = x.omega_tc;
        variance[slot::P_IM] = 2000.0f64.powi(2);
        variance[slot::P_EM] = 20_000.0f64.powi(2);
        variance[slot::OMEGA_E] = 2.0f64.powi(2);
        variance[slot::OMEGA_TC] = 500.0f64.powi(2);
        for i in 0..CYLINDERS {
            mean[slot::T_CHT + i] = x.t_cht[i];
            mean[slot::EGT_SENSOR + i] = m.egt_k[i];
            mean[slot::CHT_SENSOR + i] = m.cht_k[i];
            variance[slot::T_CHT + i] = 4.0;
            variance[slot::EGT_SENSOR + i] = 25.0;
            variance[slot::CHT_SENSOR + i] = 4.0;
        }
        mean[slot::T_COOLANT] = x.t_coolant;
        mean[slot::T_OIL] = x.t_oil;
        variance[slot::T_COOLANT] = 4.0;
        variance[slot::T_OIL] = 4.0;

        self.filter = Some(Ukf::new(
            mean,
            DMatrix::from_diagonal(&variance),
            Spread::default(),
        ));
    }

    /// Update the rolling synchronisation quality and the lock state.
    ///
    /// Measured on the filter's innovation rather than on the residual against a
    /// healthy engine, because these answer different questions. The innovation
    /// says whether the twin is tracking the machine in front of it; the residual
    /// says whether that machine is healthy. A degraded engine that the twin has
    /// understood is well synchronised and unwell, and one number cannot say both.
    fn record_quality(&mut self, t_s: f64, dt: f64, innovation: &[f64]) {
        let sum: f64 = (0..CHANNELS)
            .map(|i| (innovation[i] / TABLE[i].reference).powi(2))
            .sum();
        let rms = (sum / CHANNELS as f64).sqrt() * 100.0;
        let physics: f64 = (0..CHANNELS)
            .map(|i| (self.output.residual[i] / TABLE[i].reference).powi(2))
            .sum();
        self.output.rms_pct = (physics / CHANNELS as f64).sqrt() * 100.0;

        let capacity = (QUALITY_WINDOW_S / dt.max(integrator::DT)) as usize;
        self.quality.push(rms);
        if self.quality.len() > capacity {
            let excess = self.quality.len() - capacity;
            self.quality.drain(0..excess);
        }
        let window = self.quality.iter().sum::<f64>() / self.quality.len() as f64;
        self.output.innovation_pct = window;

        // Locked once the residual has been small for a whole window, not the
        // instant it first is. A single quiet frame proves nothing, and the claim
        // the fault story rests on is that the twin was tracking before onset.
        if window < LOCK_RMS_PCT {
            let since = *self.locked_since.get_or_insert(t_s);
            self.output.locked = t_s - since >= QUALITY_WINDOW_S;
        } else {
            self.locked_since = None;
            self.output.locked = false;
        }
        self.output.ever_locked |= self.output.locked;
    }
}

/// Advance one sigma point by `dt`.
///
/// Crankshaft speed is restored after each sub-step rather than integrated. A
/// constant-speed propeller under a governor absorbs whatever the engine makes, so
/// the load torque that would close the speed equation is a property of the
/// airframe and is not broadcast. Holding speed and letting the measurement move it
/// is honest about that: the twin claims no knowledge of the propeller, and the
/// speed channel carries no information about engine health, which on a governed
/// installation is true.
fn propagate(base: &EngineParams, x: &DVector<f64>, u: &Inputs, dt: f64) -> DVector<f64> {
    let health = Health::from_slice(x.as_slice());
    let params = health.apply(base);
    let mut state = unpack(x);

    let steps = (dt / integrator::DT).round().max(1.0) as u32;
    let h = dt / f64::from(steps);
    for _ in 0..steps {
        let held = state.omega_e;
        state = engine_model::step(&params, &state, u, h);
        state.omega_e = held;
    }

    let mut next = x.clone();
    pack(&mut next, &state);

    // The instruments are driven by where the engine ended up. A zero-order hold
    // over one frame is exact to within a part in a thousand for a two-second time
    // constant, and it costs one evaluation rather than one per sub-step.
    let outputs = engine_model::evaluate(&params, &state, u);
    let egt_alpha = 1.0 - (-dt / EGT_TAU_S).exp();
    let cht_alpha = 1.0 - (-dt / CHT_TAU_S).exp();
    for i in 0..CYLINDERS {
        let egt = x[slot::EGT_SENSOR + i];
        let cht = x[slot::CHT_SENSOR + i];
        next[slot::EGT_SENSOR + i] = egt_alpha.mul_add(outputs.t_egt[i] - egt, egt);
        next[slot::CHT_SENSOR + i] = cht_alpha.mul_add(state.t_cht[i] - cht, cht);
    }
    next
}

/// What the instruments would read at one sigma point.
fn observe(base: &EngineParams, x: &DVector<f64>, u: &Inputs) -> DVector<f64> {
    let health = Health::from_slice(x.as_slice());
    let params = health.apply(base);
    let state = unpack(x);
    let o = engine_model::evaluate(&params, &state, u);

    let mut z = DVector::zeros(CHANNELS);
    z[channels::index::RPM] = state.rpm();
    z[channels::index::MAP] = state.p_im;
    z[channels::index::MAT] = o.t_intake;
    z[channels::index::MAF] = o.w_air;
    z[channels::index::TURBO] = state.turbo_rpm();
    z[channels::index::TORQUE] = o.torque_brake;
    z[channels::index::FUEL_FLOW] = o.w_fuel * 3600.0;
    z[channels::index::OIL_PRESSURE] = o.p_oil;
    z[channels::index::OIL_TEMPERATURE] = state.t_oil;
    z[channels::index::COOLANT] = state.t_coolant;
    for i in 0..CYLINDERS {
        z[channels::index::EGT + i] = x[slot::EGT_SENSOR + i];
        z[channels::index::CHT + i] = x[slot::CHT_SENSOR + i];
        // Excess air ratio is unbounded above at zero fuelling, which is physically
        // right and useless to a filter. Clamped to the range an instrument would
        // report rather than allowed to reach infinity and poison the covariance.
        z[channels::index::LAMBDA + i] = o.lambda_cylinder[i].clamp(0.5, 20.0);
    }
    z
}

fn unpack(x: &DVector<f64>) -> State {
    State {
        p_im: x[slot::P_IM].max(1.0),
        p_em: x[slot::P_EM].max(1.0),
        omega_e: x[slot::OMEGA_E].max(0.0),
        omega_tc: x[slot::OMEGA_TC].max(1.0),
        t_cht: std::array::from_fn(|i| x[slot::T_CHT + i].max(1.0)),
        t_coolant: x[slot::T_COOLANT].max(1.0),
        t_oil: x[slot::T_OIL].max(1.0),
    }
}

fn pack(x: &mut DVector<f64>, state: &State) {
    x[slot::P_IM] = state.p_im;
    x[slot::P_EM] = state.p_em;
    x[slot::OMEGA_E] = state.omega_e;
    x[slot::OMEGA_TC] = state.omega_tc;
    for i in 0..CYLINDERS {
        x[slot::T_CHT + i] = state.t_cht[i];
    }
    x[slot::T_COOLANT] = state.t_coolant;
    x[slot::T_OIL] = state.t_oil;
}

fn blank_output() -> TwinOutput {
    TwinOutput {
        locked: false,
        ever_locked: false,
        rms_pct: f64::NAN,
        transient: 0.0,
        predicted: [f64::NAN; CHANNELS],
        residual: [f64::NAN; CHANNELS],
        sigma: [f64::NAN; CHANNELS],
        normalised: [f64::NAN; CHANNELS],
        theta: std::array::from_fn(|i| health::DESCRIPTORS[i].nominal),
        theta_sigma: [f64::NAN; PARAMS],
        innovation_pct: f64::NAN,
        detection: Detection::default(),
        // A blank diagnosis is the null hypothesis at certainty, not an empty one:
        // before the first frame the honest statement about the engine is that
        // nothing has been found wrong with it.
        diagnosis: Diagnosis {
            posterior: {
                let mut p = [0.0; crate::signature::HYPOTHESES];
                p[crate::signature::NOMINAL] = 1.0;
                p
            },
            match_score: [0.0; crate::signature::HYPOTHESES],
            best: crate::signature::NOMINAL,
            unexplained: false,
            rejection: [""; crate::signature::HYPOTHESES],
        },
        health: [f64::NAN; INDICES],
        health_driver: [""; INDICES],
        health_driver_value: [f64::NAN; INDICES],
        health_driver_limit: [f64::NAN; INDICES],
    }
}
