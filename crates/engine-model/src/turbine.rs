//! Radial turbine and the wastegate that bypasses it.
//!
//! Both are restrictions out of the exhaust manifold and both use the same
//! dimensionless flow function, so they live together: separating them would put six
//! lines of wastegate in a different file from the pressure ratio they depend on.
//!
//! Turbine efficiency is a parabola in **blade speed ratio**, the wheel tip speed
//! divided by the spouting velocity the available expansion would produce. That is
//! the correct similarity variable for a radial turbine and it is why efficiency
//! falls off at both ends: too slow and the gas is not turned, too fast and the wheel
//! outruns the flow.
//!
//! Sivertsson & Eriksson, "Modeling for optimal control: a validated diesel-electric
//! powertrain model", SIMS 55, 2014.
//! <http://www.fs.isy.liu.se/Publications/Articles/SIMS_14_MS_LE_2.pdf>

use crate::EngineParams;

/// Turbine operating point.
#[derive(Clone, Copy, Debug, Default)]
pub struct Operation {
    /// Expansion pressure ratio, downstream over upstream. Below 1 in operation.
    pub pressure_ratio: f64,
    /// Mass flow through the turbine, kg/s.
    pub mass_flow: f64,
    /// Blade speed ratio.
    pub blade_speed_ratio: f64,
    /// Combined isentropic and mechanical efficiency.
    pub efficiency: f64,
    /// Shaft power delivered, W.
    pub power: f64,
}

/// Specific heat at constant pressure of the exhaust gas, J/(kg K).
///
/// Derived from the gas constant and the ratio of specific heats rather than carried
/// as a separate parameter, so the three can never be edited into disagreement.
#[must_use]
pub fn cp_exhaust(p: &EngineParams) -> f64 {
    p.gas.gamma_exh * p.gas.r_exh / (p.gas.gamma_exh - 1.0)
}

/// The dimensionless flow function shared by the turbine and the wastegate.
fn psi(pressure_ratio: f64, coefficients: [f64; 2]) -> f64 {
    coefficients[0] * (1.0 - pressure_ratio.powf(coefficients[1])).max(0.0).sqrt()
}

/// Solve the turbine at one operating point.
///
/// Returns an all-zero operation when the manifold is at or below the downstream
/// pressure. There is nothing to expand, and the alternative is a square root of a
/// negative number in the flow function and a division by zero in the blade speed
/// ratio.
#[must_use]
pub fn operate(
    p: &EngineParams,
    omega_tc: f64,
    p_em: f64,
    p_downstream: f64,
    t_em: f64,
) -> Operation {
    let t = &p.turbine;
    if p_em <= p_downstream || t_em <= 0.0 {
        return Operation::default();
    }
    let pressure_ratio = (p_downstream / p_em).clamp(0.0, 1.0);
    let kappa = (p.gas.gamma_exh - 1.0) / p.gas.gamma_exh;
    let cp = cp_exhaust(p);

    let mass_flow =
        p_em * psi(pressure_ratio, t.c_flow) * t.area_eff_m2 / (t_em * p.gas.r_exh).sqrt();

    let available = 1.0 - pressure_ratio.powf(kappa);
    let spouting = (2.0 * cp * t_em * available).max(1e-9).sqrt();
    let blade_speed_ratio = t.r_wheel_m * omega_tc / spouting;

    let off = blade_speed_ratio - t.bsr_opt;
    // Floored rather than allowed negative: a turbine far off its design blade speed
    // ratio makes very little power, but it never drives the shaft backwards.
    let efficiency = (t.eta_max - t.c_bsr * off * off).clamp(0.0, 0.95);

    Operation {
        pressure_ratio,
        mass_flow,
        blade_speed_ratio,
        efficiency,
        power: mass_flow * cp * t_em * efficiency * available,
    }
}

