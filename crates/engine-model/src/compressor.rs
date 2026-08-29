//! Centrifugal compressor and the intercooler behind it.
//!
//! Mass flow comes from the **ellipse model**: at a given shaft speed the compressor
//! traces a quarter ellipse between choke flow at unit pressure ratio and zero flow
//! at the maximum head it can raise. Parametric rather than an interpolated map,
//! deliberately, and for two reasons. No manufacturer publishes a map for this
//! engine's turbocharger, so there is nothing to interpolate; and a map extrapolates
//! unpredictably off its measured region, which is exactly where an engine at
//! 32,000 ft operates.
//!
//! Everything is expressed in corrected quantities, referred to a standard inlet
//! condition. That is what makes one set of coefficients valid at every altitude:
//! the compressor does not know its ambient pressure, only its corrected speed and
//! pressure ratio.
//!
//! Leufven & Eriksson, "A surge and choke capable compressor flow model for control
//! purposes", Control Engineering Practice 21(12), 2013, for the ellipse.
//! Sivertsson & Eriksson, SIMS 55, 2014, for the efficiency form and the parameter
//! shapes. <https://doi.org/10.1016/j.conengprac.2013.08.006>

use crate::EngineParams;

/// Compressor operating point.
#[derive(Clone, Copy, Debug)]
pub struct Operation {
    /// Pressure ratio across the compressor.
    pub pressure_ratio: f64,
    /// Mass flow, kg/s.
    pub mass_flow: f64,
    /// Isentropic efficiency.
    pub efficiency: f64,
    /// Shaft power absorbed, W.
    pub power: f64,
    /// Outlet temperature before the intercooler, K.
    pub outlet_temperature: f64,
    /// Margin to the surge line, in pressure-ratio units. Negative means surging.
    pub surge_margin: f64,
}

/// Corrected shaft speed, rad/s, referred to the standard inlet temperature.
#[must_use]
pub fn corrected_speed(p: &EngineParams, omega_tc: f64, t_in: f64) -> f64 {
    omega_tc / (t_in / p.compressor.t_ref_k).sqrt()
}

/// Highest pressure ratio the wheel can raise at a given corrected speed.
///
/// Euler turbomachinery work: the head is the square of the wheel tip speed times a
/// dimensionless head coefficient, and the pressure ratio follows isentropically.
/// This is the top of the ellipse, reached only at zero flow.
#[must_use]
pub fn max_pressure_ratio(p: &EngineParams, omega_corrected: f64, t_in: f64) -> f64 {
    let tip = omega_corrected * p.compressor.r_wheel_m;
    let gamma = p.gas.gamma_air;
    (tip * tip * p.compressor.psi_max / (2.0 * p.gas.cp_air * t_in) + 1.0)
        .powf(gamma / (gamma - 1.0))
}

/// Solve the compressor at one operating point.
///
/// Returns zero flow rather than a complex number when the manifold is asking for
/// more head than the wheel can raise. That happens routinely on a cold start and
/// during a fast altitude change, and the physical behaviour is that flow stops and
/// the manifold empties through the engine, which is what zero produces.
#[must_use]
pub fn operate(p: &EngineParams, omega_tc: f64, p_in: f64, t_in: f64, p_out: f64) -> Operation {
    let c = &p.compressor;
    let gamma = p.gas.gamma_air;
    let kappa = (gamma - 1.0) / gamma;

    let pressure_ratio = (p_out / p_in).max(1.0);
    let omega_corrected = corrected_speed(p, omega_tc, t_in);
    let omega_norm = omega_corrected / c.omega_ref;
    let pi_max = max_pressure_ratio(p, omega_corrected, t_in);

    let m = c.m_corr_max;
    let m_corr_max = (m[0] * omega_norm * omega_norm + m[1] * omega_norm + m[2]).max(0.0);
    // The ellipse. Clamping the ratio at 1 is what makes the over-head case return
    // zero flow instead of the square root of a negative number.
    let ratio = (pressure_ratio / pi_max).min(1.0);
    let m_corr = m_corr_max * (1.0 - ratio * ratio).max(0.0).sqrt();
    let mass_flow = m_corr * (p_in / c.p_ref_pa) / (t_in / c.t_ref_k).sqrt();

    // Efficiency as a quadratic form in the deviation of flow coefficient and
    // corrected speed from their optima. Two variables because efficiency falls off
    // in both directions on a compressor map, and the cross term is what tilts the
    // island along the speed lines rather than leaving it axis-aligned.
    let denom = omega_tc * 8.0 * c.r_wheel_m.powi(3) * p_in;
    let phi = if denom > 0.0 {
        mass_flow * p.gas.r_air * t_in / denom
    } else {
        c.phi_opt
    };
    let d_phi = phi - c.phi_opt;
    let d_n = omega_norm - c.omega_norm_opt;
    let q = c.q_form;
    let penalty = q[0] * d_phi * d_phi + 2.0 * q[2] * d_phi * d_n + q[1] * d_n * d_n;
    // Floored well above zero: efficiency divides the power, and a compressor that
    // has wandered far off its island must still absorb a large, finite torque.
    let efficiency = (c.eta_max - penalty).clamp(0.15, 0.95);

    let rise = pressure_ratio.powf(kappa) - 1.0;
    Operation {
        pressure_ratio,
        mass_flow,
        efficiency,
        power: mass_flow * p.gas.cp_air * t_in * rise / efficiency,
        outlet_temperature: t_in * (1.0 + rise / efficiency),
        surge_margin: c.surge[0] * m_corr + c.surge[1] - pressure_ratio,
    }
}

