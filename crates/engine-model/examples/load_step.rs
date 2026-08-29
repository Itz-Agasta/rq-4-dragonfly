//! Transient response to a step in the fuelling command.
//!
//! Writes CSV to stdout. The engine starts at part load driving a propeller, the
//! FADEC command steps to full, and the model integrates manifold filling, the
//! smoke limiter and crankshaft inertia through the transient.
//!
//! Two things here are stand-ins rather than model, and both are outside the crate
//! for exactly that reason.
//!
//! The flow delivered into the intake manifold follows the demand at the boost
//! set-point through a first-order lag. A turbocharger does not respond instantly
//! because its shaft has to accelerate, and until a turbocharger model exists that
//! delay has to come from somewhere; a lag is the honest cheapest version of it and
//! it is what makes the excess air ratio dip after the step. Replace it, do not tune
//! it.
//!
//! The propeller is treated as fixed pitch, absorbing torque proportional to the
//! square of speed. The real unit is constant speed and would hold rpm through the
//! step by coarsening blade pitch, which is a governor and belongs with the airframe
//! rather than with the engine.

use engine_model::{EngineParams, Inputs, State, atmosphere, engines, integrator};

/// Time constant of the intake flow stand-in, s.
const SPOOL_TAU: f64 = 0.9;
/// Simulated duration, s.
const DURATION: f64 = 8.0;
/// Fuelling command step time, s.
const STEP_AT: f64 = 1.0;

fn main() {
    let p: EngineParams = engines::ae330();
    let p_amb = atmosphere::isa(0.0).p;

    // Fixed-pitch propeller constant, sized to absorb the rated torque at the rated
    // speed so the engine settles near its rating rather than at an arbitrary point.
    let rated_omega = 3880.0 * std::f64::consts::TAU / 60.0;
    let k_prop = 325.0 / (rated_omega * rated_omega);

    let mut x = State::at_rest(p_amb, 2000.0);
    x.p_im = 1.6e5;
    let mut w_intake = 0.10;
    let mut u = Inputs {
        fuel_cmd: 0.30,
        w_intake,
        p_amb,
        load_torque: 0.0,
    };

    println!(
        "t_s,fuel_cmd,rpm,map_bar,pem_bar,w_air_kgs,w_intake_kgs,lambda,\
         torque_nm,power_kw,egt_k,fuel_lph"
    );

    let steps = (DURATION / integrator::DT).round() as u32;
    for i in 0..=steps {
        let t = f64::from(i) * integrator::DT;
        if t >= STEP_AT {
            u.fuel_cmd = 1.0;
        }

        let o = engine_model::evaluate(&p, &x, &u);
        u.load_torque = k_prop * x.omega_e * x.omega_e;

        if i % 10 == 0 {
            println!(
                "{t:.3},{:.2},{:.1},{:.4},{:.4},{:.5},{:.5},{:.3},{:.2},{:.3},{:.1},{:.2}",
                u.fuel_cmd,
                x.rpm(),
                x.p_im / 1e5,
                x.p_em / 1e5,
                o.w_air,
                u.w_intake,
                o.lambda.min(99.0),
                o.torque_brake,
                o.power_brake_w / 1000.0,
                o.t_exhaust,
                o.fuel_litres_per_hour(&p),
            );
        }

        // Intake flow relaxes towards what the cylinders would swallow at the boost
        // set-point. See the module note: this is a placeholder for shaft dynamics.
        let demand = engine_model::cylinder::air_flow(
            &p,
            p.control.map_setpoint_pa,
            x.omega_e,
            p.control.t_im_k,
        );
        w_intake += (demand - w_intake) * integrator::DT / SPOOL_TAU;
        u.w_intake = w_intake;

        x = engine_model::step(&p, &x, &u, integrator::DT);
    }
}
