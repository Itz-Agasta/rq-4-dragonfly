//! Breathing, fuelling and gross indicated work.
//!
//! On a compression-ignition engine the air path is unthrottled and load is set by
//! injected fuel quantity, so this module has no throttle term: air flow follows
//! from manifold pressure and speed, fuel follows from the injection command, and
//! the excess air ratio is whatever those two produce. That is the opposite causal
//! direction from a spark-ignition engine and it is why the fault modes differ.
//!
//! Flow and efficiency are parametric functions rather than lookup tables. A table
//! fitted to data we do not have would be a surface nobody can defend; a three-term
//! parametric form has three numbers, each of which can be argued about.
//!
//! Wahlstrom & Eriksson, "Modelling diesel engines with a variable-geometry
//! turbocharger and exhaust gas recirculation by optimization of model parameters
//! for capturing non-linear system dynamics", Proc IMechE Part D 225(7), 2011.
//! https://doi.org/10.1177/0954407011398177

use crate::EngineParams;
use std::f64::consts::PI;

/// Volumetric efficiency at a manifold pressure and crank speed.
///
/// The square-root form is fitted over the cruise-to-take-off band and extrapolates
/// badly below about 1500 rpm, where it climbs through unity. The clamp is a guard
/// against that extrapolation, not a physical statement: a well scavenged
/// turbocharged engine really can exceed unity, so raise `eta_vol_max` rather than
/// assuming the clamp is correct if the low-speed region ever starts to matter.
#[must_use]
pub fn volumetric_efficiency(p: &EngineParams, p_im: f64, omega_e: f64) -> f64 {
    let c = p.cylinder.c_vol;
    let eta = c[0] * p_im.max(0.0).sqrt() + c[1] * omega_e.max(0.0).sqrt() + c[2];
    eta.clamp(0.0, p.cylinder.eta_vol_max)
}

/// Air mass flow drawn into the cylinders, kg/s.
#[must_use]
pub fn air_flow(p: &EngineParams, p_im: f64, omega_e: f64, t_im: f64) -> f64 {
    if t_im <= 0.0 {
        return 0.0;
    }
    let eta_vol = volumetric_efficiency(p, p_im, omega_e);
    eta_vol * p_im.max(0.0) * omega_e.max(0.0) * p.geometry.displacement_m3
        / (2.0 * PI * p.geometry.revs_per_cycle * p.gas.r_air * t_im)
}

/// Injected fuel mass per cylinder per cycle, mg, after the smoke limit.
///
/// Clipping fuel to hold a minimum excess air ratio is what a real FADEC does, and
/// it is the reason a turbocharged engine loses power at altitude in a soft roll-off
/// rather than simply producing black smoke. Without it the model would happily
/// report full torque at 30,000 ft.
#[must_use]
pub fn injected_fuel(p: &EngineParams, fuel_cmd: f64, w_air: f64, omega_e: f64) -> f64 {
    let commanded = fuel_cmd.clamp(0.0, 1.0) * p.cylinder.u_f_max_mg;
    if omega_e <= 0.0 {
        return 0.0;
    }
    let smoke_limited = w_air * 2.0 * PI * p.geometry.revs_per_cycle
        / (p.fuel.stoich_afr * p.limits.lambda_min * 1e-6 * p.geometry.n_cyl * omega_e);
    commanded.min(smoke_limited).max(0.0)
}

/// Fuel mass flow, kg/s, from injected mass per cylinder per cycle.
#[must_use]
pub fn fuel_flow(p: &EngineParams, u_f_mg: f64, omega_e: f64) -> f64 {
    u_f_mg * 1e-6 * p.geometry.n_cyl * omega_e.max(0.0) / (2.0 * PI * p.geometry.revs_per_cycle)
}

/// Excess air ratio. Returns `f64::INFINITY` on zero fuel, which is the physically
/// correct limit for a motoring engine and keeps the caller from dividing by zero.
#[must_use]
pub fn lambda(p: &EngineParams, w_air: f64, w_fuel: f64) -> f64 {
    if w_fuel <= 0.0 {
        return f64::INFINITY;
    }
    w_air / (p.fuel.stoich_afr * w_fuel)
}

