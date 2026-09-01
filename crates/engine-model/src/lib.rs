//! Mean value engine model of a turbocharged heavy-fuel aero piston engine.
//!
//! Pure by construction: no I/O, no async, no clock, no global state. The caller
//! owns the integration loop and hands in state plus inputs. That purity is what
//! lets a thirty-hour mission be projected in seconds, and it is what makes every
//! equation testable against a hand-computed case in isolation.
//!
//! The engine modelled is a compression-ignition common-rail direct-injection
//! diesel, not a spark-ignition engine. The distinction is structural rather than
//! cosmetic: there is no throttle plate, load is set by injected fuel quantity, the
//! excess air ratio is an output rather than a controlled variable, and it runs
//! between about 1.3 and 5 rather than near unity. Most of the published mean value
//! engine model literature is written for spark ignition; the model here follows
//! the diesel line of that work instead.
//!
//! # Structure
//!
//! State is intake and exhaust manifold pressure and crankshaft speed. Manifolds
//! are isothermal ideal-gas control volumes; the cylinders are a flow and a work
//! source between them; losses are mean effective pressures.
//!
//! Two quantities are inputs here that become states as the model grows: the flow
//! delivered into the intake manifold, which a compressor model supplies, and the
//! intake manifold temperature, which an intercooler model supplies. Both are
//! marked at their definitions.
//!
//! # References
//!
//! Wahlstrom & Eriksson, "Modelling diesel engines with a variable-geometry
//! turbocharger and exhaust gas recirculation by optimization of model parameters
//! for capturing non-linear system dynamics", Proc IMechE Part D 225(7), 2011.
//! Model structure, volumetric efficiency and manifold treatment.
//! <https://doi.org/10.1177/0954407011398177>
//!
//! Ekberg, Leek & Eriksson, "Validation of an Open-Source Mean-Value Heavy-Duty
//! Diesel Engine Model", SIMS 59, 2018. Torque and exhaust temperature.
//! <https://doi.org/10.3384/ecp18153290>
//!
//! Neither of those authors' published implementations is used or derived from
//! here; the model is written from the equations in the papers.
#![forbid(unsafe_code)]

pub mod atmosphere;
pub mod compressor;
pub mod control;
pub mod cylinder;
pub mod engines;
pub mod friction;
pub mod integrator;
pub mod manifold;
pub mod oil;
pub mod output;
pub mod params;
pub mod published;
pub mod thermal;
pub mod turbine;

pub use output::Outputs;
pub use params::{EngineParams, ParamError};

/// Number of cylinders.
///
/// A compile-time constant rather than a parameter because the per-cylinder
/// channels are fixed-width arrays. Modelling a different cylinder count means
/// changing this and rebuilding, which is honest: the array width is in the type
/// system, so nothing can silently disagree about it. Parameter loading rejects a
/// file whose cylinder count is anything else.
pub const CYLINDERS: usize = 4;

use std::ops::{Add, Mul};

/// Engine state vector.
///
/// Also used to carry the time derivative of the state, which is why it implements
/// addition and scaling. Reusing the type keeps the integrator free of a parallel
/// derivative type that would have to be kept in step with this one by hand.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    /// Intake manifold pressure, Pa.
    pub p_im: f64,
    /// Exhaust manifold pressure, Pa.
    pub p_em: f64,
    /// Crankshaft speed, rad/s.
    pub omega_e: f64,
    /// Turbocharger shaft speed, rad/s.
    pub omega_tc: f64,
    /// Cylinder head metal temperature, K, one per cylinder.
    pub t_cht: [f64; CYLINDERS],
    /// Coolant temperature, K.
    pub t_coolant: f64,
    /// Oil temperature, K.
    pub t_oil: f64,
}

