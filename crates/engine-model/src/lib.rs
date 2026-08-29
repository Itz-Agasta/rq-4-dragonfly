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
pub mod cylinder;
pub mod engines;
pub mod flow;
pub mod friction;
pub mod integrator;
pub mod manifold;
pub mod params;

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
}

impl Add for State {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self {
            p_im: self.p_im + o.p_im,
            p_em: self.p_em + o.p_em,
            omega_e: self.omega_e + o.omega_e,
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
        }
    }
}

impl State {
    /// A plausible starting point: manifolds at ambient, engine at the given speed.
    #[must_use]
    pub fn at_rest(p_amb: f64, rpm: f64) -> Self {
        Self {
            p_im: p_amb,
            p_em: p_amb,
            omega_e: rpm * std::f64::consts::TAU / 60.0,
        }
    }

    /// Crankshaft speed in rpm.
    #[must_use]
    pub fn rpm(&self) -> f64 {
        self.omega_e * 60.0 / std::f64::consts::TAU
    }
}

/// Everything acting on the engine from outside it.
#[derive(Clone, Copy, Debug)]
pub struct Inputs {
    /// FADEC fuelling command, 0 to 1. The load actuator of a diesel.
    pub fuel_cmd: f64,
    /// Mass flow delivered into the intake manifold, kg/s. Becomes an output of the
    /// compressor once a turbocharger model is attached.
    pub w_intake: f64,
    /// Ambient static pressure, Pa. From [`atmosphere::isa`] and the altitude.
    pub p_amb: f64,
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
    pub t_exhaust: f64,
    /// Mass flow leaving the exhaust manifold, kg/s.
    pub w_exhaust_out: f64,
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
}

/// Evaluate every algebraic quantity at one state and set of inputs.
#[must_use]
pub fn evaluate(p: &EngineParams, x: &State, u: &Inputs) -> Outputs {
    let t_im = p.control.t_im_k;

    let eta_vol = cylinder::volumetric_efficiency(p, x.p_im, x.omega_e);
    let w_air = cylinder::air_flow(p, x.p_im, x.omega_e, t_im);
    let u_f_mg = cylinder::injected_fuel(p, u.fuel_cmd, w_air, x.omega_e);
    let w_fuel = cylinder::fuel_flow(p, u_f_mg, x.omega_e);
    let lambda = cylinder::lambda(p, w_air, w_fuel);

    let eta_ig = cylinder::indicated_efficiency(p, lambda);
    let imep_gross = cylinder::imep_gross(p, eta_ig, u_f_mg);
    let torque_indicated = friction::torque_from_mep(p, imep_gross);
    let torque_friction = friction::friction_torque(p, x.omega_e);
    let torque_pumping = friction::pumping_torque(p, x.p_im, x.p_em);
    let torque_brake = torque_indicated - torque_friction - torque_pumping;

    let t_exhaust = cylinder::exhaust_temperature(p, w_air, w_fuel, x.p_im, x.p_em, t_im);
    let w_exhaust_out = manifold::exhaust_outflow(p, x.p_em, u.p_amb, t_exhaust);

    Outputs {
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
        t_exhaust,
        w_exhaust_out,
    }
}

/// Time derivative of the state.
#[must_use]
pub fn derivative(p: &EngineParams, x: &State, u: &Inputs) -> State {
    let o = evaluate(p, x, u);
    State {
        p_im: manifold::intake_pressure_rate(p, p.control.t_im_k, u.w_intake, o.w_air),
        p_em: manifold::exhaust_pressure_rate(p, o.t_exhaust, o.w_air + o.w_fuel, o.w_exhaust_out),
        omega_e: (o.torque_brake - u.load_torque) / p.geometry.inertia_kg_m2,
    }
}

/// Advance the state by one step.
///
/// Inputs are held constant across the step, which is the standard zero-order hold
/// and is exact for a model driven by a digital controller at or above the step
/// rate. Pressures and speed are floored at zero afterwards: a manifold pressure
/// cannot go negative, and a transient that drove one there would otherwise put a
/// negative number under a square root on the next step.
#[must_use]
pub fn step(p: &EngineParams, x: &State, u: &Inputs, dt: f64) -> State {
    let next = integrator::rk4(*x, dt, |s| derivative(p, &s, u));
    State {
        p_im: next.p_im.max(1.0),
        p_em: next.p_em.max(1.0),
        omega_e: next.omega_e.max(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn omega(rpm: f64) -> f64 {
        rpm * std::f64::consts::TAU / 60.0
    }

    /// The published take-off rating: 132 kW and 550 N.m at the propeller, at a
    /// crank speed of 3880 rpm, burning 39 litres an hour.
    #[test]
    fn reproduces_the_published_rating_point() {
        let p = engines::ae330();
        let p_amb = atmosphere::isa(0.0).p;
        let x = State {
            p_im: p.control.map_setpoint_pa,
            p_em: 3.45e5,
            omega_e: omega(3880.0),
        };
        let u = Inputs {
            fuel_cmd: 1.0,
            w_intake: 0.0,
            p_amb,
            load_torque: 0.0,
        };
        let o = evaluate(&p, &x, &u);

        let power_error = (o.power_brake_w - 132_000.0).abs() / 132_000.0;
        let torque_error = (o.torque_prop - 550.0).abs() / 550.0;
        let fuel_error = (o.fuel_litres_per_hour(&p) - 39.0).abs() / 39.0;
        assert!(power_error < 0.10, "power {} W", o.power_brake_w);
        assert!(torque_error < 0.10, "prop torque {} N.m", o.torque_prop);
        assert!(fuel_error < 0.10, "fuel {} L/h", o.fuel_litres_per_hour(&p));
        assert!((o.rpm_prop - 2296.0).abs() < 5.0, "prop {} rpm", o.rpm_prop);

        let bsfc = o.bsfc_g_per_kwh().unwrap();
        assert!((200.0..280.0).contains(&bsfc), "bsfc {bsfc} g/kWh");
    }

    #[test]
    fn motoring_produces_negative_torque_and_no_bsfc() {
        let p = engines::ae330();
        let x = State::at_rest(101_325.0, 2000.0);
        let u = Inputs {
            fuel_cmd: 0.0,
            w_intake: 0.0,
            p_amb: 101_325.0,
            load_torque: 0.0,
        };
        let o = evaluate(&p, &x, &u);
        assert!(o.torque_brake < 0.0);
        assert!(o.bsfc_g_per_kwh().is_none());
        assert!(o.lambda.is_infinite());
    }

    #[test]
    fn state_stays_finite_over_a_full_load_step() {
        let p = engines::ae330();
        let p_amb = atmosphere::isa(0.0).p;
        let mut x = State::at_rest(p_amb, 2000.0);
        let mut u = Inputs {
            fuel_cmd: 0.0,
            w_intake: 0.05,
            p_amb,
            load_torque: 0.0,
        };
        for i in 0..4000 {
            if i == 1000 {
                u.fuel_cmd = 1.0;
                u.w_intake = 0.20;
            }
            x = step(&p, &x, &u, integrator::DT);
            u.load_torque = evaluate(&p, &x, &u).torque_brake;
            assert!(
                x.p_im.is_finite() && x.p_em.is_finite() && x.omega_e.is_finite(),
                "diverged at step {i}: {x:?}"
            );
        }
    }
}
