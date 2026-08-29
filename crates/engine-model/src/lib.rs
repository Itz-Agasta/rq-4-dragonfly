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
pub mod flow;
pub mod friction;
pub mod integrator;
pub mod manifold;
pub mod params;
pub mod turbine;

pub use params::{EngineParams, ParamError};

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
}

impl Add for State {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self {
            p_im: self.p_im + o.p_im,
            p_em: self.p_em + o.p_em,
            omega_e: self.omega_e + o.omega_e,
            omega_tc: self.omega_tc + o.omega_tc,
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
        }
    }
}

impl State {
    /// A plausible starting point: manifolds at ambient, engine at the given speed,
    /// turbocharger idling.
    #[must_use]
    pub fn at_rest(p_amb: f64, rpm: f64) -> Self {
        Self {
            p_im: p_amb,
            p_em: p_amb,
            omega_e: rpm * std::f64::consts::TAU / 60.0,
            omega_tc: 2000.0,
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
    /// Torque absorbed by whatever the crankshaft drives, N.m, referred to the
    /// crankshaft. A constant-speed propeller unit makes this the governor output.
    pub load_torque: f64,
}

/// Everything the model computes at one operating point.
///
/// Deliberately wide. Diagnosis works on the disagreement between a measurement and
/// the corresponding modelled quantity, so any quantity a sensor might report has to
/// be available here to be compared against.
#[derive(Clone, Copy, Debug)]
pub struct Outputs {
    /// Intake charge temperature after the intercooler, K.
    pub t_intake: f64,
    /// Compressor pressure ratio.
    pub compressor_ratio: f64,
    /// Compressor mass flow, kg/s.
    pub w_compressor: f64,
    /// Compressor isentropic efficiency.
    pub eta_compressor: f64,
    /// Shaft power absorbed by the compressor, W.
    pub power_compressor: f64,
    /// Margin to the compressor surge line, in pressure-ratio units.
    pub surge_margin: f64,
    /// Volumetric efficiency.
    pub eta_vol: f64,
    /// Air mass flow into the cylinders, kg/s.
    pub w_air: f64,
    /// Injected fuel per cylinder per cycle after the smoke limit, mg.
    pub u_f_mg: f64,
    /// Fuel mass flow, kg/s.
    pub w_fuel: f64,
    /// Excess air ratio. Infinite while motoring.
    pub lambda: f64,
    /// Gross indicated efficiency.
    pub eta_ig: f64,
    /// Gross indicated mean effective pressure, Pa.
    pub imep_gross: f64,
    /// Gross indicated torque, N.m.
    pub torque_indicated: f64,
    /// Friction and accessory torque, N.m.
    pub torque_friction: f64,
    /// Pumping torque, N.m. Negative when boost exceeds back pressure.
    pub torque_pumping: f64,
    /// Brake torque at the crankshaft, N.m.
    pub torque_brake: f64,
    /// Brake power, W.
    pub power_brake_w: f64,
    /// Torque at the propeller flange, N.m.
    pub torque_prop: f64,
    /// Propeller speed, rpm.
    pub rpm_prop: f64,
    /// Temperature of the gas leaving the cylinders, K.
    pub t_cylinder_out: f64,
    /// Temperature of the gas reaching the turbine, K. This is what a manifold
    /// thermocouple reads, and it is cooler than [`Outputs::t_cylinder_out`].
    pub t_exhaust: f64,
    /// Mass flow through the turbine, kg/s.
    pub w_turbine: f64,
    /// Mass flow bypassing the turbine, kg/s.
    pub w_wastegate: f64,
    /// Turbine blade speed ratio.
    pub blade_speed_ratio: f64,
    /// Turbine combined isentropic and mechanical efficiency.
    pub eta_turbine: f64,
    /// Shaft power delivered by the turbine, W.
    pub power_turbine: f64,
}

impl Outputs {
    /// Brake specific fuel consumption, g/kWh.
    ///
    /// `None` while the engine is motoring, because specific consumption is
    /// undefined at or below zero output. Returning a sentinel instead would put a
    /// number on a display that means the opposite of what it appears to.
    #[must_use]
    pub fn bsfc_g_per_kwh(&self) -> Option<f64> {
        (self.power_brake_w > 0.0).then(|| self.w_fuel * 3.6e9 / self.power_brake_w)
    }

    /// Volumetric fuel flow, litres/hour.
    #[must_use]
    pub fn fuel_litres_per_hour(&self, p: &EngineParams) -> f64 {
        self.w_fuel / p.fuel.density_kg_m3 * 3.6e6
    }

    /// Boost pressure above ambient, Pa.
    #[must_use]
    pub fn boost_pa(&self, p_amb: f64, p_im: f64) -> f64 {
        p_im - p_amb
    }
}

/// Evaluate every algebraic quantity at one state and set of inputs.
#[must_use]
pub fn evaluate(p: &EngineParams, x: &State, u: &Inputs) -> Outputs {
    let comp = compressor::operate(p, x.omega_tc, u.p_amb, u.t_amb, x.p_im);
    let t_intake = compressor::intercooler_outlet(p, comp.outlet_temperature, u.t_amb);

    let eta_vol = cylinder::volumetric_efficiency(p, x.p_im, x.omega_e);
    let w_air = cylinder::air_flow(p, x.p_im, x.omega_e, t_intake);
    let u_f_mg = cylinder::injected_fuel(p, u.fuel_cmd, w_air, x.omega_e);
    let w_fuel = cylinder::fuel_flow(p, u_f_mg, x.omega_e);
    let lambda = cylinder::lambda(p, w_air, w_fuel);

    let eta_ig = cylinder::indicated_efficiency(p, lambda);
    let imep_gross = cylinder::imep_gross(p, eta_ig, u_f_mg);
    let torque_indicated = friction::torque_from_mep(p, imep_gross);
    let torque_friction = friction::friction_torque(p, x.omega_e);
    let torque_pumping = friction::pumping_torque(p, x.p_im, x.p_em);
    let torque_brake = torque_indicated - torque_friction - torque_pumping;

    let t_cylinder_out = cylinder::exhaust_temperature(p, w_air, w_fuel, x.p_im, x.p_em, t_intake);
    let t_exhaust = manifold::exhaust_gas_temperature(p, t_cylinder_out, u.t_amb, w_air + w_fuel);

    let turb = turbine::operate(p, x.omega_tc, x.p_em, u.p_amb, t_exhaust);
    let w_wastegate = turbine::wastegate_flow(p, u.wastegate, x.p_em, u.p_amb, t_exhaust);

    Outputs {
        t_intake,
        compressor_ratio: comp.pressure_ratio,
        w_compressor: comp.mass_flow,
        eta_compressor: comp.efficiency,
        power_compressor: comp.power,
        surge_margin: comp.surge_margin,
        eta_vol,
        w_air,
        u_f_mg,
        w_fuel,
        lambda,
        eta_ig,
        imep_gross,
        torque_indicated,
        torque_friction,
        torque_pumping,
        torque_brake,
        power_brake_w: torque_brake * x.omega_e,
        torque_prop: torque_brake * p.geometry.gearbox_ratio,
        rpm_prop: x.rpm() / p.geometry.gearbox_ratio,
        t_cylinder_out,
        t_exhaust,
        w_turbine: turb.mass_flow,
        w_wastegate,
        blade_speed_ratio: turb.blade_speed_ratio,
        eta_turbine: turb.efficiency,
        power_turbine: turb.power,
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
    }
}

/// Advance the state by one step.
///
/// Inputs are held constant across the step, which is the standard zero-order hold
/// and is exact for a model driven by a digital controller at or above the step
/// rate. Pressures and speeds are floored afterwards: a manifold pressure cannot go
/// negative, and a transient that drove one there would otherwise put a negative
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
            load_torque: 0.0,
        }
    }

    /// The published take-off rating: 132 kW and 550 N.m at the propeller, at a
    /// crank speed of 3880 rpm, burning 39 litres an hour.
    #[test]
    fn reproduces_the_published_rating_point() {
        let p = engines::ae330();
        let x = State {
            p_im: p.control.map_setpoint_pa,
            p_em: 3.45e5,
            omega_e: omega(3880.0),
            omega_tc: 14_900.0,
        };
        let o = evaluate(&p, &x, &sea_level(1.0, 0.0));

        assert!(
            (o.power_brake_w - 132_000.0).abs() / 132_000.0 < 0.10,
            "{} W",
            o.power_brake_w
        );
        assert!(
            (o.torque_prop - 550.0).abs() / 550.0 < 0.10,
            "{} N.m",
            o.torque_prop
        );
        let lph = o.fuel_litres_per_hour(&p);
        assert!((lph - 39.0).abs() / 39.0 < 0.10, "{lph} L/h");
        assert!((o.rpm_prop - 2296.0).abs() < 5.0, "prop {} rpm", o.rpm_prop);

        let bsfc = o.bsfc_g_per_kwh().unwrap();
        assert!((200.0..280.0).contains(&bsfc), "bsfc {bsfc} g/kWh");
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
            u.wastegate = boost.update(&p, x.p_im, x.omega_tc, integrator::DT);
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
            u.wastegate = boost.update(&p, x.p_im, x.omega_tc, integrator::DT);
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