/// Gross indicated efficiency.
///
/// Two factors, both driven by the fuel/air equivalence ratio. The ideal
/// constant-volume cycle efficiency supplies the compression ratio dependence, with
/// the ratio of specific heats falling as the charge is enriched because burned gas
/// holds more polyatomic species. The combustion-quality island supplies the rest:
/// efficiency is poor at very light load, where heat loss is a large fraction of a
/// small heat release, peaks around an excess air ratio of 1.8, and falls away again
/// approaching the smoke limit as combustion becomes incomplete.
///
/// Equivalence ratio is the load coordinate rather than injected mass per unit crank
/// speed, which is what the cited work uses. That coordinate is fitted over a narrow
/// speed band and it diverges outside it: at a quarter of rated speed with full
/// fuelling it puts this engine four times past the far zero of the parabola and
/// returns an efficiency near zero. Equivalence ratio is bounded by the smoke limit
/// at one end and by zero fuelling at the other, so it cannot leave the fitted range
/// however the engine is driven.
///
/// The ideal cycle used is the constant-volume one, despite this being a
/// compression-ignition engine. That is deliberate and follows the cited work: it
/// makes the compression ratio dependence a one-parameter model, and everything a
/// constant-pressure cycle would add is absorbed into the calibration island, which
/// has to be fitted anyway.
#[must_use]
pub fn indicated_efficiency(p: &EngineParams, lambda: f64) -> f64 {
    let phi = if lambda.is_finite() && lambda > 0.0 {
        1.0 / lambda
    } else {
        0.0
    };

    let isl = p.cylinder.eta_ig_island;
    let (eta_0, eta_peak, phi_peak) = (isl[0], isl[1], isl[2]);
    let a = (eta_peak - eta_0) / -(phi_peak * phi_peak);
    let b = -2.0 * a * phi_peak;
    let eta_cal = (a * phi * phi + b * phi + eta_0).max(0.0);

    let gamma_cyl = p.cylinder.gamma_cyl[0] + p.cylinder.gamma_cyl[1] * phi;
    let eta_ideal = 1.0 - p.geometry.compression_ratio.powf(1.0 - gamma_cyl);

    (eta_cal * eta_ideal).clamp(0.0, 1.0)
}

/// Gross indicated mean effective pressure, Pa.
#[must_use]
pub fn imep_gross(p: &EngineParams, eta_ig: f64, u_f_mg: f64) -> f64 {
    eta_ig * p.fuel.lhv_j_per_kg * u_f_mg * 1e-6 * p.geometry.n_cyl / p.geometry.displacement_m3
}

