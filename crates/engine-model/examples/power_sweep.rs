//! Steady-state power and torque against crankshaft speed, at sea level.
//!
//! Writes CSV to stdout. Intake manifold pressure is prescribed at the control
//! set-point rather than produced by a compressor, which is what makes this a test
//! of the breathing, combustion and loss models alone. Once a turbocharger model is
//! attached the same sweep run at altitude produces the power lapse curve, and any
//! disagreement there is then known to be in the turbocharger rather than here.
//!
//! Exhaust manifold pressure is solved for equilibrium at each speed, so the
//! pumping term is the one the exhaust restriction actually implies.
//!
//! Two things this sweep is not. Below roughly 2500 rpm the prescribed boost is
//! more than a turbocharger of this size could actually deliver, so the torque
//! shown at the low end is an upper bound rather than a prediction. And fuelling is
//! held at full command all the way to the overspeed limit, with no engine control
//! schedule, so the peak of the curve lies above the rated speed and is not a
//! rating.

use engine_model::{EngineParams, Inputs, State, atmosphere, cylinder, engines, manifold};

fn main() {
    let p: EngineParams = engines::ae330();
    let p_amb = atmosphere::isa(0.0).p;
    let t_im = p.control.t_im_k;

    println!(
        "rpm,rpm_prop,map_bar,pem_bar,eta_vol,w_air_kgs,lambda,eta_ig,\
         torque_nm,torque_prop_nm,power_kw,power_hp,egt_k,fuel_lph,bsfc_gkwh"
    );

    let mut best = (0.0, 0.0);
    let mut rpm = 1500.0;
    while rpm <= p.limits.rpm_max + 1e-9 {
        let omega_e = rpm * std::f64::consts::TAU / 60.0;
        let p_im = p.control.map_setpoint_pa;

        let w_air = cylinder::air_flow(&p, p_im, omega_e, t_im);
        let u_f = cylinder::injected_fuel(&p, 1.0, w_air, omega_e);
        let w_fuel = cylinder::fuel_flow(&p, u_f, omega_e);

        let Some(p_em) = manifold::steady_exhaust_pressure(&p, w_air + w_fuel, p_amb, |pe| {
            cylinder::exhaust_temperature(&p, w_air, w_fuel, p_im, pe, t_im)
        }) else {
            eprintln!("no exhaust equilibrium at {rpm} rpm");
            rpm += 20.0;
            continue;
        };

        let x = State {
            p_im,
            p_em,
            omega_e,
        };
        let u = Inputs {
            fuel_cmd: 1.0,
            w_intake: w_air,
            p_amb,
            load_torque: 0.0,
        };
        let o = engine_model::evaluate(&p, &x, &u);

        let power_kw = o.power_brake_w / 1000.0;
        if power_kw > best.1 {
            best = (rpm, power_kw);
        }

        println!(
            "{rpm:.0},{:.0},{:.4},{:.4},{:.4},{:.5},{:.3},{:.4},\
             {:.2},{:.2},{:.3},{:.1},{:.1},{:.2},{:.1}",
            o.rpm_prop,
            p_im / 1e5,
            p_em / 1e5,
            o.eta_vol,
            o.w_air,
            o.lambda,
            o.eta_ig,
            o.torque_brake,
            o.torque_prop,
            power_kw,
            power_kw * 1000.0 / 745.6999,
            o.t_exhaust,
            o.fuel_litres_per_hour(&p),
            o.bsfc_g_per_kwh().unwrap_or(0.0),
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
