//! Transient response to a step in the fuelling command.
//!
//! Writes CSV to stdout. The engine starts at part load driving a propeller, the
//! FADEC command steps to full, and the model integrates turbocharger shaft
//! acceleration, manifold filling, the smoke limiter and crankshaft inertia through
//! the transient. Turbocharger lag is the shaft spinning up, not a lag constant.
//!
//! The propeller is treated as constant speed, which is what the real unit is: it
//! coarsens blade pitch to absorb exactly the torque the engine makes, so crank speed
//! is held through the step. That is deliberate here rather than incidental. With the
//! crank pinned, the only dynamics left are manifold filling and the turbocharger
//! shaft, so the boost trace **is** the spool and the lag can be read off it. A
//! fixed-pitch propeller would let the engine accelerate at the same time and the two
//! transients would be impossible to separate.

use engine_model::{EngineParams, Inputs, State, atmosphere, control, engines, integrator};

/// Simulated duration, s.
const DURATION: f64 = 10.0;
/// Fuelling command step time, s.
const STEP_AT: f64 = 2.0;

fn main() {
    let p: EngineParams = engines::ae330();
    let amb = atmosphere::isa(0.0);

    let mut x = State::at_rest(amb.p, 3000.0);
    let mut boost = control::BoostController::new();
    let mut u = Inputs {
        fuel_cmd: 0.30,
        wastegate: 1.0,
        p_amb: amb.p,
        t_amb: amb.t,
        load_torque: 0.0,
    };

    println!(
        "t_s,fuel_cmd,rpm,turbo_rpm,wastegate,map_bar,pem_bar,t_im_k,w_air_kgs,\
         lambda,torque_nm,power_kw,egt_k,eta_c,fuel_lph"
    );

    let mut boost_trace: Vec<(f64, f64)> = Vec::new();
    let steps = (DURATION / integrator::DT).round() as u32;
    for i in 0..=steps {
        let t = f64::from(i) * integrator::DT;
        if t >= STEP_AT {
            u.fuel_cmd = 1.0;
        }
        u.wastegate = boost.update(&p, x.p_im, x.omega_tc, integrator::DT);
        let o = engine_model::evaluate(&p, &x, &u);
        u.load_torque = o.torque_brake;
        if i % 10 == 0 {
            println!(
                "{t:.3},{:.2},{:.1},{:.0},{:.4},{:.4},{:.4},{:.1},{:.5},{:.3},\
                 {:.2},{:.3},{:.1},{:.4},{:.2}",
                u.fuel_cmd,
                x.rpm(),
                x.turbo_rpm(),
                u.wastegate,
                x.p_im / 1e5,
                x.p_em / 1e5,
                o.t_intake,
                o.w_air,
                o.lambda.min(99.0),
                o.torque_brake,
                o.power_brake_w / 1000.0,
                o.t_exhaust,
                o.eta_compressor,
                o.fuel_litres_per_hour(&p),
            );
        }

        boost_trace.push((t, x.p_im));
        x = engine_model::step(&p, &x, &u, integrator::DT);
    }

    // Spool time, reported rather than asserted: the interval from the fuelling step
    // to the moment boost first reaches 90% of the rise it eventually makes.
    let before = boost_trace
        .iter()
        .filter(|(t, _)| *t < STEP_AT)
        .map(|(_, p)| *p)
        .next_back()
        .unwrap_or(amb.p);
    let settled = boost_trace.last().map_or(amb.p, |(_, p)| *p);
    let target = before + 0.9 * (settled - before);
    if let Some((t, _)) = boost_trace
        .iter()
        .find(|(t, p)| *t >= STEP_AT && *p >= target)
    {
        eprintln!(
            "spool to 90% of {:.2} bar: {:.2} s",
            settled / 1e5,
            t - STEP_AT
        );
    }
}