impl Add for State {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self {
            p_im: self.p_im + o.p_im,
            p_em: self.p_em + o.p_em,
            omega_e: self.omega_e + o.omega_e,
            omega_tc: self.omega_tc + o.omega_tc,
            t_cht: std::array::from_fn(|i| self.t_cht[i] + o.t_cht[i]),
            t_coolant: self.t_coolant + o.t_coolant,
            t_oil: self.t_oil + o.t_oil,
        }
    }
}

impl Mul<f64> for State {
    type Output = Self;
    fn mul(self, k: f64) -> Self {
        Self {
            p_im: self.p_im * k,
            p_em: self.p_em * k,
            omega_e: self.omega_e * k,
            omega_tc: self.omega_tc * k,
            t_cht: std::array::from_fn(|i| self.t_cht[i] * k),
            t_coolant: self.t_coolant * k,
            t_oil: self.t_oil * k,
        }
    }
}

impl State {
    /// A plausible starting point: manifolds at ambient, engine at the given speed,
    /// turbocharger idling, everything thermal already warm.
    ///
    /// Warm rather than cold deliberately. A cold start takes minutes of simulated
    /// time to settle and almost every use of this is a study of an engine already
    /// running; [`State::cold`] exists for the cases that are not.
    #[must_use]
    pub fn at_rest(p_amb: f64, rpm: f64) -> Self {
        Self {
            p_im: p_amb,
            p_em: p_amb,
            omega_e: rpm * std::f64::consts::TAU / 60.0,
            omega_tc: 2000.0,
            t_cht: [400.0; CYLINDERS],
            t_coolant: 360.0,
            t_oil: 355.0,
        }
    }

    /// Everything thermal at ambient, for a cold start.
    #[must_use]
    pub fn cold(p_amb: f64, t_amb: f64) -> Self {
        Self {
            p_im: p_amb,
            p_em: p_amb,
            omega_e: 0.0,
            omega_tc: 300.0,
            t_cht: [t_amb; CYLINDERS],
            t_coolant: t_amb,
            t_oil: t_amb,
        }
    }

    /// Crankshaft speed in rpm.
    #[must_use]
    pub fn rpm(&self) -> f64 {
        self.omega_e * 60.0 / std::f64::consts::TAU
    }

    /// Turbocharger speed in rpm.
    #[must_use]
    pub fn turbo_rpm(&self) -> f64 {
        self.omega_tc * 60.0 / std::f64::consts::TAU
    }
}

/// Everything acting on the engine from outside it.
#[derive(Clone, Copy, Debug)]
pub struct Inputs {
    /// FADEC fuelling command, 0 to 1. The load actuator of a diesel.
    pub fuel_cmd: f64,
    /// Wastegate position, 0 shut to 1 fully open. An input rather than a state
    /// because the controller that drives it is not part of the engine; see
    /// [`control::BoostController`].
    pub wastegate: f64,
    /// Ambient static pressure, Pa. From [`atmosphere::isa`] and the altitude.
    pub p_amb: f64,
    /// Ambient static temperature, K. Compressor inlet, intercooler sink, and the
    /// reference the exhaust manifold loses heat to.
    pub t_amb: f64,
    /// True airspeed, m/s. Ram air through the radiator and the oil cooler. True
    /// rather than indicated because it is mass flow that cools, and the caller
    /// already knows the density.
    pub tas_m_s: f64,
    /// Torque absorbed by whatever the crankshaft drives, N.m, referred to the
    /// crankshaft. A constant-speed propeller unit makes this the governor output.
    pub load_torque: f64,
}

