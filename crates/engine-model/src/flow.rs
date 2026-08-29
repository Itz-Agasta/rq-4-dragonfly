//! Compressible flow through a restriction.
//!
//! The isentropic nozzle relation, used for every gas restriction in the model:
//! the exhaust outlet, the wastegate, the turbine.
//!
//! It is deliberately **not** called `throttle.rs`. The reference engine is a
//! common-rail compression-ignition diesel and has no throttle plate: load is set
//! by injected fuel quantity, and the air path is unthrottled. A file named for a
//! part the engine does not have would be a standing invitation to model the wrong
//! engine.
//!
//! Eriksson & Nielsen, *Modeling and Control of Engines and Drivelines*, Wiley 2014,
//! ch. 5, compressible restriction.

/// Pressure ratio below which a restriction is choked, for a given ratio of
/// specific heats.
#[must_use]
pub fn critical_pressure_ratio(gamma: f64) -> f64 {
    (2.0 / (gamma + 1.0)).powf(gamma / (gamma - 1.0))
}

/// The dimensionless flow function, `Psi(p_down / p_up)`.
///
/// Returns 0 for a non-positive pressure difference rather than a negative or `NaN`
/// flow. Reverse flow through a restriction is not modelled anywhere in this engine
/// and silently returning zero is the correct degenerate behaviour.
///
/// Landmine: at a pressure ratio just under 1 the bracketed difference is the
/// subtraction of two nearly equal powers, and in f64 it goes slightly negative
/// before the analytic value reaches zero. Without the clamp that is the square root
/// of a negative number, and the resulting `NaN` propagates into the manifold state
/// and never leaves. Guard it at the source.
#[must_use]
pub fn psi(pressure_ratio: f64, gamma: f64) -> f64 {
    if pressure_ratio >= 1.0 {
        return 0.0;
    }
    let pr_crit = critical_pressure_ratio(gamma);
    if pressure_ratio <= pr_crit {
        gamma.sqrt() * (2.0 / (gamma + 1.0)).powf((gamma + 1.0) / (2.0 * (gamma - 1.0)))
    } else {
        let a = pressure_ratio.powf(2.0 / gamma);
        let b = pressure_ratio.powf((gamma + 1.0) / gamma);
        (2.0 * gamma / (gamma - 1.0) * (a - b).max(0.0)).sqrt()
    }
}

/// Mass flow through a restriction, kg/s.
///
/// `area` is the effective area, discharge coefficient already folded in, m^2.
#[must_use]
pub fn restriction_flow(
    area: f64,
    p_upstream: f64,
    p_downstream: f64,
    t_upstream: f64,
    r_gas: f64,
    gamma: f64,
) -> f64 {
    if p_upstream <= 0.0 || t_upstream <= 0.0 {
        return 0.0;
    }
    let pr = (p_downstream / p_upstream).clamp(0.0, 1.0);
    area * p_upstream / (r_gas * t_upstream).sqrt() * psi(pr, gamma)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAMMA_E: f64 = 1.2734;

    #[test]
    fn critical_ratio_matches_the_textbook_value_for_air() {
        assert!((critical_pressure_ratio(1.4) - 0.528_282).abs() < 1e-6);
    }

    #[test]
    fn psi_is_continuous_across_the_choking_point() {
        // The two branches are analytically equal at pr_crit. If they ever disagree
        // the integrator sees a step in the derivative and RK4 loses its order.
        for gamma in [1.3, 1.35, 1.4, GAMMA_E] {
            let pr = critical_pressure_ratio(gamma);
            let below = psi(pr - 1e-9, gamma);
            let above = psi(pr + 1e-9, gamma);
            assert!(
                (below - above).abs() < 1e-6,
                "gamma {gamma}: {below} vs {above}"
            );
        }
    }

    #[test]
    fn psi_peaks_at_the_choking_point_and_vanishes_at_unity() {
        let pr_crit = critical_pressure_ratio(1.4);
        let peak = psi(pr_crit, 1.4);
        assert!(psi(0.9, 1.4) < peak);
        assert!((psi(1.0, 1.4)).abs() < 1e-12);
        // Below choking the flow function is flat, which is what makes a choked
        // exhaust outlet insensitive to ambient pressure at altitude.
        assert!((psi(0.1, 1.4) - peak).abs() < 1e-12);
    }

    #[test]
    fn no_nan_anywhere_near_unity() {
        let mut pr = 0.999_999;
        while pr < 1.000_001 {
            assert!(psi(pr, 1.4).is_finite(), "pr {pr}");
            pr += 1e-9;
        }
    }

    #[test]
    fn flow_is_zero_when_the_restriction_is_balanced() {
        assert!(restriction_flow(1e-3, 2e5, 2e5, 900.0, 286.0, GAMMA_E).abs() < 1e-12);
    }

    #[test]
    fn flow_scales_with_upstream_pressure_when_choked() {
        let a = restriction_flow(1e-3, 2e5, 1e4, 900.0, 286.0, GAMMA_E);
        let b = restriction_flow(1e-3, 4e5, 1e4, 900.0, 286.0, GAMMA_E);
        assert!((b / a - 2.0).abs() < 1e-9);
    }
}
