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

/// Fuel mass flow for **one** cylinder, kg/s, from its injected mass per cycle.
#[must_use]
pub fn fuel_flow(p: &EngineParams, u_f_mg: f64, omega_e: f64) -> f64 {
    u_f_mg * 1e-6 * omega_e.max(0.0) / (2.0 * PI * p.geometry.revs_per_cycle)
}

/// Injected mass per cylinder after the per-cylinder injector scale, mg.
///
/// A healthy engine has every scale at unity and every entry equal. A coked or stuck
/// injector moves one of them, and note that the engine controller does **not**
/// compensate: a common-rail system without per-cylinder feedback cannot know, so
/// total fuel and therefore torque fall along with it. That is half of why an
/// injector fault is visible at all.
#[must_use]
pub fn per_cylinder_fuel(p: &EngineParams, u_f_mg: f64) -> [f64; crate::CYLINDERS] {
    std::array::from_fn(|i| u_f_mg * p.cylinder.injector_scale[i])
}

/// Fuel that releases its heat, from the fuel that was delivered.
///
/// Everything thermodynamic downstream takes this rather than the delivered
/// quantity: heat release, exhaust temperature, heat into the head, and the
/// oxygen a lambda probe finds left over. What still takes the delivered quantity
/// is the mass flow, because unburnt fuel leaves the tank and passes through the
/// exhaust all the same.
///
/// Post-oxidation in the manifold is deliberately **not** modelled. A diesel
/// exhaust carries enough excess oxygen at 800 K for some of the unburnt charge to
/// burn on its way to the turbine, which would recover part of the temperature
/// drop, but nothing here could calibrate how much and guessing would soften the
/// one signature misfire is diagnosed by.
/// Unit agnostic: `delivered` may be a mass flow or a mass per cycle, and both
/// callers exist.
#[must_use]
pub fn burned_fuel(
    p: &EngineParams,
    delivered: &[f64; crate::CYLINDERS],
) -> [f64; crate::CYLINDERS] {
    std::array::from_fn(|i| delivered[i] * p.cylinder.combustion_efficiency[i])
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

/// Gross indicated mean effective pressure, Pa, summed over the cylinders.
///
/// Each cylinder contributes its own heat release at its own efficiency, so a
/// cylinder running lean contributes less. Summing rather than scaling a mean is
/// what lets a single-cylinder fault reach brake torque.
///
/// The quantity is the fuel that **burned**, not the fuel that was delivered; see
/// [`burned_fuel`]. A misfiring cylinder does no indicated work on the charge it
/// was given.
#[must_use]
pub fn imep_gross(
    p: &EngineParams,
    eta_ig: &[f64; crate::CYLINDERS],
    u_f_burned_mg: &[f64; crate::CYLINDERS],
) -> f64 {
    let work: f64 = (0..crate::CYLINDERS)
        .map(|i| eta_ig[i] * u_f_burned_mg[i] * 1e-6)
        .sum();
    work * p.fuel.lhv_j_per_kg / p.geometry.displacement_m3
}

/// Temperature of the gas leaving the cylinders, K.
///
/// An energy balance over compression, constant-volume heat addition and expansion
/// to the exhaust manifold pressure. It carries the two dependences that matter for
/// diagnosis: exhaust temperature rises when a cylinder runs lean at constant air
/// flow, and rises with exhaust back pressure.
///
/// `w_fuel` is what was delivered and `w_fuel_burned` is what released its heat.
/// They differ only on a misfiring cylinder, and separating them is what stops
/// unburnt fuel from heating a gas it never ignited in while still counting towards
/// the mass being expanded.
///
/// Ekberg, Leek & Eriksson, "Validation of an Open-Source Mean-Value Heavy-Duty
/// Diesel Engine Model", SIMS 59, 2018, eq. 21.
/// https://doi.org/10.3384/ecp18153290
#[must_use]
pub fn exhaust_temperature(
    p: &EngineParams,
    w_air: f64,
    w_fuel: f64,
    w_fuel_burned: f64,
    p_im: f64,
    p_em: f64,
    t_im: f64,
) -> f64 {
    let total = w_air + w_fuel;
    if total <= 0.0 || p_im <= 0.0 {
        return t_im;
    }
    let q_in = w_fuel_burned / total * p.fuel.lhv_j_per_kg;
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
        let lam = lambda(&p, w_air_thin, 4.0 * fuel_flow(&p, u_f, omega(3880.0)));
        assert!((lam - p.limits.lambda_min).abs() < 1e-9, "lambda {lam}");
    }

    #[test]
    fn full_command_is_unclipped_at_the_rating_point() {
        let p = engines::ae330();
        let om = omega(3880.0);
        let w_air = air_flow(&p, p.control.map_setpoint_pa, om, 320.0);
        let u_f = injected_fuel(&p, 1.0, w_air, om);
        assert!((u_f - p.cylinder.u_f_max_mg).abs() < 1e-9, "u_f {u_f}");
        let lam = lambda(&p, w_air, 4.0 * fuel_flow(&p, u_f, om));
        assert!((1.45..1.65).contains(&lam), "lambda {lam}");
    }

    #[test]
    fn fuel_flow_matches_the_published_consumption() {
        // The factsheet gives 39 L/h at 100% power. At 800 kg/m3 that is 8.67 g/s.
        let p = engines::ae330();
        let om = omega(3880.0);
        let w_f = 4.0 * fuel_flow(&p, p.cylinder.u_f_max_mg, om);
        let litres_per_hour = w_f / p.fuel.density_kg_m3 * 3600.0 * 1000.0;
        assert!(
            (litres_per_hour - 39.0).abs() < 2.0,
            "{litres_per_hour} L/h"
        );
    }

    #[test]
    fn a_starved_cylinder_lowers_the_summed_indicated_work() {
        let p = engines::ae330();
        let eta = [0.39; crate::CYLINDERS];
        let healthy = [67.4; crate::CYLINDERS];
        let mut coked = healthy;
        coked[2] = 55.0;
        assert!(imep_gross(&p, &eta, &coked) < imep_gross(&p, &eta, &healthy));
    }

    #[test]
    fn indicated_efficiency_sits_where_a_diesel_should() {
        let p = engines::ae330();
        let eta = indicated_efficiency(&p, 1.55);
        assert!((0.36..0.44).contains(&eta), "eta_ig {eta}");
    }

    /// The signature every injection fault is diagnosed by, and its direction is
    /// the opposite of the spark-ignition intuition.
    ///
    /// On a spark-ignition engine the mixture sits near stoichiometric and peak
    /// exhaust temperature lies just lean of it, so leaning from rich runs the
    /// cylinder hotter. A compression-ignition engine runs between about 1.3 and
    /// 5 excess air, which is far lean of any peak: heat release falls
    /// monotonically with fuel from there, and so does exhaust temperature.
    ///
    /// So a coked injector, which delivers less fuel than commanded, makes its
    /// cylinder **cooler** than its neighbours. Getting this backwards would make
    /// the whole fault library look for the wrong sign.
    #[test]
    fn leaning_a_cylinder_lowers_its_exhaust_temperature() {
        let p = engines::ae330();
        let nominal = exhaust_temperature(&p, 0.20, 0.0088, 0.0088, 3.1e5, 3.45e5, 320.0);
        let coked =
            exhaust_temperature(&p, 0.20, 0.0088 * 0.84, 0.0088 * 0.84, 3.1e5, 3.45e5, 320.0);
        assert!(
            coked < nominal,
            "coked {coked} should be cooler than nominal {nominal}"
        );
        // 700 to 1150 K is the cylinder-out band a turbocharged diesel runs at
        // before the exhaust manifold cools the gas on its way to the turbine.
        assert!((700.0..1150.0).contains(&nominal), "T_e {nominal}");
        assert!((700.0..1150.0).contains(&coked), "T_e coked {coked}");
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
        assert!(per_cylinder_fuel(&p, 0.0).iter().all(|v| v.is_finite()));
        assert!(indicated_efficiency(&p, f64::INFINITY).is_finite());
        assert!(exhaust_temperature(&p, 0.0, 0.0, 0.0, 0.0, 0.0, 320.0).is_finite());
    }

    /// The pair of faults a threshold monitor cannot separate, separated.
    ///
    /// Both make one cylinder cooler and lean. What differs is upstream of the
    /// cylinder: a restricted nozzle never delivers the fuel, so it never leaves the
    /// tank, while a misfiring cylinder is fuelled normally and passes it through
    /// unburnt. Fuel flow is the discriminating channel and this is where that
    /// becomes true of the model rather than of the prose.
    #[test]
    fn misfire_and_coking_differ_on_delivered_fuel_and_agree_on_everything_else() {
        let p = engines::ae330();
        let (air, fuel) = (0.20, 0.0088);
        let severity = 0.16;

        let coked = exhaust_temperature(
            &p,
            air,
            fuel * (1.0 - severity),
            fuel * (1.0 - severity),
            3.1e5,
            3.45e5,
            320.0,
        );
        let misfiring =
            exhaust_temperature(&p, air, fuel, fuel * (1.0 - severity), 3.1e5, 3.45e5, 320.0);
        let nominal = exhaust_temperature(&p, air, fuel, fuel, 3.1e5, 3.45e5, 320.0);

        assert!(misfiring < nominal, "misfire must run the exhaust cooler");
        // Within a few kelvin of each other: the extra unburnt mass in the misfiring
        // case dilutes the same heat release over a slightly larger stream.
        assert!((misfiring - coked).abs() < 5.0, "{misfiring} vs {coked}");

        let mut misfire_params = p.clone();
        misfire_params.cylinder.combustion_efficiency[2] = 1.0 - severity;
        let burned = burned_fuel(&misfire_params, &[fuel; crate::CYLINDERS]);
        assert!((burned[2] - fuel * (1.0 - severity)).abs() < 1e-15);
        for i in [0, 1, 3] {
            assert!((burned[i] - fuel).abs() < 1e-15, "cylinder {i} moved");
        }
    }
}