/// Evaluate every algebraic quantity at one state and set of inputs.
#[must_use]
pub fn evaluate(p: &EngineParams, x: &State, u: &Inputs) -> Outputs {
    let comp = compressor::operate(p, x.omega_tc, u.p_amb, u.t_amb, x.p_im);
    let t_intake = compressor::intercooler_outlet(p, comp.outlet_temperature, u.t_amb);

    let eta_vol = cylinder::volumetric_efficiency(p, x.p_im, x.omega_e);
    let w_air = cylinder::air_flow(p, x.p_im, x.omega_e, t_intake);
    let u_f_mg = cylinder::injected_fuel(p, u.fuel_cmd, w_air, x.omega_e);

    // Per cylinder from here. Air is assumed to distribute evenly; fuel is not,
    // because the injector scale is the parameter every injection fault acts on.
    let u_f_cylinder = cylinder::per_cylinder_fuel(p, u_f_mg);
    let w_air_cylinder = w_air / CYLINDERS as f64;
    let w_fuel_cylinder: [f64; CYLINDERS] =
        std::array::from_fn(|i| cylinder::fuel_flow(p, u_f_cylinder[i], x.omega_e));
    let w_fuel: f64 = w_fuel_cylinder.iter().sum();

    // Delivered above, burned here. Identical on a healthy engine; a misfiring
    // cylinder is the only thing that separates them, and everything thermodynamic
    // below takes the burned quantity while the published fuel flow stays delivered.
    let w_fuel_burned_cylinder = cylinder::burned_fuel(p, &w_fuel_cylinder);
    let u_f_burned = cylinder::burned_fuel(p, &u_f_cylinder);
    let w_fuel_burned: f64 = w_fuel_burned_cylinder.iter().sum();

    // Excess air is what an oxygen probe reads, so it is set by the fuel that
    // consumed oxygen. Unburnt fuel leaves its share of the air untouched and the
    // cylinder reads lean, which is why misfire and a restricted nozzle look alike
    // on this channel and are told apart on fuel flow.
    let lambda_cylinder: [f64; CYLINDERS] =
        std::array::from_fn(|i| cylinder::lambda(p, w_air_cylinder, w_fuel_burned_cylinder[i]));
    let lambda = cylinder::lambda(p, w_air, w_fuel_burned);

    // Efficiency is evaluated at the **delivered** mixture, not the burned one, and
    // the difference is the whole of how misfire produces work. Misfire is a
    // cycle-to-cycle event: the cycles that ignite run on the charge they were
    // given, at their own unchanged excess air ratio, and the ones that do not
    // produce nothing. So the mean work is the nominal efficiency times the fuel
    // that burned, and the fault enters through the mass alone.
    //
    // Evaluating efficiency at the burned-fuel ratio instead would put the firing
    // cycles at a leaner mixture than any of them ever saw, and since the
    // efficiency island is curved that charges misfire a second torque penalty it
    // has no mechanism for. A restricted nozzle is unaffected either way, because
    // there the charge really is leaner on every cycle.
    let eta_ig: [f64; CYLINDERS] = std::array::from_fn(|i| {
        cylinder::indicated_efficiency(p, cylinder::lambda(p, w_air_cylinder, w_fuel_cylinder[i]))
    });

    let imep_gross = cylinder::imep_gross(p, &eta_ig, &u_f_burned);
    let torque_indicated = friction::torque_from_mep(p, imep_gross);
    let torque_friction = friction::friction_torque(p, x.omega_e);
    let torque_pumping = friction::pumping_torque(p, x.p_im, x.p_em);
    let torque_brake = torque_indicated - torque_friction - torque_pumping;

    let t_cylinder_out: [f64; CYLINDERS] = std::array::from_fn(|i| {
        cylinder::exhaust_temperature(
            p,
            w_air_cylinder,
            w_fuel_cylinder[i],
            w_fuel_burned_cylinder[i],
            x.p_im,
            x.p_em,
            t_intake,
        )
    });
    let t_egt: [f64; CYLINDERS] = std::array::from_fn(|i| {
        manifold::exhaust_gas_temperature(
            p,
            t_cylinder_out[i],
            u.t_amb,
            w_air_cylinder + w_fuel_cylinder[i],
        )
    });
    // The turbine sees the mixed stream, not any one runner.
    let t_exhaust = t_egt.iter().sum::<f64>() / CYLINDERS as f64;

    let turb = turbine::operate(p, x.omega_tc, x.p_em, u.p_amb, t_exhaust);
    let w_wastegate = turbine::wastegate_flow(p, u.wastegate, x.p_em, u.p_amb, t_exhaust);

    let head_conductance = thermal::head_conductance(p, x.omega_e);
    let heat_to_coolant: f64 = (0..CYLINDERS)
        .map(|i| head_conductance * (x.t_cht[i] - x.t_coolant))
        .sum();
    let heat_to_oil = oil::heat_into_oil(p, torque_friction * x.omega_e, w_fuel_burned);

    Outputs {
        t_intake,
        w_compressor: comp.mass_flow,
        eta_compressor: comp.efficiency,
        power_compressor: comp.power,
        surge_margin: comp.surge_margin,
        eta_vol,
        w_air,
        u_f_mg,
        u_f_cylinder,
        w_fuel,
        w_fuel_cylinder,
        w_fuel_burned_cylinder,
        lambda,
        lambda_cylinder,
        eta_ig,
        torque_indicated,
        torque_friction,
        torque_pumping,
        torque_brake,
        power_brake_w: torque_brake * x.omega_e,
        torque_prop: torque_brake * p.geometry.gearbox_ratio,
        rpm_prop: x.rpm() / p.geometry.gearbox_ratio,
        t_egt,
        t_exhaust,
        w_turbine: turb.mass_flow,
        w_wastegate,
        blade_speed_ratio: turb.blade_speed_ratio,
        eta_turbine: turb.efficiency,
        power_turbine: turb.power,
        heat_to_coolant,
        heat_to_oil,
        p_oil: oil::gallery_pressure(p, x.omega_e, x.t_oil),
    }
}

