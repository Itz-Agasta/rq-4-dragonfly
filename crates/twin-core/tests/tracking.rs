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

use engine_model::{EngineParams, Inputs, State, atmosphere, control, integrator};
use twin_core::channels::index as ch;
use twin_core::health::index as th;
use twin_core::{Measurement, Twin};

/// Rated brake power, W. The denominator of the load percentage on the bus.
const RATED_W: f64 = 132_000.0;

/// Injector flow scale a coked nozzle settles at, matching the simulator.
const COKED_SCALE: f64 = 0.84;

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
    params: EngineParams,
    state: State,
    boost: control::BoostController,
    egt_lag: [f64; engine_model::CYLINDERS],
    cht_lag: [f64; engine_model::CYLINDERS],
    rng: Rng,
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
            params,
            boost: control::BoostController::new(),
            egt_lag: [0.0; engine_model::CYLINDERS],
            cht_lag: [0.0; engine_model::CYLINDERS],
            rng: Rng(0x5EED_1234),
            t_s: 0.0,
        }
    }

    /// Advance to the next telemetry instant and report what the bus would carry.
    fn advance(&mut self, c: &Condition, dt: f64, injector_scale: f64) -> Measurement {
        let (p_amb, t_amb) = c.ambient();
        self.params.cylinder.injector_scale[2] = injector_scale;

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
            egt_k: std::array::from_fn(|i| self.egt_lag[i] + 1.6 * self.rng.normal()),
            cht_k: std::array::from_fn(|i| self.cht_lag[i] + 0.8 * self.rng.normal()),
            lambda: o.lambda_cylinder,
            bus_v: 27.8,
            vib_rms_g: 1.2,
            vib_kurtosis: 2.0,
        }
    }
}

/// Run for `seconds`, returning the twin and the plant.
fn fly(c: &Condition, seconds: f64, scale_at: impl Fn(f64) -> f64) -> (Twin, Plant) {
    const DT: f64 = 0.05;
    let mut plant = Plant::new(c);
    let mut twin = Twin::new(engine_model::engines::ae330());

    // Settle the plant before the twin is attached: an engine is already running
    // when a monitor is switched on, and starting both from the same instant would
    // hide a convergence problem behind a plant that has not converged either.
    for _ in 0..(60.0 / DT) as u32 {
        plant.advance(c, DT, 1.0);
    }

    let frames = (seconds / DT) as u32;
    for _ in 0..frames {
        let m = plant.advance(c, DT, scale_at(plant.t_s));
        twin.update(&m).expect("the filter stays well conditioned");
    }
    (twin, plant)
}

/// The gate: a healthy engine has to sit inside two percent, and the twin has to
/// say so itself rather than being asserted into it from outside.
#[test]
fn a_healthy_engine_locks_and_stays_locked() {
    let c = Condition::cruise();
    let (twin, _) = fly(&c, 60.0, |_| 1.0);
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
    twin.update(&plant.advance(&c, DT, 1.0)).expect("seeds");
    let seeded = twin.output().rms_pct;

    // Zero speed makes the load percentage on the bus unresolvable into a torque,
    // which is how a stopped engine reaches the twin.
    let mut stopped = plant.advance(&c, DT, 1.0);
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

/// A healthy engine must not be diagnosed. Every parameter staying within its own
/// posterior of nominal is the statement that the filter is not inventing faults to
/// explain its own modelling error.
#[test]
fn a_healthy_engine_leaves_every_health_parameter_at_nominal() {
    let c = Condition::cruise();
    let (twin, _) = fly(&c, 60.0, |_| 1.0);
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
    let onset = 30.0;
    let ramp = 120.0;
    let (twin, _) = fly(&c, 400.0, move |t| {
        if t < onset {
            1.0
        } else {
            let growth = 1.0 - (-3.0 * (t - onset) / ramp).exp();
            1.0 - (1.0 - COKED_SCALE) * growth
        }
    });
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
        plant.advance(&c, DT, 1.0);
    }

    let mut before = 0.0;
    for _ in 0..(40.0 / DT) as u32 {
        let m = plant.advance(&c, DT, 1.0);
        twin.update(&m).expect("well conditioned");
        before = twin.output().normalised[ch::EGT + 2].abs();
    }
    assert!(before < 2.0, "residual before onset is {before} sigma");

    for _ in 0..(30.0 / DT) as u32 {
        let m = plant.advance(&c, DT, COKED_SCALE);
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
        plant.advance(&c, DT, 1.0);
    }
    for _ in 0..(40.0 / DT) as u32 {
        let m = plant.advance(&c, DT, 1.0);
        twin.update(&m).expect("well conditioned");
    }
    let settled = twin.output().theta;

    // Up to three quarters and back down again, which is the rapid-transient
    // scenario the problem statement names.
    for (fuel, seconds) in [(0.75, 20.0), (0.38, 20.0)] {
        c.fuel_cmd = fuel;
        for _ in 0..(seconds / DT) as u32 {
            let m = plant.advance(&c, DT, 1.0);
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
        plant.advance(&c, DT, 1.0);
    }

    let mut worst: f64 = 0.0;
    for step in 0..(180.0 / DT) as u32 {
        c.altitude_m = 3000.0 + f64::from(step) * DT * 40.0;
        c.oat_k = atmosphere::isa(c.altitude_m).t;
        let m = plant.advance(&c, DT, 1.0);
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
        plant.advance(&c, 0.05, 1.0);
    }
    let recorded: Vec<Measurement> = (0..2000).map(|_| plant.advance(&c, 0.05, 1.0)).collect();
    let mut timed = Twin::new(engine_model::engines::ae330());
    timed.update(&recorded[0]).expect("seed");
    let t0 = std::time::Instant::now();
    for m in &recorded[1..] {
        timed.update(m).expect("step");
    }
    let per = t0.elapsed().as_secs_f64() / (recorded.len() - 1) as f64;
    println!("--- {:.3} ms per filter step", per * 1000.0);
    for (label, scale) in [("healthy", 1.0f64), ("coked", COKED_SCALE)] {
        let (twin, _) = fly(&c, 400.0, move |t| {
            if t < 30.0 {
                1.0
            } else {
                1.0 - (1.0 - scale) * (1.0 - (-3.0 * (t - 30.0) / 120.0).exp())
            }
        });
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