/// Temperature of the gas leaving the cylinders, K.
///
/// An energy balance over compression, constant-volume heat addition and expansion
/// to the exhaust manifold pressure. It carries the two dependences that matter for
/// diagnosis: exhaust temperature rises when a cylinder runs lean at constant air
/// flow, and rises with exhaust back pressure.
///
/// Ekberg, Leek & Eriksson, "Validation of an Open-Source Mean-Value Heavy-Duty
/// Diesel Engine Model", SIMS 59, 2018, eq. 21.
/// https://doi.org/10.3384/ecp18153290
#[must_use]
pub fn exhaust_temperature(
    p: &EngineParams,
    w_air: f64,
    w_fuel: f64,
    p_im: f64,
    p_em: f64,
    t_im: f64,
) -> f64 {
    let total = w_air + w_fuel;
    if total <= 0.0 || p_im <= 0.0 {
        return t_im;
    }
    let q_in = w_fuel / total * p.fuel.lhv_j_per_kg;
    let gamma = p.gas.gamma_air;
    let r_c = p.geometry.compression_ratio;
    let pressure_ratio = (p_em / p_im).max(1e-6);

    p.cylinder.eta_sc
        * pressure_ratio.powf(1.0 - 1.0 / gamma)
        * r_c.powf(1.0 - gamma)
        * (q_in / p.gas.cp_air + t_im * r_c.powf(gamma - 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines;

    fn omega(rpm: f64) -> f64 {
        rpm * 2.0 * PI / 60.0
    }

    #[test]
    fn volumetric_efficiency_is_plausible_at_the_rating_point() {
        let p = engines::ae330();
        let eta = volumetric_efficiency(&p, p.control.map_setpoint_pa, omega(3880.0));
        assert!((0.85..0.96).contains(&eta), "eta_vol {eta}");
    }

    #[test]
    fn volumetric_efficiency_never_leaves_its_clamp() {
        let p = engines::ae330();
        for rpm in [0.0, 500.0, 2000.0, 4220.0, 10_000.0] {
            for p_im in [0.0, 1e4, 1e5, 5e5] {
                let eta = volumetric_efficiency(&p, p_im, omega(rpm));
                assert!(
                    (0.0..=p.cylinder.eta_vol_max).contains(&eta),
                    "{rpm} {p_im} {eta}"
                );
            }
        }
    }

    #[test]
    fn air_flow_matches_a_hand_computed_case() {
        // eta_vol p_im omega V_d / (2 pi n_r R T). At 3.10 bar, 3880 rpm, 320 K the
        // volumetric efficiency is 0.9214, giving 0.2002 kg/s by hand.
        let p = engines::ae330();
        let w = air_flow(&p, 3.10e5, omega(3880.0), 320.0);
        assert!((w - 0.2002).abs() < 2e-3, "w_air {w}");
    }

    #[test]
    fn smoke_limit_clips_fuel_when_the_air_runs_out() {
        let p = engines::ae330();
        let w_air_thin = 0.05;
        let u_f = injected_fuel(&p, 1.0, w_air_thin, omega(3880.0));
        assert!(u_f < p.cylinder.u_f_max_mg, "expected clipping, got {u_f}");
        let lam = lambda(&p, w_air_thin, fuel_flow(&p, u_f, omega(3880.0)));
        assert!((lam - p.limits.lambda_min).abs() < 1e-9, "lambda {lam}");
    }

    #[test]
    fn full_command_is_unclipped_at_the_rating_point() {
        let p = engines::ae330();
        let om = omega(3880.0);
        let w_air = air_flow(&p, p.control.map_setpoint_pa, om, 320.0);
        let u_f = injected_fuel(&p, 1.0, w_air, om);
        assert!((u_f - p.cylinder.u_f_max_mg).abs() < 1e-9, "u_f {u_f}");
        let lam = lambda(&p, w_air, fuel_flow(&p, u_f, om));
        assert!((1.45..1.65).contains(&lam), "lambda {lam}");
    }

    #[test]
    fn fuel_flow_matches_the_published_consumption() {
        // The factsheet gives 39 L/h at 100% power. At 800 kg/m3 that is 8.67 g/s.
        let p = engines::ae330();
        let om = omega(3880.0);
        let w_f = fuel_flow(&p, p.cylinder.u_f_max_mg, om);
        let litres_per_hour = w_f / p.fuel.density_kg_m3 * 3600.0 * 1000.0;
        assert!(
            (litres_per_hour - 39.0).abs() < 2.0,
            "{litres_per_hour} L/h"
        );
    }

    #[test]
    fn indicated_efficiency_sits_where_a_diesel_should() {
        let p = engines::ae330();
        let eta = indicated_efficiency(&p, 1.55);
        assert!((0.36..0.44).contains(&eta), "eta_ig {eta}");
    }

    #[test]
    fn leaning_a_cylinder_raises_its_exhaust_temperature() {
        // The signature every injector fault is diagnosed by: at constant air flow,
        // less fuel means a hotter, not cooler, exhaust, because the ratio of
        // specific heats rises faster than the heat release falls.
        let p = engines::ae330();
        let hot = exhaust_temperature(&p, 0.20, 0.0080, 3.1e5, 3.45e5, 320.0);
        let hotter = exhaust_temperature(&p, 0.20, 0.0088, 3.1e5, 3.45e5, 320.0);
        assert!(hotter > hot);
        assert!((700.0..1100.0).contains(&hot), "T_e {hot}");
    }

    #[test]
    fn indicated_efficiency_stays_sane_across_the_whole_operating_envelope() {
        // The failure this replaced: the previous load coordinate returned 0.035 at
        // low speed and full fuelling, which would have been read as a real torque
        // deficit by anything downstream.
        let p = engines::ae330();
        for lambda in [1.30, 1.5, 1.8, 2.5, 5.0, 20.0] {
            let eta = indicated_efficiency(&p, lambda);
            assert!(
                (0.25..0.55).contains(&eta),
                "lambda {lambda} gave eta_ig {eta}"
            );
        }
        // Peak efficiency sits between the smoke limit and light load, not at either.
        let peak = indicated_efficiency(&p, 1.0 / p.cylinder.eta_ig_island[2]);
        assert!(peak > indicated_efficiency(&p, p.limits.lambda_min));
        assert!(peak > indicated_efficiency(&p, 6.0));
    }

    #[test]
    fn nothing_returns_nan_at_zero_speed_or_zero_flow() {
        let p = engines::ae330();
        assert!(air_flow(&p, 0.0, 0.0, 320.0).is_finite());
        assert!(injected_fuel(&p, 1.0, 0.0, 0.0).is_finite());
        assert!(fuel_flow(&p, 0.0, 0.0).is_finite());
        assert!(indicated_efficiency(&p, f64::INFINITY).is_finite());
        assert!(exhaust_temperature(&p, 0.0, 0.0, 0.0, 0.0, 320.0).is_finite());
    }
}
