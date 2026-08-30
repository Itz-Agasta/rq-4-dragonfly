//! Steady state against fuelling demand, at one altitude and one crankshaft speed.
//!
//! Writes CSV to stdout. `power_sweep` answers "what can this engine do", by
//! sweeping speed at full command; this answers "what is it doing right now", by
//! holding speed and sweeping the load actuator. That is the sweep you need to
//! identify an observed operating point from a handful of reported channels.
//!
//! Every point is run to steady state with the boost controller closed and a
//! perfect governor holding crankshaft speed, so manifold pressure, turbocharger
//! speed and the thermal nodes are all whatever the engine actually settles at.

use engine_model::{EngineParams, Inputs, State, atmosphere, control, engines, integrator};

/// Simulated seconds allowed for each point to settle. The gas path is quiet in a
/// fraction of a second; the coolant and oil nodes have thermal time constants of a
/// minute or more, and it is those that set this.
const SETTLE_S: f64 = 400.0;

/// Pressure altitude of the condition being identified, metres.
const ALTITUDE_M: f64 = 6827.5;

/// Outside air temperature there, K. Not the standard-atmosphere value: this is a
/// reported observation, and the deviation from standard is part of the condition.
const OAT_K: f64 = 242.15;

/// True airspeed, m/s. Radiator and oil-cooler flow scale with it.
const TAS_M_S: f64 = 57.1;

/// Crankshaft speed held by the governor, rpm.
const RPM: f64 = 3720.0;

fn main() {
    let p: EngineParams = engines::ae330();
    let amb = atmosphere::isa(ALTITUDE_M);

    eprintln!(
        "alt {ALTITUDE_M:.0} m, p_amb {:.1} hPa, OAT {OAT_K:.2} K (ISA {:+.1} K), \
         TAS {TAS_M_S:.1} m/s, crank {RPM:.0} rpm",
        amb.p / 100.0,
        OAT_K - amb.t,
    );
    println!(
        "fuel_cmd,map_hpa,boost_bar,pem_bar,turbo_rpm,wastegate,w_air_kgs,lambda,\
         fuel_lph,power_kw,power_hp,pct_rated,torque_nm,torque_prop_nm,rpm_prop,\
         egt_k,egt1_k,egt2_k,egt3_k,egt4_k,cht_k,coolant_k,oil_k,oil_bar,\
         eta_vol,surge_margin,bsfc_gkwh"
    );

    let mut fuel_cmd = 0.05;
    while fuel_cmd <= 1.0 + 1e-9 {
        let mut x = State::at_rest(amb.p, RPM);
        let mut boost = control::BoostController::new();
        let mut u = Inputs {
            fuel_cmd,
            wastegate: 1.0,
            p_amb: amb.p,
            t_amb: OAT_K,
            tas_m_s: TAS_M_S,
            load_torque: 0.0,
        };

        for _ in 0..(SETTLE_S / integrator::DT) as u32 {
            u.wastegate = boost.update(&p, u.fuel_cmd, x.p_im, x.omega_tc, integrator::DT);
            // Perfect governor: the propeller absorbs exactly what the engine makes,
            // so crank speed is held while the turbocharger finds its equilibrium.
            u.load_torque = engine_model::evaluate(&p, &x, &u).torque_brake;
            x = engine_model::step(&p, &x, &u, integrator::DT);
        }

        let o = engine_model::evaluate(&p, &x, &u);
        let power_kw = o.power_brake_w / 1000.0;
        let power_hp = power_kw * 1000.0 / 745.6999;

        println!(
            "{fuel_cmd:.3},{:.1},{:.4},{:.4},{:.0},{:.4},{:.5},{:.3},{:.2},{:.3},{:.1},\
             {:.1},{:.2},{:.2},{:.0},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},\
             {:.3},{:.4},{:.3},{:.1}",
            x.p_im / 100.0,
            (x.p_im - amb.p) / 1e5,
            x.p_em / 1e5,
            x.turbo_rpm(),
            u.wastegate,
            o.w_air,
            o.lambda,
            o.fuel_litres_per_hour(&p),
            power_kw,
            power_hp,
            power_hp / 180.0 * 100.0,
            o.torque_brake,
            o.torque_prop,
            o.rpm_prop,
            o.t_exhaust,
            o.t_egt[0],
            o.t_egt[1],
            o.t_egt[2],
            o.t_egt[3],
            x.t_cht[0],
            x.t_coolant,
            x.t_oil,
            o.p_oil / 1e5,
            o.eta_vol,
            o.surge_margin,
            o.bsfc_g_per_kwh().unwrap_or(0.0),
        );
        fuel_cmd += 0.01;
    }
}
