//! The twin against a plant, with no bus, no daemon and no serialisation.
//!
//! The plant here is the engine model driven the way an airframe drives it: a
//! governor holding crankshaft speed, a boost controller working the wastegate, and
//! instruments that lag, round and add noise the way the ones on the bus do. That
//! last part matters more than it looks. Two of the fields an engine controller
//! broadcasts are whole numbers, and a rounded input is a constant offset rather
//! than zero-mean noise, which is exactly the thing an estimator with health
//! parameters will quietly absorb.
//!
//! Everything asserted here is asserted again against a live bus. This exists so a
//! tuning change can be evaluated in a second rather than in a minute.

use dragonfly_sim::fault::{
    CoolingDegradation, Faults, InjectorCoking, Misfire, SensorDrift, SensorFreeze,
};
use engine_model::{EngineParams, Inputs, State, atmosphere, control, integrator};
use twin_core::channels::index as ch;
use twin_core::health::index as th;
use twin_core::{Measurement, Twin};

/// Rated brake power, W. The denominator of the load percentage on the bus.
const RATED_W: f64 = 132_000.0;

/// Injector flow scale a coked nozzle settles at, matching the simulator.
const COKED_SCALE: f64 = 0.84;

/// Fraction of one cylinder's firings that fail, at the settled misfire.
const MISFIRE_RATE: f64 = 0.20;

/// Radiator effectiveness the fouling fault settles at.
const FOULED_SCALE: f64 = 0.83;

/// Exhaust probe drift rate, K/h. Fast enough to reach several sigma inside the
/// few hundred seconds these tests run, which is a signal-chain fault rather than
/// probe oxidation; `dragonfly_sim::fault::SensorDrift` has the distinction.
const DRIFT_K_PER_H: f64 = 120.0;

/// A deterministic generator, so a failing run can be reproduced exactly.
struct Rng(u64);