/// Mass flow bypassing the turbine through the wastegate, kg/s.
///
/// `command` is the actuator position, 0 shut to 1 fully open. Below the critical
/// altitude the boost controller holds it partly open and dumps energy; the altitude
/// at which it reaches zero **is** the critical altitude, and above that there is
/// nothing left to close and boost falls away.
#[must_use]
pub fn wastegate_flow(
    p: &EngineParams,
    command: f64,
    p_em: f64,
    p_downstream: f64,
    t_em: f64,
) -> f64 {
    if p_em <= p_downstream || t_em <= 0.0 {
        return 0.0;
    }
    let pressure_ratio = (p_downstream / p_em).clamp(0.0, 1.0);
    p_em * psi(pressure_ratio, p.wastegate.c_flow)
        * p.wastegate.area_eff_m2
        * command.clamp(0.0, 1.0)
        / (t_em * p.gas.r_exh).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines;

    #[test]
    fn derived_exhaust_cp_matches_the_literature_value() {
        // 286 J/(kg K) and 1.2734 give 1332 J/(kg K), which is the figure the mean
        // value diesel literature quotes for burned gas.
        let p = engines::ae330();
        assert!((cp_exhaust(&p) - 1332.0).abs() < 1.0, "{}", cp_exhaust(&p));
    }

    #[test]
    fn no_expansion_means_no_flow_and_no_power() {
        let p = engines::ae330();
        let o = operate(&p, 14_000.0, 101_325.0, 101_325.0, 900.0);
        assert!(o.mass_flow.abs() < 1e-12 && o.power.abs() < 1e-12);
        let backwards = operate(&p, 14_000.0, 90_000.0, 101_325.0, 900.0);
        assert!(backwards.power.abs() < 1e-12);
        assert!(wastegate_flow(&p, 1.0, 90_000.0, 101_325.0, 900.0).abs() < 1e-12);
    }

    #[test]
    fn power_and_flow_rise_with_manifold_pressure() {
        let p = engines::ae330();
        let low = operate(&p, 14_000.0, 2.0e5, 101_325.0, 900.0);
        let high = operate(&p, 14_000.0, 3.5e5, 101_325.0, 900.0);
        assert!(high.mass_flow > low.mass_flow);
        assert!(high.power > low.power);
    }

    #[test]
    fn efficiency_peaks_at_the_design_blade_speed_ratio() {
        let p = engines::ae330();
        let at = |w: f64| operate(&p, w, 3.0e5, 101_325.0, 900.0);
        // Find the speed that puts the blade speed ratio on its optimum, then check
        // efficiency falls off on both sides of it.
        let mut best = (0.0, 0.0);
        let mut w = 1_000.0;
        while w < 25_000.0 {
            let o = at(w);
            if o.efficiency > best.1 {
                best = (w, o.efficiency);
            }
            w += 100.0;
        }
        assert!((best.1 - p.turbine.eta_max).abs() < 1e-3, "peak {}", best.1);
        assert!(at(best.0 - 4_000.0).efficiency < best.1);
        assert!(at(best.0 + 4_000.0).efficiency < best.1);
    }

    #[test]
    fn wastegate_command_scales_its_flow_linearly() {
        let p = engines::ae330();
        let half = wastegate_flow(&p, 0.5, 3.0e5, 101_325.0, 900.0);
        let full = wastegate_flow(&p, 1.0, 3.0e5, 101_325.0, 900.0);
        assert!((full / half - 2.0).abs() < 1e-9);
        assert!(wastegate_flow(&p, 0.0, 3.0e5, 101_325.0, 900.0).abs() < 1e-12);
    }

    #[test]
    fn nothing_returns_nan_across_the_whole_pressure_range() {
        let p = engines::ae330();
        for p_em in [1.0, 5e4, 1.0e5, 1.013e5, 2e5, 6e5] {
            for w in [0.0, 500.0, 14_000.0, 19_000.0] {
                let o = operate(&p, w, p_em, 101_325.0, 900.0);
                assert!(
                    o.mass_flow.is_finite() && o.power.is_finite() && o.efficiency.is_finite(),
                    "p_em {p_em} w {w}"
                );
            }
        }
    }
}
