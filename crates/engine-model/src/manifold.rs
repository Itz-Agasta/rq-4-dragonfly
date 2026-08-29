//! Manifold filling dynamics.
//!
//! Each manifold is one ideal-gas control volume at fixed temperature. Isothermal
//! rather than three-state, following the cited work, which showed that adding
//! temperature states changes the pressure and turbocharger dynamics only slightly
//! while costing two states per volume. Temperature states are added elsewhere for
//! the thermal channels an operator actually reads, not for gas-path accuracy.
//!
//! Wahlstrom & Eriksson, Proc IMechE Part D 225(7), 2011, sec. 2.

use crate::EngineParams;

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

/// Temperature of the gas reaching the turbine, K.
///
/// Exhaust gas cools between the port and the turbine inlet, and how much it cools
/// depends on how fast it is moving: a slow-flowing exhaust spends longer in contact
/// with the manifold wall. The exponential is the closed-form solution of a plug flow
/// losing heat to a wall at ambient, which is why a single conductance parameter
/// covers the whole flow range.
///
/// This matters more than it looks. A thermocouple sits in the manifold, not in the
/// port, so without this term every modelled exhaust temperature carries a bias of
/// 50 to 150 K against its measurement, and a twin would have to absorb that
/// somewhere dishonest.
///
/// Eriksson, "Mean value models for exhaust system temperatures", SAE 2002-01-0374.
#[must_use]
pub fn exhaust_gas_temperature(
    p: &EngineParams,
    t_cylinder_out: f64,
    t_amb: f64,
    w_exhaust: f64,
) -> f64 {
    let thermal_flow = w_exhaust * crate::turbine::cp_exhaust(p);
    if thermal_flow <= 0.0 {
        return t_amb;
    }
    t_amb + (t_cylinder_out - t_amb) * (-p.manifolds.h_loss_w_per_k / thermal_flow).exp()
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
    fn exhaust_cools_more_at_low_flow_than_at_high_flow() {
        let p = engines::ae330();
        let fast = exhaust_gas_temperature(&p, 915.0, 288.0, 0.21);
        let slow = exhaust_gas_temperature(&p, 915.0, 288.0, 0.04);
        assert!(slow < fast, "{slow} should be below {fast}");
        assert!(fast < 915.0 && fast > 288.0);
        // At the rating point the drop should be tens of kelvin, not hundreds.
        assert!(
            (915.0 - fast) > 20.0 && (915.0 - fast) < 120.0,
            "drop {}",
            915.0 - fast
        );
    }

    #[test]
    fn no_flow_means_the_manifold_sits_at_ambient() {
        let p = engines::ae330();
        assert!((exhaust_gas_temperature(&p, 915.0, 288.0, 0.0) - 288.0).abs() < 1e-12);
    }
}