/// Intake manifold temperature after the intercooler, K.
///
/// Effectiveness form. The intercooler rejects to ram air at ambient temperature, so
/// charge temperature falls with altitude even as pressure ratio and therefore
/// compressor outlet temperature rise. Those two effects nearly cancel on this
/// engine, which is why charge temperature is roughly flat across the envelope and
/// why a constant was a tolerable stand-in before this existed.
#[must_use]
pub fn intercooler_outlet(p: &EngineParams, t_compressor_out: f64, t_amb: f64) -> f64 {
    t_compressor_out - p.compressor.intercooler_effectiveness * (t_compressor_out - t_amb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines;

    #[test]
    fn flow_falls_to_zero_at_the_top_of_the_ellipse() {
        let p = engines::ae330();
        let omega = 14_000.0;
        let pi_max = max_pressure_ratio(&p, omega, 288.15);
        let at_top = operate(&p, omega, 101_325.0, 288.15, 101_325.0 * pi_max);
        assert!(at_top.mass_flow.abs() < 1e-9, "{}", at_top.mass_flow);
        let below = operate(&p, omega, 101_325.0, 288.15, 101_325.0 * pi_max * 0.8);
        assert!(below.mass_flow > 0.0);
    }

    #[test]
    fn asking_for_more_head_than_the_wheel_can_raise_gives_zero_not_nan() {
        let p = engines::ae330();
        let o = operate(&p, 2000.0, 101_325.0, 288.15, 8.0e5);
        assert!(o.mass_flow.is_finite() && o.power.is_finite());
        assert!(o.mass_flow.abs() < 1e-12);
    }

    #[test]
    fn head_rises_with_the_square_of_tip_speed() {
        // Doubling corrected speed quadruples the Euler head, so (Pi_max^k - 1)
        // must quadruple. This is the check that catches a dropped square.
        let p = engines::ae330();
        let k = (p.gas.gamma_air - 1.0) / p.gas.gamma_air;
        let head = |w: f64| max_pressure_ratio(&p, w, 288.15).powf(k) - 1.0;
        assert!((head(14_000.0) / head(7_000.0) - 4.0).abs() < 1e-6);
    }

    #[test]
    fn reaches_the_boost_set_point_below_the_containment_speed() {
        // The whole architecture depends on this: the wheel must raise the design
        // manifold pressure at sea level while staying under the turbocharger speed
        // the type certificate demonstrates containment for.
        let p = engines::ae330();
        let mut omega = 4_000.0;
        while omega < p.turbocharger.omega_max {
            let o = operate(&p, omega, 101_325.0, 288.15, p.control.map_setpoint_pa);
            if o.mass_flow > 0.20 {
                return;
            }
            omega += 100.0;
        }
        panic!(
            "cannot reach the boost set-point below {} rad/s",
            p.turbocharger.omega_max
        );
    }

    #[test]
    fn intercooler_tracks_ambient() {
        let p = engines::ae330();
        let warm = intercooler_outlet(&p, 430.0, 288.15);
        let cold = intercooler_outlet(&p, 430.0, 240.0);
        assert!(cold < warm);
        assert!(warm > 288.15 && warm < 430.0);
    }
}
