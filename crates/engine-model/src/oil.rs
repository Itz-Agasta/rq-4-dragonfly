//! Lubrication circuit: oil temperature, viscosity and gallery pressure.
//!
//! Gallery pressure is algebraic rather than a state. The circuit balances in
//! milliseconds against an engine whose slowest thermal mode is minutes, so carrying
//! it as a state would add stiffness the integrator has to absorb for a quantity that
//! is a function of pump speed, viscosity and clearance anyway.
//!
//! There is no control-oriented literature for this. A search of the engine and
//! tribology literature returns surface science and elastohydrodynamics, not circuit
//! models, so this is built from first principles: a positive-displacement pump
//! delivering flow proportional to crank speed, that flow leaving through bearing and
//! journal clearances whose conductance is inversely proportional to viscosity, and a
//! relief valve capping the result.
//!
//! That structure is what makes the lubrication fault modes fall out rather than
//! being scripted. Worn clearances raise the leakage conductance and pressure falls
//! at unchanged speed and temperature. A hot-running engine thins the oil and
//! pressure falls for a reason that is not wear at all. Distinguishing those two is
//! most of what oil-pressure diagnosis is.

use crate::EngineParams;

/// Dynamic viscosity, Pa s, by the Vogel equation.
///
/// `mu = a exp(b / (T - c))`. Three parameters, and it holds across the whole
/// working range where a two-parameter Arrhenius form does not. Fitted here to a
/// multigrade aviation oil.
#[must_use]
pub fn viscosity(p: &EngineParams, t_oil: f64) -> f64 {
    let v = p.oil.vogel;
    // Two guards, and both matter. Below the Vogel pole the exponent changes sign and
    // the fit is meaningless; well above the pole but still very cold the exponent
    // overflows f64 long before it reaches anything physical. Capping the result at a
    // viscosity thicker than any oil ever is keeps a bad input visible as an
    // implausible number rather than as an infinity that poisons everything it
    // touches. The fit itself is only meant for the working range, roughly 270 K up.
    const MAX_VISCOSITY: f64 = 10.0;
    let denominator = (t_oil - v[2]).max(1.0);
    (v[0] * (v[1] / denominator).exp()).min(MAX_VISCOSITY)
}

/// Oil gallery pressure above ambient, Pa.
///
/// The coefficient is the pump displacement divided by the leakage conductance. Only
/// that group is identifiable without stripping the engine, so it is carried as one
/// number rather than as two that could not be told apart.
#[must_use]
pub fn gallery_pressure(p: &EngineParams, omega_e: f64, t_oil: f64) -> f64 {
    let raw = p.oil.pressure_coefficient * omega_e.max(0.0) * viscosity(p, t_oil);
    raw.min(p.oil.relief_pressure_pa)
}

/// Heat entering the oil, W.
///
/// Two sources: all of the friction work, which ends up in the oil film by
/// definition, and the piston-cooling jets carrying combustion heat out of the piston
/// crowns.
#[must_use]
pub fn heat_into_oil(p: &EngineParams, friction_power: f64, w_fuel: f64) -> f64 {
    friction_power.max(0.0) + p.oil.heat_fraction_from_fuel * w_fuel * p.fuel.lhv_j_per_kg
}

/// Rate of change of oil temperature, K/s.
///
/// The oil rejects to ram air through its own cooler rather than to the coolant. That
/// is the aviation arrangement, and it is also what allows oil to run slightly cooler
/// than coolant, which a coolant-coupled exchanger could never produce.
#[must_use]
pub fn temperature_rate(
    p: &EngineParams,
    t_oil: f64,
    t_amb: f64,
    heat_in: f64,
    rho: f64,
    tas_m_s: f64,
) -> f64 {
    let admitted = crate::thermal::thermostat(
        t_oil,
        p.oil.thermostat_open_k,
        p.oil.thermostat_band_k,
        p.oil.bypass_fraction,
    );
    let w_air = crate::thermal::ram_air_flow(p, p.oil.cooler_area_m2, rho, tas_m_s) * admitted;
    let rejected = crate::thermal::exchanger_heat(
        p.oil.cooler_effectiveness,
        w_air,
        p.gas.cp_air,
        t_oil,
        t_amb,
    );
    (heat_in - rejected) / p.oil.capacity_j_per_k
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines;

    fn omega(rpm: f64) -> f64 {
        rpm * std::f64::consts::TAU / 60.0
    }

    #[test]
    fn viscosity_matches_the_grade_it_was_fitted_to() {
        // A 15W-50 aviation oil is about 0.096 Pa s at 40 C and 0.016 at 100 C.
        let p = engines::ae330();
        assert!(
            (viscosity(&p, 313.15) - 0.096).abs() < 0.010,
            "{}",
            viscosity(&p, 313.15)
        );
        assert!(
            (viscosity(&p, 373.15) - 0.0157).abs() < 0.003,
            "{}",
            viscosity(&p, 373.15)
        );
    }

    #[test]
    fn viscosity_stays_finite_below_the_vogel_pole() {
        let p = engines::ae330();
        for t in [1.0, 100.0, p.oil.vogel[2], p.oil.vogel[2] + 0.5, 250.0] {
            assert!(viscosity(&p, t).is_finite(), "at {t} K");
        }
    }

    #[test]
    fn pressure_at_the_canonical_cruise_point() {
        // 4.2 bar at 3720 rpm with the oil at 358 K.
        let p = engines::ae330();
        let bar = gallery_pressure(&p, omega(3720.0), 358.0) / 1e5;
        assert!((bar - 4.2).abs() < 0.15, "{bar} bar");
    }

    #[test]
    fn pressure_falls_with_speed_and_with_temperature() {
        // The two confounded causes that oil-pressure diagnosis exists to separate.
        let p = engines::ae330();
        let reference = gallery_pressure(&p, omega(3720.0), 358.0);
        assert!(gallery_pressure(&p, omega(1200.0), 358.0) < reference);
        assert!(gallery_pressure(&p, omega(3720.0), 390.0) < reference);
    }

    #[test]
    fn cold_thick_oil_is_held_by_the_relief_valve() {
        let p = engines::ae330();
        let cold = gallery_pressure(&p, omega(1500.0), 280.0);
        assert!((cold - p.oil.relief_pressure_pa).abs() < 1e-9, "{cold} Pa");
    }

    #[test]
    fn hot_oil_sheds_heat_and_cold_oil_gains_it() {
        let p = engines::ae330();
        assert!(temperature_rate(&p, 400.0, 288.0, 0.0, 1.225, 50.0) < 0.0);
        assert!(temperature_rate(&p, 300.0, 288.0, 20_000.0, 1.225, 50.0) > 0.0);
    }
}