impl Rng {
    fn normal(&mut self) -> f64 {
        let mut next = || {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
        };
        let u1: f64 = next().max(f64::MIN_POSITIVE);
        let u2: f64 = next();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

/// The engine, its controller, and the instruments watching it.
struct Plant {
    /// The engine as delivered. Faults are severities against this, never against
    /// the degraded set, so nothing compounds step to step.
    base: EngineParams,
    params: EngineParams,
    state: State,
    boost: control::BoostController,
    egt_lag: [f64; engine_model::CYLINDERS],
    cht_lag: [f64; engine_model::CYLINDERS],
    rng: Rng,
    /// Storage for a frozen probe's held sample. See `Faults::corrupt_exhaust`.
    held_egt: Option<f64>,
    t_s: f64,
}

/// Where the engine is flown for these tests: the canonical cruise point.
struct Condition {
    altitude_m: f64,
    oat_k: f64,
    ias_m_s: f64,
    fuel_cmd: f64,
}

impl Condition {
    fn cruise() -> Self {
        Self {
            altitude_m: 6828.0,
            oat_k: 242.15,
            ias_m_s: 40.1,
            fuel_cmd: 0.38,
        }
    }

    /// Hot, low, slow and near full power: the case `engine_model::thermal` names
    /// as the binding one for a liquid-cooled engine. ISA+30 at 2,000 ft.
    ///
    /// It exists because the cooling fault is barely observable anywhere else. At
    /// cruise the thermostat is only about a fifth open, so it answers a fouled
    /// radiator by opening further and the coolant temperature hardly moves; the
    /// loop has so much reserve authority that the fault hides inside it. Here the
    /// thermostat is against its stop and the reserve is gone.
    fn hot_and_low() -> Self {
        Self {
            altitude_m: 610.0,
            oat_k: atmosphere::isa(610.0).t + 30.0,
            ias_m_s: 45.0,
            fuel_cmd: 0.90,
        }
    }

    fn ambient(&self) -> (f64, f64) {
        let a = atmosphere::isa(self.altitude_m);
        (a.p, self.oat_k)
    }

    fn tas(&self) -> f64 {
        let (p, t) = self.ambient();
        let rho = p / (287.05 * t);
        self.ias_m_s * (1.225 / rho).sqrt()
    }
}

impl Plant {
    fn new(c: &Condition) -> Self {
        let params = engine_model::engines::ae330();
        let (p_amb, _) = c.ambient();
        Self {
            state: State {
                p_im: p_amb * 2.5,
                p_em: p_amb * 2.0,
                ..State::at_rest(p_amb, 3720.0)
            },
            base: params.clone(),
            params,
            boost: control::BoostController::new(),
            egt_lag: [0.0; engine_model::CYLINDERS],
            cht_lag: [0.0; engine_model::CYLINDERS],
            rng: Rng(0x5EED_1234),
            held_egt: None,
            t_s: 0.0,
        }
    }

    /// Advance to the next telemetry instant and report what the bus would carry.
    ///
    /// Faults arrive through the simulator's own [`Faults::apply`] rather than by
    /// poking a parameter here. A test that reimplements the fault it is testing
    /// tests the engine model and not the injection path, and the two drift apart
    /// without anything failing.
    fn advance(&mut self, c: &Condition, dt: f64, faults: &Faults) -> Measurement {
        let (p_amb, t_amb) = c.ambient();
        faults.apply(&mut self.params, &self.base, self.t_s);

        let mut u = Inputs {
            fuel_cmd: c.fuel_cmd,
            wastegate: 0.0,
            p_amb,
            t_amb,
            tas_m_s: c.tas(),
            load_torque: 0.0,
        };

        let steps = (dt / integrator::DT).round() as u32;
        for _ in 0..steps {
            u.wastegate = self.boost.update(
                &self.params,
                u.fuel_cmd,
                self.state.p_im,
                self.state.omega_tc,
                integrator::DT,
            );
            // A constant-speed unit absorbs exactly what the engine makes, which is
            // what holds crankshaft speed while the turbocharger finds its own.
            u.load_torque = engine_model::evaluate(&self.params, &self.state, &u).torque_brake;
            self.state = engine_model::step(&self.params, &self.state, &u, integrator::DT);
        }
        self.t_s += dt;

        let o = engine_model::evaluate(&self.params, &self.state, &u);
        let settle = self.t_s <= dt;
        for i in 0..engine_model::CYLINDERS {
            let (egt_alpha, cht_alpha) = if settle {
                (1.0, 1.0)
            } else {
                (1.0 - (-dt / 2.0f64).exp(), 1.0 - (-dt / 0.5f64).exp())
            };
            self.egt_lag[i] += egt_alpha * (o.t_egt[i] - self.egt_lag[i]);
            self.cht_lag[i] += cht_alpha * (self.state.t_cht[i] - self.cht_lag[i]);
        }

        // Two fields on the bus are whole numbers, so they are rounded here too.
        let load_pct = (o.power_brake_w / RATED_W * 100.0).round();
        let rpm = self.state.rpm().round();

        let mut egt_k = std::array::from_fn(|i| self.egt_lag[i] + 1.6 * self.rng.normal());
        faults.corrupt_exhaust(&mut egt_k, &mut self.held_egt, self.t_s);

        Measurement {
            t_s: self.t_s,
            p_amb_pa: p_amb,
            oat_k: t_amb,
            ias_m_s: c.ias_m_s,
            wastegate: u.wastegate,
            injection_ms: o.u_f_mg / self.params.cylinder.injector_flow_g_per_s,
            rpm,
            map_pa: self.state.p_im + 250.0 * self.rng.normal(),
            mat_k: o.t_intake + 0.8 * self.rng.normal(),
            maf_kg_s: o.w_air + 3.0e-4 * self.rng.normal(),
            turbo_rpm: self.state.turbo_rpm() + 200.0 * self.rng.normal(),
            torque_nm: load_pct / 100.0 * RATED_W / self.state.omega_e,
            fuel_flow_kg_h: o.w_fuel * 3600.0,
            oil_p_pa: o.p_oil + 1500.0 * self.rng.normal(),
            oil_t_k: self.state.t_oil + 0.8 * self.rng.normal(),
            coolant_t_k: self.state.t_coolant + 0.8 * self.rng.normal(),
            egt_k,
            cht_k: std::array::from_fn(|i| self.cht_lag[i] + 0.8 * self.rng.normal()),
            lambda: o.lambda_cylinder,
            bus_v: 27.8,
            vib_rms_g: 1.2,
            vib_kurtosis: 2.0,
        }
    }
}

/// A healthy engine.
fn healthy() -> Faults {
    Faults::default()
}

/// The demonstration fault: injector 3 coking to 84% of nominal flow.
fn coking() -> Faults {
    Faults {
        injector: Some(InjectorCoking {
            cylinder: 2,
            onset_s: 30.0,
            ramp_s: 120.0,
            final_scale: COKED_SCALE,
        }),
        ..Faults::default()
    }
}

/// The same fault applied as a step, for the before-and-after comparison.
fn step_coking() -> Faults {
    Faults {
        injector: Some(InjectorCoking {
            cylinder: 2,
            onset_s: 0.0,
            ramp_s: 0.0,
            final_scale: COKED_SCALE,
        }),
        ..Faults::default()
    }
}

/// Cylinder 3 misfiring on a fifth of its firings.
fn misfire() -> Faults {
    Faults {
        misfire: Some(Misfire {
            cylinder: 2,
            onset_s: 30.0,
            ramp_s: 120.0,
            final_rate: MISFIRE_RATE,
        }),
        ..Faults::default()
    }
}

/// The radiator fouled to 83% of its clean effectiveness.
fn fouling() -> Faults {
    Faults {
        cooling: Some(CoolingDegradation {
            onset_s: 30.0,
            ramp_s: 240.0,
            final_scale: FOULED_SCALE,
        }),
        ..Faults::default()
    }
}

/// The exhaust probe on cylinder 3 reading progressively high.
fn drift() -> Faults {
    Faults {
        drift: Some(SensorDrift {
            cylinder: 2,
            onset_s: 30.0,
            rate_k_per_h: DRIFT_K_PER_H,
        }),
        ..Faults::default()
    }
}

/// The exhaust probe on cylinder 3 holding its last reading.
fn freeze() -> Faults {
    Faults {
        freeze: Some(SensorFreeze {
            cylinder: 2,
            onset_s: 30.0,
        }),
        ..Faults::default()
    }
}

/// Run for `seconds`, returning the twin and the plant.
fn fly(c: &Condition, seconds: f64, faults: &Faults) -> (Twin, Plant) {
    const DT: f64 = 0.05;
    let mut plant = Plant::new(c);
    let mut twin = Twin::new(engine_model::engines::ae330());

    // Settle the plant before the twin is attached: an engine is already running
    // when a monitor is switched on, and starting both from the same instant would
    // hide a convergence problem behind a plant that has not converged either.
    for _ in 0..(60.0 / DT) as u32 {
        plant.advance(c, DT, &healthy());
    }

    let frames = (seconds / DT) as u32;
    for _ in 0..frames {
        let m = plant.advance(c, DT, faults);
        twin.update(&m).expect("the filter stays well conditioned");
    }
    (twin, plant)
}

/// The gate: a healthy engine has to sit inside two percent, and the twin has to
/// say so itself rather than being asserted into it from outside.
#[test]
fn a_healthy_engine_locks_and_stays_locked() {
    let c = Condition::cruise();
    let (twin, _) = fly(&c, 60.0, &healthy());
    let out = twin.output();

    assert!(out.rms_pct < 2.0, "residual RMS {} percent", out.rms_pct);
    assert!(out.locked, "twin did not lock at {} percent", out.rms_pct);
    for i in 0..twin_core::channels::CHANNELS {
        assert!(
            out.normalised[i].abs() < 3.0,
            "{} is {} sigma out",
            twin_core::channels::TABLE[i].name,
            out.normalised[i]
        );
    }
}

/// An engine that has stopped while its controller keeps broadcasting is the case
/// a health monitor must not lie in: the last diagnosis is still sitting in the
/// twin, and a caller told only "no error" would attach it to the new frame.
#[test]
fn an_unusable_measurement_yields_no_estimate() {
    const DT: f64 = 0.05;
    let c = Condition::cruise();
    let mut plant = Plant::new(&c);
    let mut twin = Twin::new(engine_model::engines::ae330());
    twin.update(&plant.advance(&c, DT, &healthy()))
        .expect("seeds");
    let seeded = twin.output().rms_pct;

    // Zero speed makes the load percentage on the bus unresolvable into a torque,
    // which is how a stopped engine reaches the twin.
    let mut stopped = plant.advance(&c, DT, &healthy());
    stopped.rpm = 0.0;
    stopped.torque_nm = f64::NAN;

    assert!(
        twin.update(&stopped).expect("no filter error").is_none(),
        "an unusable measurement must not hand back an estimate"
    );
    assert!(
        twin.is_seeded(),
        "the estimate is kept, it just is not current"
    );
    assert_eq!(twin.output().rms_pct, seeded, "nothing advanced");
}

/// The latch a screen joining mid-mission reads, and the reset it has to survive.
#[test]
fn ever_locked_latches_and_outlives_a_filter_reset() {
    const DT: f64 = 0.05;
    let c = Condition::cruise();
    let mut twin = Twin::new(engine_model::engines::ae330());
    assert!(
        !twin.output().ever_locked,
        "a twin that has never run has never locked"
    );

    let (locked, mut plant) = fly(&c, 60.0, &healthy());
    assert!(
        locked.output().locked && locked.output().ever_locked,
        "flew but did not lock"
    );

    // Reaching the reset needs a measurement that passes `is_usable` and then
    // breaks the filter. An unusable one returns `Ok(None)` and keeps the
    // estimate, so absurd but finite values are the way through.
    twin = locked;
    let mut broken = plant.advance(&c, DT, &healthy());
    broken.map_pa = 1e300;
    broken.cht_k = [1e300; 4];
    let _ = twin.update(&broken);
    assert!(
        twin.output().rms_pct.is_nan(),
        "the reset did not happen, so this test proves nothing"
    );
    assert!(
        twin.output().ever_locked,
        "the latch was blanked by the reset, which is the case it exists to report"
    );
}

/// A healthy engine must not be diagnosed. Every parameter staying within its own
/// posterior of nominal is the statement that the filter is not inventing faults to
/// explain its own modelling error.
#[test]
fn a_healthy_engine_leaves_every_health_parameter_at_nominal() {
    let c = Condition::cruise();
    let (twin, _) = fly(&c, 60.0, &healthy());
    let out = twin.output();

    for i in 0..twin_core::health::PARAMS {
        let d = &twin_core::health::DESCRIPTORS[i];
        let departure = (out.theta[i] - d.nominal).abs() / d.nominal;
        assert!(
            departure < 0.02,
            "{} drifted to {} from {}",
            d.name,
            out.theta[i],
            d.nominal
        );
    }
}

/// The whole claim, in one test: the parameter that moved is the one the fault was
/// injected on, and the other nine did not.
#[test]
fn a_coked_injector_moves_its_own_parameter_and_no_other() {
    let c = Condition::cruise();
    let (twin, _) = fly(&c, 400.0, &coking());
    let out = twin.output();

    let coked = out.theta[th::INJECTOR + 2];
    let nominal = twin_core::health::INJECTOR_CD_NOMINAL;
    let expected = nominal * COKED_SCALE;
    assert!(
        (coked - expected).abs() < 0.03,
        "injector-3 Cd estimated {coked}, plant is at {expected}"
    );

    for i in [0, 1, 3] {
        let other = out.theta[th::INJECTOR + i];
        assert!(
            (other - nominal).abs() < 0.02,
            "injector-{} moved to {other}",
            i + 1
        );
    }
    for i in [
        th::ETA_VOL,
        th::ETA_COMPRESSOR,
        th::ETA_TURBINE,
        th::RADIATOR,
        th::HEAD_CONDUCTANCE,
        th::OIL_SUPPLY,
    ] {
        assert!(
            (out.theta[i] - 1.0).abs() < 0.03,
            "{} moved to {}",
            twin_core::health::DESCRIPTORS[i].name,
            out.theta[i]
        );
    }
}

/// Misfire is detected and **misattributed**, and this pins both halves.
///
/// It has no health parameter, for the reason `crate::health` records: a per-cylinder
/// combustion efficiency is not identifiable against a single total fuel flow.
/// What the filter does instead is spend the nearest parameter it has, which is that
/// cylinder's injector coefficient, so the rail reports a fuel fault. Measured live,
/// a 20% misfire on cylinder 3 reads FUEL 47, AIR PATH 84 and COMBUSTION 98.
///
/// The last of those is the one to understand. `indices::combustion` scores the
/// dispersion of the **innovation**, and the innovation is flat here precisely
/// because the injector estimate absorbed the per-cylinder pattern. So the index
/// that ought to fire is the one that does not.
///
/// This is asserted rather than hidden because it is the honest limit of a health
/// rail: the fault is unmistakable in the residual pattern and invisible to the
/// index that names it. Separating misfire from coking needs the fuel-flow channel,
/// and reading residual patterns is the diagnosis layer's job at D10. If this test
/// starts failing because the attribution improved, that is the moment to rewrite it
/// rather than to relax it.
#[test]
fn a_misfire_is_detected_but_lands_on_the_wrong_subsystem() {
    let c = Condition::cruise();
    let (healthy_twin, _) = fly(&c, 400.0, &healthy());
    let (twin, _) = fly(&c, 400.0, &misfire());
    let (baseline, out) = (healthy_twin.output(), twin.output());

    // Detected: the twin knows this engine is not the healthy one.
    assert!(
        out.rms_pct > 10.0 * baseline.rms_pct,
        "misfire shows {}% against a healthy engine, healthy is {}%",
        out.rms_pct,
        baseline.rms_pct
    );
    assert!(
        out.innovation_pct > 3.0 * baseline.innovation_pct,
        "the filter should not be able to explain a misfire away: {}% against {}%",
        out.innovation_pct,
        baseline.innovation_pct
    );

    // Misattributed: it lands on the injector, which is the wrong subsystem.
    assert!(
        out.theta[th::INJECTOR + 2] < 0.90,
        "injector-3 reads {}; if it has stopped absorbing the misfire the rail has \
         been fixed and this test is stale",
        out.theta[th::INJECTOR + 2]
    );

    // Bounded: cooling and lubrication stay clean, so the misattribution cannot
    // spread across the whole rail.
    for i in [th::RADIATOR, th::HEAD_CONDUCTANCE, th::OIL_SUPPLY] {
        assert!(
            (out.theta[i] - 1.0).abs() < 0.03,
            "{} moved to {}",
            twin_core::health::DESCRIPTORS[i].name,
            out.theta[i]
        );
    }
}

/// The discrimination the whole fault library turns on, in one test.
///
/// A coked nozzle and a misfiring cylinder are the same fault on every per-cylinder
/// channel: both run that cylinder cool and lean and both cost torque. What
/// separates them is total fuel flow, because fuel a nozzle never passes never
/// leaves the tank while fuel that is injected and not burnt does. Any diagnosis
/// that cannot use this channel has to guess between the two, and this is the one
/// channel the health indices do not carry, which is why the diagnosis layer at D10
/// reads residual patterns and not the rail.
#[test]
fn coking_and_misfire_are_told_apart_by_fuel_flow() {
    let c = Condition::cruise();
    let (coked, _) = fly(&c, 400.0, &coking());
    let (misfiring, _) = fly(&c, 400.0, &misfire());
    let (coked, misfiring) = (coked.output(), misfiring.output());

    // Agreeing halves. Both cylinders are cool, lean and down on torque.
    for out in [&coked, &misfiring] {
        assert!(out.normalised[ch::EGT + 2] < -4.0);
        assert!(out.normalised[ch::LAMBDA + 2] > 4.0);
        assert!(out.normalised[ch::TORQUE] < -2.0);
    }

    // The disagreeing one.
    assert!(
        coked.normalised[ch::FUEL_FLOW] < -2.0,
        "a coked nozzle must show a fuel deficit, got {} sigma",
        coked.normalised[ch::FUEL_FLOW]
    );
    assert!(
        misfiring.normalised[ch::FUEL_FLOW].abs() < 1.0,
        "a misfiring cylinder is fuelled normally, so fuel flow must be nominal, \
         got {} sigma",
        misfiring.normalised[ch::FUEL_FLOW]
    );

    // Only one of the two is a parameter the filter carries, so only one of them is
    // explained. Coking lands on its own coefficient and leaves nothing over;
    // misfire has no parameter and stays unexplained, which is the correct outcome
    // and not a shortcoming of the estimate.
    assert!(
        coked.theta[th::INJECTOR + 2] < 0.85,
        "coking must land on injector-3, it reads {}",
        coked.theta[th::INJECTOR + 2]
    );
    assert!(
        coked.innovation_pct < 1.0,
        "coking is explained, so the innovation must stay small, it is {}%",
        coked.innovation_pct
    );
    assert!(
        misfiring.innovation_pct > coked.innovation_pct,
        "misfire has no parameter and must leave more unexplained than coking does: \
         {}% against {}%",
        misfiring.innovation_pct,
        coked.innovation_pct
    );
}

/// The money feature: a lying instrument, told apart from a failing engine.
///
/// The engine is untouched, so the signature is one channel wide. Every other
/// channel that a genuinely hot cylinder would move stays where the physics puts
/// it, and no health parameter is spent explaining it.
#[test]
fn a_drifting_probe_moves_one_channel_and_leaves_the_engine_alone() {
    let c = Condition::cruise();
    let (twin, _) = fly(&c, 600.0, &drift());
    let out = twin.output();

    assert!(
        out.normalised[ch::EGT + 2] > 1.0,
        "the drifting probe reads {} sigma high",
        out.normalised[ch::EGT + 2]
    );
    // A cylinder actually running that hot would take these with it.
    for (name, i) in [
        ("lambda 3", ch::LAMBDA + 2),
        ("CHT 3", ch::CHT + 2),
        ("fuel flow", ch::FUEL_FLOW),
    ] {
        assert!(
            out.normalised[i].abs() < 0.5,
            "{name} moved to {} sigma, which a sensor fault must not do",
            out.normalised[i]
        );
    }
    for i in 0..twin_core::health::PARAMS {
        let d = &twin_core::health::DESCRIPTORS[i];
        assert!(
            (out.theta[i] - d.nominal).abs() / d.nominal < 0.03,
            "{} moved to {} for a fault that is not in the engine",
            d.name,
            out.theta[i]
        );
    }
}

/// A frozen probe is invisible to a residual until the engine moves, and saying so
/// is the point of this test rather than a caveat on it.
///
/// At a steady operating point the held value stays right, so no residual grows and
/// nothing here can detect it. The signature of a dead channel is **zero variance**,
/// which needs a monitor watching the channel's own dispersion rather than its
/// disagreement with a model, and there is not one yet. What a residual does catch
/// is the moment the engine leaves the point the channel died at: the fuelling step
/// below takes the other three exhausts with it and leaves the frozen one behind.
///
/// A held reading is the most dangerous fault in the set for exactly this reason. It
/// looks healthy from every direction, right up until the engine moves.
#[test]
fn a_frozen_probe_is_invisible_until_the_engine_moves() {
    const DT: f64 = 0.05;
    let mut c = Condition::cruise();
    let mut plant = Plant::new(&c);
    let mut twin = Twin::new(engine_model::engines::ae330());
    for _ in 0..(60.0 / DT) as u32 {
        plant.advance(&c, DT, &healthy());
    }
    for _ in 0..(120.0 / DT) as u32 {
        let m = plant.advance(&c, DT, &freeze());
        twin.update(&m).expect("well conditioned");
    }

    let settled = twin.output().normalised[ch::EGT + 2].abs();
    assert!(
        settled < 1.0,
        "a frozen probe at a steady point reads {settled} sigma out, so this test is \
         no longer describing what it claims"
    );

    // Up to three quarters fuelling, which moves every exhaust several hundred K.
    c.fuel_cmd = 0.75;
    for _ in 0..(40.0 / DT) as u32 {
        let m = plant.advance(&c, DT, &freeze());
        twin.update(&m).expect("well conditioned");
    }

    let out = twin.output();
    assert!(
        out.normalised[ch::EGT + 2] < -3.0,
        "the frozen probe should be left far behind the step, it is {} sigma",
        out.normalised[ch::EGT + 2]
    );
    for i in [0, 1, 3] {
        assert!(
            out.normalised[ch::EGT + i].abs() < 2.0,
            "EGT {} is {} sigma out and only the frozen channel should be",
            i + 1,
            out.normalised[ch::EGT + i]
        );
    }
}

/// Cooling degradation is an operating-point-dependent fault, and pretending
/// otherwise would put a number on a screen that is wrong half the mission.
///
/// At cruise the thermostat is about a fifth open, so it answers a fouled radiator
/// by opening further and the coolant temperature barely moves: the loop's reserve
/// authority hides the fault. Hot, low and near full power the thermostat is against
/// its stop, the reserve is gone, and the same fault is plainly identifiable.
///
/// The coolant **channel** is nearly flat in both cases. It is the estimated
/// parameter that moves, which is the argument for estimating it.
#[test]
fn radiator_fouling_hides_at_cruise_and_shows_hot_and_low() {
    let (cruise, _) = fly(&Condition::cruise(), 600.0, &fouling());
    let (hot, _) = fly(&Condition::hot_and_low(), 600.0, &fouling());
    let (hot_healthy, _) = fly(&Condition::hot_and_low(), 600.0, &healthy());
    let (cruise, hot, hot_healthy) = (cruise.output(), hot.output(), hot_healthy.output());

    let loss = |o: &twin_core::TwinOutput| 1.0 - o.theta[th::RADIATOR];
    assert!(
        loss(hot) > 2.0 * loss(cruise),
        "fouling recovered {:.1}% hot and low against {:.1}% at cruise; the point of \
         the test is that these differ",
        100.0 * loss(hot),
        100.0 * loss(cruise)
    );
    assert!(
        loss(hot) > 0.06,
        "only {:.1}% of a 17% effectiveness loss recovered where it should be visible",
        100.0 * loss(hot)
    );
    assert!(
        loss(hot_healthy).abs() < 0.02,
        "the same condition reads {:.1}% on a clean radiator, so the estimate is \
         tracking the condition and not the fault",
        100.0 * loss(hot_healthy)
    );

    // Not one exhaust or head channel is a full sigma out in either case, which is
    // why a threshold monitor sees nothing here.
    for out in [&cruise, &hot] {
        assert!(out.normalised[ch::COOLANT].abs() < 1.0);
        for i in 0..engine_model::CYLINDERS {
            assert!(out.normalised[ch::CHT + i].abs() < 1.0);
        }
    }
}

/// Before the fault is injected the traces have to be on top of each other. This is
/// the half of the story that proves the twin was ever locked, and without it a
/// separation afterwards proves nothing at all.
#[test]
fn the_exhaust_residual_is_flat_before_onset_and_large_after() {
    let c = Condition::cruise();
    let mut plant = Plant::new(&c);
    let mut twin = Twin::new(engine_model::engines::ae330());
    const DT: f64 = 0.05;
    for _ in 0..(60.0 / DT) as u32 {
        plant.advance(&c, DT, &healthy());
    }

    let mut before = 0.0;
    for _ in 0..(40.0 / DT) as u32 {
        let m = plant.advance(&c, DT, &healthy());
        twin.update(&m).expect("well conditioned");
        before = twin.output().normalised[ch::EGT + 2].abs();
    }
    assert!(before < 2.0, "residual before onset is {before} sigma");

    for _ in 0..(30.0 / DT) as u32 {
        let m = plant.advance(&c, DT, &step_coking());
        twin.update(&m).expect("well conditioned");
    }
    let after = twin.output().normalised[ch::EGT + 2];
    assert!(
        after < -3.0,
        "residual after onset is {after} sigma, and it must be negative: a coked \
         injector delivers less fuel, so its cylinder runs cooler"
    );
}

/// A fuelling step is model error, not degradation. If the health parameters move
/// through a transient then every throttle change is a diagnosis.
#[test]
fn a_fuelling_transient_does_not_move_the_health_parameters() {
    let mut c = Condition::cruise();
    const DT: f64 = 0.05;
    let mut plant = Plant::new(&c);
    let mut twin = Twin::new(engine_model::engines::ae330());
    for _ in 0..(60.0 / DT) as u32 {
        plant.advance(&c, DT, &healthy());
    }
    for _ in 0..(40.0 / DT) as u32 {
        let m = plant.advance(&c, DT, &healthy());
        twin.update(&m).expect("well conditioned");
    }
    let settled = twin.output().theta;

    // Up to three quarters and back down again, which is the rapid-transient
    // scenario the problem statement names.
    for (fuel, seconds) in [(0.75, 20.0), (0.38, 20.0)] {
        c.fuel_cmd = fuel;
        for _ in 0..(seconds / DT) as u32 {
            let m = plant.advance(&c, DT, &healthy());
            twin.update(&m).expect("well conditioned");
        }
    }

    let out = twin.output();
    for (i, before) in settled.iter().enumerate() {
        let d = &twin_core::health::DESCRIPTORS[i];
        let moved = (out.theta[i] - before).abs() / d.nominal;
        assert!(
            moved < 0.02,
            "{} moved {moved} through the transient",
            d.name
        );
    }
}

/// A twin that only tracks where it was tuned is not tracking. Climbing changes
/// ambient pressure by a factor of two and every temperature in the engine with it.
#[test]
fn the_twin_tracks_through_a_climb() {
    const DT: f64 = 0.05;
    let mut c = Condition::cruise();
    c.altitude_m = 3000.0;
    let mut plant = Plant::new(&c);
    let mut twin = Twin::new(engine_model::engines::ae330());
    for _ in 0..(60.0 / DT) as u32 {
        plant.advance(&c, DT, &healthy());
    }

    let mut worst: f64 = 0.0;
    for step in 0..(180.0 / DT) as u32 {
        c.altitude_m = 3000.0 + f64::from(step) * DT * 40.0;
        c.oat_k = atmosphere::isa(c.altitude_m).t;
        let m = plant.advance(&c, DT, &healthy());
        twin.update(&m).expect("well conditioned");
        if f64::from(step) * DT > 20.0 {
            worst = worst.max(twin.output().rms_pct);
        }
    }

    assert!(c.altitude_m > 9000.0, "climb ended at {} m", c.altitude_m);
    assert!(
        worst < 2.0,
        "worst residual through the climb {worst} percent"
    );
}

/// The tuning bench. Prints the cost of a step and where every parameter and every
/// channel ends up, healthy and with a coked injector.
///
/// Ignored by default because it asserts nothing: it exists so a change to a prior,
/// a process noise or a channel's declared uncertainty can be evaluated against
/// numbers rather than against whether the assertions above happen to still pass.
///
///     cargo test -p twin-core --release --test tracking -- --ignored --nocapture
#[test]
#[ignore]
fn tuning_bench() {
    let c = Condition::cruise();
    let mut plant = Plant::new(&c);
    for _ in 0..1200 {
        plant.advance(&c, 0.05, &healthy());
    }
    let recorded: Vec<Measurement> = (0..2000)
        .map(|_| plant.advance(&c, 0.05, &healthy()))
        .collect();
    let mut timed = Twin::new(engine_model::engines::ae330());
    timed.update(&recorded[0]).expect("seed");
    let t0 = std::time::Instant::now();
    for m in &recorded[1..] {
        timed.update(m).expect("step");
    }
    let per = t0.elapsed().as_secs_f64() / (recorded.len() - 1) as f64;
    println!("--- {:.3} ms per filter step", per * 1000.0);
    for (label, condition, faults) in [
        ("healthy", Condition::cruise(), healthy()),
        ("coked", Condition::cruise(), coking()),
        ("misfire", Condition::cruise(), misfire()),
        ("fouled, cruise", Condition::cruise(), fouling()),
        ("fouled, hot and low", Condition::hot_and_low(), fouling()),
        ("healthy, hot and low", Condition::hot_and_low(), healthy()),
        ("EGT-3 drift", Condition::cruise(), drift()),
        ("EGT-3 frozen", Condition::cruise(), freeze()),
    ] {
        let (twin, _) = fly(&condition, 600.0, &faults);
        let out = twin.output();
        println!(
            "--- {label}  innovation {:.3}%  physics {:.3}%",
            out.innovation_pct, out.rms_pct
        );
        for i in 0..twin_core::health::PARAMS {
            println!(
                "  {:14} {:.4} +- {:.4}",
                twin_core::health::DESCRIPTORS[i].name,
                out.theta[i],
                out.theta_sigma[i]
            );
        }
        for i in 0..twin_core::channels::CHANNELS {
            println!(
                "  {:9} pred {:12.4} resid {:10.4} = {:7.2} sigma",
                twin_core::channels::TABLE[i].name,
                out.predicted[i],
                out.residual[i],
                out.normalised[i]
            );
        }
    }
}
