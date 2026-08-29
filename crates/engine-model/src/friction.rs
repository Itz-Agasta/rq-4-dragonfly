//! Rubbing, accessory and pumping losses.
//!
//! Both losses are expressed as mean effective pressures and converted to torque by
//! the same relation, which is the convention in the engine literature and makes
//! them directly comparable to the indicated work they are subtracted from.
//!
//! Heywood, *Internal Combustion Engine Fundamentals*, McGraw-Hill 1988, ch. 13.

use crate::EngineParams;
use std::f64::consts::PI;

/// Torque corresponding to a mean effective pressure, N.m.
#[must_use]
pub fn torque_from_mep(p: &EngineParams, mep: f64) -> f64 {
    p.geometry.displacement_m3 * mep / (2.0 * PI * p.geometry.revs_per_cycle)
}

/// Friction mean effective pressure, Pa.
///
/// A quadratic in crank speed, covering rubbing friction and the engine-driven
/// accessories. Speed enters in thousands of rpm rather than rad/s so the
/// coefficients stay near unity and remain readable when hand-tuned.
///
/// The reduction gearbox is deliberately **not** in here. The published power and
/// propeller torque figures for this engine class are consistent only with a
/// lossless gearbox, so adding a gearbox efficiency would put the model at odds
/// with the data it is validated against.
#[must_use]
pub fn fmep(p: &EngineParams, omega_e: f64) -> f64 {
    let n_krpm = omega_e.max(0.0) * 60.0 / (2.0 * PI * 1000.0);
    let c = p.friction.c_fr;
    1e5 * (c[0] * n_krpm * n_krpm + c[1] * n_krpm + c[2])
}

/// Friction torque, N.m.
#[must_use]
pub fn friction_torque(p: &EngineParams, omega_e: f64) -> f64 {
    torque_from_mep(p, fmep(p, omega_e))
}

/// Pumping mean effective pressure, Pa.
///
/// The physical definition, exhaust manifold pressure minus intake. It goes
/// negative when boost exceeds back pressure, which is a genuine positive
/// contribution to output and must not be clamped away.
#[must_use]
pub fn pmep(p_im: f64, p_em: f64) -> f64 {
    p_em - p_im
}

/// Pumping torque, N.m. Negative when the engine is being supercharged through the
/// cycle rather than pumping against back pressure.
#[must_use]
pub fn pumping_torque(p: &EngineParams, p_im: f64, p_em: f64) -> f64 {
    torque_from_mep(p, pmep(p_im, p_em))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines;

    fn omega(rpm: f64) -> f64 {
        rpm * 2.0 * PI / 60.0
    }

    #[test]
    fn mep_to_torque_matches_the_four_stroke_relation() {
        // 4 pi M = V_d MEP for a four-stroke.
        let p = engines::ae330();
        let m = torque_from_mep(&p, 20.0e5);
        assert!((m - 1.991e-3 * 20.0e5 / (4.0 * PI)).abs() < 1e-9);
    }

    #[test]
    fn friction_hits_its_fitted_anchor_points() {
        let p = engines::ae330();
        assert!((fmep(&p, omega(1000.0)) / 1e5 - 0.75).abs() < 0.01);
        assert!((fmep(&p, omega(3880.0)) / 1e5 - 2.20).abs() < 0.01);
    }

    #[test]
    fn friction_rises_monotonically_with_speed() {
        let p = engines::ae330();
        let mut previous = 0.0;
        for rpm in (0..=4200).step_by(200) {
            let f = fmep(&p, omega(f64::from(rpm)));
            assert!(f > previous, "not monotone at {rpm} rpm");
            previous = f;
        }
    }

    #[test]
    fn boost_above_back_pressure_gives_a_negative_pumping_torque() {
        let p = engines::ae330();
        assert!(pumping_torque(&p, 3.1e5, 2.9e5) < 0.0);
        assert!(pumping_torque(&p, 3.1e5, 3.45e5) > 0.0);
    }
}
