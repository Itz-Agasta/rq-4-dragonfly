//! Steady-state power and torque against crankshaft speed, at sea level.
//!
//! Writes CSV to stdout. Every speed is run to steady state with the boost
//! controller closed, so manifold pressure is whatever the turbocharger actually
//! delivers rather than a prescribed value.
//!
//! Fuelling is held at full command all the way to the overspeed limit, with no
//! engine control schedule, so the peak of the curve lies above the rated speed and
//! is not a rating.

use engine_model::{EngineParams, Inputs, State, atmosphere, control, engines, integrator};

/// Simulated seconds allowed for each speed to settle. Long, because the coolant
/// and oil nodes have thermal time constants of a minute or more; the gas path
/// alone would be settled in a fraction of a second.
const SETTLE_S: f64 = 400.0;

fn main() {
    let p: EngineParams = engines::ae330();
    let amb = atmosphere::isa(0.0);

    println!(
        "rpm,rpm_prop,map_bar,boost_bar,pem_bar,t_im_k,turbo_rpm,wastegate,eta_vol,\
         w_air_kgs,lambda,eta_ig,torque_nm,torque_prop_nm,power_kw,power_hp,\
         egt_k,eta_c,eta_t,surge_margin,fuel_lph,bsfc_gkwh,\
         cht_k,coolant_k,oil_k,oil_bar"
    );

    let mut best = (0.0, 0.0);
    let mut rpm = 1500.0;
    while rpm <= p.limits.rpm_max + 1e-9 {
        let mut x = State::at_rest(amb.p, rpm);
        let mut boost = control::BoostController::new();
        let mut u = Inputs {
            fuel_cmd: 1.0,
            wastegate: 1.0,
            p_amb: amb.p,
            t_amb: amb.t,
            tas_m_s: 60.0,
            load_torque: 0.0,
        };

        for _ in 0..(SETTLE_S / integrator::DT) as u32 {
            u.wastegate = boost.update(&p, u.fuel_cmd, x.p_im, x.omega_tc, integrator::DT);
            let o = engine_model::evaluate(&p, &x, &u);
            // Perfect governor: the propeller takes exactly what the engine makes, so
            // crank speed is held while the turbocharger finds its own equilibrium.
            u.load_torque = o.torque_brake;
            x = engine_model::step(&p, &x, &u, integrator::DT);
        }

        let o = engine_model::evaluate(&p, &x, &u);
        let power_kw = o.power_brake_w / 1000.0;
        if power_kw > best.1 {
            best = (rpm, power_kw);
        }

        println!(
            "{rpm:.0},{:.0},{:.4},{:.4},{:.4},{:.1},{:.0},{:.4},{:.4},{:.5},{:.3},{:.4},\
             {:.2},{:.2},{:.3},{:.1},{:.1},{:.4},{:.4},{:.3},{:.2},{:.1},{:.1},{:.1},{:.1},{:.3}",
            o.rpm_prop,
            x.p_im / 1e5,
            (x.p_im - amb.p) / 1e5,
            x.p_em / 1e5,
            o.t_intake,
            x.turbo_rpm(),
            u.wastegate,
            o.eta_vol,
            o.w_air,
            o.lambda,
            o.eta_ig_mean(),
            o.torque_brake,
            o.torque_prop,
            power_kw,
            power_kw * 1000.0 / 745.6999,
            o.t_exhaust,
            o.eta_compressor,
            o.eta_turbine,
            o.surge_margin,
            o.fuel_litres_per_hour(&p),
            o.bsfc_g_per_kwh().unwrap_or(0.0),
            x.t_cht[0],
            x.t_coolant,
            x.t_oil,
            o.p_oil / 1e5,
        );
        rpm += 20.0;
    }

    eprintln!(
        "unlimited full command: {:.1} kW ({:.0} hp) at {:.0} rpm",
        best.1,
        best.1 * 1000.0 / 745.6999,
        best.0
    );
}
