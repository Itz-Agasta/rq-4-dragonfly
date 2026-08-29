//! Manifold filling dynamics.
//!
//! Each manifold is one ideal-gas control volume at fixed temperature. Isothermal
//! rather than three-state, following the cited work, which showed that adding
//! temperature states changes the pressure and turbocharger dynamics only slightly
//! while costing two states per volume. Temperature states are added elsewhere for
//! the thermal channels an operator actually reads, not for gas-path accuracy.
//!
//! Wahlstrom & Eriksson, Proc IMechE Part D 225(7), 2011, sec. 2.

use crate::{EngineParams, flow};

/// Rate of change of intake manifold pressure, Pa/s.
#[must_use]
pub fn intake_pressure_rate(p: &EngineParams, t_im: f64, w_in: f64, w_cyl: f64) -> f64 {
    p.gas.r_air * t_im / p.manifolds.v_im_m3 * (w_in - w_cyl)
}

/// Rate of change of exhaust manifold pressure, Pa/s.
#[must_use]
pub fn exhaust_pressure_rate(p: &EngineParams, t_em: f64, w_in: f64, w_out: f64) -> f64 {
    p.gas.r_exh * t_em / p.manifolds.v_em_m3 * (w_in - w_out)
}

/// Mass flow out of the exhaust manifold, kg/s.
#[must_use]
pub fn exhaust_outflow(p: &EngineParams, p_em: f64, p_amb: f64, t_em: f64) -> f64 {
    flow::restriction_flow(
        p.manifolds.exhaust_area_m2,
        p_em,
        p_amb,
        t_em,
        p.gas.r_exh,
        p.gas.gamma_exh,
    )
}

/// Exhaust manifold pressure at which inflow and outflow balance, Pa.
///
/// The outlet temperature depends on the pressure ratio across the cylinders and the
/// pressure depends on the temperature, so this is a fixed point rather than a
/// closed form. The coupling is weak, through an exponent of about 0.28, and the
/// iteration converges in a handful of passes. `t_em` is a callback so the caller
/// keeps ownership of the combustion model.
///
/// Returns `None` if the iteration has not converged, which means the operating
/// point is outside the range where the restriction model is invertible; the caller
/// must decide what to do rather than receive a plausible-looking wrong number.
pub fn steady_exhaust_pressure<F>(
    p: &EngineParams,
    w_in: f64,
    p_amb: f64,
    mut t_em: F,
) -> Option<f64>
where
    F: FnMut(f64) -> f64,
{
    // Seed above ambient, not at it: at equal pressures the flow function is zero,
    // the first iteration divides by it, and the solve fails on its own start point.
    let mut p_em = 2.0 * p_amb.max(1.0);
    for _ in 0..200 {
        let t = t_em(p_em);
        // Invert the restriction for the pressure that passes w_in at this
        // temperature, then relax towards it. Under-relaxation keeps the loop stable
        // where the restriction is on the subsonic branch and the gain is high.
        let unit_flow = exhaust_outflow(p, p_em, p_amb, t) / p_em;
        if unit_flow <= 0.0 {
            return None;
        }
        let target = w_in / unit_flow;
        let next = p_em + 0.5 * (target - p_em);
        if (next - p_em).abs() < 1e-6 * next {
            return Some(next);
        }
        p_em = next.max(p_amb);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines;

    #[test]
    fn intake_fills_when_supply_exceeds_demand_and_empties_otherwise() {
        let p = engines::ae330();
        assert!(intake_pressure_rate(&p, 320.0, 0.25, 0.20) > 0.0);
        assert!(intake_pressure_rate(&p, 320.0, 0.15, 0.20) < 0.0);
        assert!(intake_pressure_rate(&p, 320.0, 0.20, 0.20).abs() < 1e-12);
    }

    #[test]
    fn intake_filling_time_constant_is_tens_of_milliseconds() {
        // A 3 litre volume at 320 K filled at 0.2 kg/s reaches 3.1 bar in about
        // 20 ms. Much slower than this and throttle response is wrong; much faster
        // and the fixed-step integrator has to shrink.
        let p = engines::ae330();
        let rate = intake_pressure_rate(&p, 320.0, 0.20, 0.0);
        let seconds = 3.10e5 / rate;
        assert!((0.005..0.10).contains(&seconds), "{seconds} s");
    }

    #[test]
    fn steady_exhaust_pressure_balances_the_restriction() {
        let p = engines::ae330();
        let w_in = 0.209;
        let p_em = steady_exhaust_pressure(&p, w_in, 101_325.0, |_| 917.0).unwrap();
        let out = exhaust_outflow(&p, p_em, 101_325.0, 917.0);
        assert!((out - w_in).abs() < 1e-6, "{out} vs {w_in}");
        // And it should sit modestly above the intake, as a wastegated turbo does.
        assert!((3.2e5..3.8e5).contains(&p_em), "p_em {p_em}");
    }

    #[test]
    fn steady_exhaust_pressure_reports_failure_rather_than_guessing() {
        let p = engines::ae330();
        assert!(steady_exhaust_pressure(&p, 0.209, 101_325.0, |_| -1.0).is_none());
    }
}