/// Time derivative of the state.
#[must_use]
pub fn derivative(p: &EngineParams, x: &State, u: &Inputs) -> State {
    let o = evaluate(p, x, u);
    // Floored, not clamped after the fact: the shaft power balance divides by speed,
    // and a turbocharger that has coasted to a stop must still be able to be spun up
    // by exhaust energy rather than dividing by zero.
    let omega_tc = x.omega_tc.max(p.turbocharger.omega_min);
    let rho = u.p_amb / (p.gas.r_air * u.t_amb.max(1.0));

    State {
        p_im: manifold::intake_pressure_rate(p, o.t_intake, o.w_compressor, o.w_air),
        p_em: manifold::exhaust_pressure_rate(
            p,
            o.t_exhaust,
            o.w_air + o.w_fuel,
            o.w_turbine + o.w_wastegate,
        ),
        omega_e: (o.torque_brake - u.load_torque) / p.geometry.inertia_kg_m2,
        omega_tc: (o.power_turbine - o.power_compressor)
            / (omega_tc * p.turbocharger.inertia_kg_m2),
        t_cht: std::array::from_fn(|i| {
            thermal::head_temperature_rate(
                p,
                x.t_cht[i],
                x.t_coolant,
                o.w_fuel_burned_cylinder[i],
                x.omega_e,
            )
        }),
        t_coolant: thermal::coolant_temperature_rate(
            p,
            o.heat_to_coolant,
            x.t_coolant,
            u.t_amb,
            rho,
            u.tas_m_s,
        ),
        t_oil: oil::temperature_rate(p, x.t_oil, u.t_amb, o.heat_to_oil, rho, u.tas_m_s),
    }
}

/// Advance the state by one step.
///
/// Inputs are held constant across the step, which is the standard zero-order hold
/// and is exact for a model driven by a digital controller at or above the step
/// rate. Pressures, speeds and temperatures are floored afterwards: none of them can
/// go negative, and a transient that drove one there would otherwise put a negative
/// number under a square root on the next step.
#[must_use]
pub fn step(p: &EngineParams, x: &State, u: &Inputs, dt: f64) -> State {
    let next = integrator::rk4(*x, dt, |s| derivative(p, &s, u));
    State {
        p_im: next.p_im.max(1.0),
        p_em: next.p_em.max(1.0),
        omega_e: next.omega_e.max(0.0),
        omega_tc: next
            .omega_tc
            .clamp(p.turbocharger.omega_min, p.turbocharger.omega_max),
        t_cht: std::array::from_fn(|i| next.t_cht[i].max(1.0)),
        t_coolant: next.t_coolant.max(1.0),
        t_oil: next.t_oil.max(1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn omega(rpm: f64) -> f64 {
        rpm * std::f64::consts::TAU / 60.0
    }

    fn sea_level(fuel_cmd: f64, wastegate: f64) -> Inputs {
        let a = atmosphere::isa(0.0);
        Inputs {
            fuel_cmd,
            wastegate,
            p_amb: a.p,
            t_amb: a.t,
            tas_m_s: 60.0,
            load_torque: 0.0,
        }
    }

    /// The published take-off rating, read from [`published`] rather than typed
    /// here, so that a change to one of those constants cannot pass unnoticed.
    /// The tolerance is 5% and not 10%: the model was fitted to this point, so a
    /// looser bound would let a real regression through as a fitting error.
    #[test]
    fn reproduces_the_published_rating_point() {
        use published as pubd;
        let p = engines::ae330();
        let x = State {
            p_im: p.control.map_setpoint_pa,
            p_em: 3.45e5,
            omega_e: omega(pubd::TAKEOFF_RPM_CRANK),
            omega_tc: 14_900.0,
            ..State::at_rest(atmosphere::isa(0.0).p, pubd::TAKEOFF_RPM_CRANK)
        };
        let o = evaluate(&p, &x, &sea_level(1.0, 0.0));

        assert!(
            (o.power_brake_w - pubd::TAKEOFF_POWER_W).abs() / pubd::TAKEOFF_POWER_W < 0.05,
            "{} W against a published {} W",
            o.power_brake_w,
            pubd::TAKEOFF_POWER_W
        );
        assert!(
            (o.torque_prop - pubd::MAX_TORQUE_PROP_NM).abs() / pubd::MAX_TORQUE_PROP_NM < 0.05,
            "{} N.m against a published {} N.m",
            o.torque_prop,
            pubd::MAX_TORQUE_PROP_NM
        );
        let lph = o.fuel_litres_per_hour(&p);
        assert!(
            (lph - pubd::FUEL_LPH_AT_FULL).abs() / pubd::FUEL_LPH_AT_FULL < 0.05,
            "{lph} L/h against a published {} L/h",
            pubd::FUEL_LPH_AT_FULL
        );
        assert!(
            (o.rpm_prop - pubd::TAKEOFF_RPM_PROP).abs() < 10.0,
            "prop {} rpm",
            o.rpm_prop
        );

        let bsfc = o.bsfc_g_per_kwh().unwrap();
        assert!((200.0..280.0).contains(&bsfc), "bsfc {bsfc} g/kWh");
    }

    /// Misfire is a cycle-to-cycle event, so its cost is linear in the rate.
    ///
    /// A cylinder failing a fifth of its firings loses exactly a fifth of its
    /// indicated work and no more: the cycles that ignite are untouched. The bug
    /// this pins is subtle and was live for one session: evaluating indicated
    /// efficiency at the burned-fuel excess air ratio puts the firing cycles at a
    /// mixture none of them ever saw, and the curvature of the efficiency island
    /// then charges misfire a second torque penalty with no mechanism behind it.
    #[test]
    fn misfire_costs_work_in_proportion_to_the_firings_it_loses() {
        let p = engines::ae330();
        let x = State {
            p_im: p.control.map_setpoint_pa,
            p_em: 3.45e5,
            omega_e: omega(3880.0),
            omega_tc: 14_900.0,
            ..State::at_rest(atmosphere::isa(0.0).p, 3880.0)
        };
        let u = sea_level(1.0, 0.0);
        let healthy = evaluate(&p, &x, &u);

        for rate in [0.10, 0.20, 0.50] {
            let mut faulted = p.clone();
            faulted.cylinder.combustion_efficiency[2] = 1.0 - rate;
            let o = evaluate(&faulted, &x, &u);

            // One cylinder of four losing `rate` of its work.
            let expected = healthy.torque_indicated * (1.0 - rate / CYLINDERS as f64);
            assert!(
                (o.torque_indicated - expected).abs() / expected < 1e-12,
                "at rate {rate} indicated torque is {} against {expected}",
                o.torque_indicated
            );
            // The firing cycles keep their own efficiency, and the neighbours keep
            // theirs. Only the mass that burned changed.
            for i in 0..CYLINDERS {
                assert!(
                    (o.eta_ig[i] - healthy.eta_ig[i]).abs() < 1e-12,
                    "cylinder {i} efficiency moved at rate {rate}"
                );
            }
            // Fuel still leaves the tank, which is the discriminating channel.
            assert!((o.w_fuel - healthy.w_fuel).abs() / healthy.w_fuel < 1e-12);
        }
    }

    #[test]
    fn motoring_produces_negative_torque_and_no_bsfc() {
        let p = engines::ae330();
        let x = State::at_rest(101_325.0, 2000.0);
        let o = evaluate(&p, &x, &sea_level(0.0, 0.0));
        assert!(o.torque_brake < 0.0);
        assert!(o.bsfc_g_per_kwh().is_none());
        assert!(o.lambda.is_infinite());
    }

    /// Run the whole loop closed, from a cold manifold to steady state at the rated
    /// speed, and check the boost controller finds the set-point without the shaft
    /// running away.
    #[test]
    fn the_closed_loop_settles_on_the_boost_set_point() {
        let p = engines::ae330();
        let mut x = State::at_rest(atmosphere::isa(0.0).p, 3880.0);
        let mut boost = control::BoostController::new();
        let mut u = sea_level(1.0, 1.0);

        for _ in 0..(20.0 / integrator::DT) as u32 {
            u.wastegate = boost.update(&p, u.fuel_cmd, x.p_im, x.omega_tc, integrator::DT);
            // Perfect governor: the propeller absorbs exactly what the engine makes,
            // which is what holds crank speed while the turbocharger finds its own.
            u.load_torque = evaluate(&p, &x, &u).torque_brake;
            x = step(&p, &x, &u, integrator::DT);
            assert!(x.p_im.is_finite() && x.omega_tc.is_finite());
        }

        let error = (x.p_im - p.control.map_setpoint_pa).abs() / p.control.map_setpoint_pa;
        assert!(error < 0.05, "settled at {} Pa", x.p_im);
        assert!(
            x.omega_tc < p.turbocharger.omega_max,
            "turbo {} rad/s",
            x.omega_tc
        );
        assert!(
            evaluate(&p, &x, &u).surge_margin > 0.0,
            "compressor is surging"
        );
    }

    #[test]
    fn state_stays_finite_over_a_full_load_step() {
        let p = engines::ae330();
        let mut x = State::at_rest(atmosphere::isa(0.0).p, 2000.0);
        let mut boost = control::BoostController::new();
        let mut u = sea_level(0.0, 1.0);
        for i in 0..8000 {
            if i == 1000 {
                u.fuel_cmd = 1.0;
            }
            u.wastegate = boost.update(&p, u.fuel_cmd, x.p_im, x.omega_tc, integrator::DT);
            u.load_torque = evaluate(&p, &x, &u).torque_brake;
            x = step(&p, &x, &u, integrator::DT);
            assert!(
                x.p_im.is_finite()
                    && x.p_em.is_finite()
                    && x.omega_e.is_finite()
                    && x.omega_tc.is_finite(),
                "diverged at step {i}: {x:?}"
            );
        }
    }
}
