//! Cylinder head metal, the coolant circuit and the radiator.
//!
//! A lumped-capacitance nodal model: one metal node per cylinder head, one node for
//! the coolant, and a radiator rejecting to ram air. That is the smallest set that
//! carries the channels an operator actually watches and the faults that move them.
//! Per-cylinder head nodes in particular are not decoration: an injector delivering
//! less fuel to one cylinder shows up as that cylinder's head running cooler while
//! its exhaust runs hotter, and a lumped head could not tell the two apart.
//!
//! **This engine is liquid cooled.** Heat leaves the combustion gas into head metal,
//! passes into the coolant, and only then reaches the air, at a radiator. That chain
//! behaves differently at altitude from the finned barrels of an air-cooled engine,
//! and the difference is not intuitive. Radiator air mass flow is `m * rho * V` with
//! `m` an air-flow constant, so at a constant indicated airspeed, where true airspeed
//! rises as density falls, flow scales with the **square root** of density rather
//! than with density. At 25,000 ft that is about two thirds of the sea-level flow
//! against a temperature difference nearly two thirds larger, while the heat to be
//! rejected has fallen with the engine's own power. Cooling is therefore not the
//! binding constraint at altitude for this engine; hot, low, slow and at full power
//! is.
//!
//! Chalet, Lesage, Cormerais & Marimbordes, "Nodal modelling for advanced
//! thermal-management of internal combustion engine", Applied Energy 190, 2017.
//! <https://doi.org/10.1016/j.apenergy.2016.12.115>
//!
//! Parsons & Harper, "Radiators for aircraft engines", NBS Technical Paper 211, for
//! the air-flow relation and the range of the air-flow constant.

use crate::EngineParams;

/// Heat flowing from combustion into one cylinder head, W.
///
/// A fixed fraction of that cylinder's fuel energy. The alternative, a gas-side
/// coefficient multiplying a gas-to-wall temperature difference, needs a mean gas
/// temperature and a heat transfer coefficient that no measurement here could
/// separate, and it would buy an inner feedback loop whose only visible effect is to
/// stiffen the integration. What matters downstream is that per-cylinder fuelling
/// moves per-cylinder head temperature, and this form carries that directly.
#[must_use]
pub fn heat_to_head(p: &EngineParams, w_fuel_cylinder: f64) -> f64 {
    p.thermal.heat_fraction_to_head * w_fuel_cylinder * p.fuel.lhv_j_per_kg
}

/// Head-to-coolant conductance at a given engine speed, W/K.
///
/// The coolant pump is engine driven, so coolant flow tracks crank speed, and the
/// jacket side is turbulent, so the coefficient follows flow to the 0.8 power. Over
/// this engine's speed range that is a factor of two, which is too much to lump into
/// a constant.
#[must_use]
pub fn head_conductance(p: &EngineParams, omega_e: f64) -> f64 {
    let reference = p.limits.rpm_max * std::f64::consts::TAU / 60.0;
    p.thermal.head_conductance_w_per_k * (omega_e.max(0.0) / reference).powf(0.8)
}

/// Fraction of a heat exchanger's air flow that its thermostat is admitting, 0 to 1.
///
/// A cold engine has to be allowed to warm up, and at altitude both the radiator and
/// the oil cooler have far more capacity than the engine needs, so without a
/// thermostat the coolant and the oil would settle well below their working
/// temperatures. The bypass leak is what stops the loop from closing completely.
///
/// Shared by the coolant and the oil circuits: same behaviour, different set-points.
#[must_use]
pub fn thermostat(t: f64, open_k: f64, band_k: f64, bypass: f64) -> f64 {
    bypass + (1.0 - bypass) * ((t - open_k) / band_k).clamp(0.0, 1.0)
}

/// Cooling air mass flow through a heat exchanger of the given frontal area, kg/s.
///
/// `mdot = m * rho * V`, with `m` the fraction of the approaching stream that passes
/// through the core rather than spilling around it. Parsons & Harper give 0.3 to 0.7
/// for ordinary cores.
#[must_use]
pub fn ram_air_flow(p: &EngineParams, area_m2: f64, rho: f64, tas_m_s: f64) -> f64 {
    p.cooling.air_flow_constant * rho.max(0.0) * tas_m_s.max(0.0) * area_m2
}

/// Heat rejected by an air-cooled exchanger, W.
#[must_use]
pub fn exchanger_heat(effectiveness: f64, w_air: f64, cp_air: f64, t_hot: f64, t_air: f64) -> f64 {
    effectiveness * w_air * cp_air * (t_hot - t_air)
}

/// Rate of change of one cylinder head metal temperature, K/s.
#[must_use]
pub fn head_temperature_rate(
    p: &EngineParams,
    t_cht: f64,
    t_coolant: f64,
    w_fuel_cylinder: f64,
    omega_e: f64,
) -> f64 {
    let into = heat_to_head(p, w_fuel_cylinder);
    let out = head_conductance(p, omega_e) * (t_cht - t_coolant);
    (into - out) / p.thermal.head_capacity_j_per_k
}

/// Rate of change of coolant temperature, K/s.
#[must_use]
pub fn coolant_temperature_rate(
    p: &EngineParams,
    heat_in: f64,
    t_coolant: f64,
    t_amb: f64,
    rho: f64,
    tas_m_s: f64,
) -> f64 {
    let c = &p.cooling;
    let admitted = thermostat(
        t_coolant,
        c.thermostat_open_k,
        c.thermostat_band_k,
        c.bypass_fraction,
    );
    let w_air = ram_air_flow(p, c.radiator_area_m2, rho, tas_m_s) * admitted;
    let rejected = exchanger_heat(
        p.cooling.radiator_effectiveness,
        w_air,
        p.gas.cp_air,
        t_coolant,
        t_amb,
    );
    (heat_in - rejected) / p.cooling.coolant_capacity_j_per_k
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{atmosphere, engines};

    fn omega(rpm: f64) -> f64 {
        rpm * std::f64::consts::TAU / 60.0
    }

    #[test]
    fn head_conductance_rises_with_speed() {
        let p = engines::ae330();
        assert!(head_conductance(&p, omega(3880.0)) > head_conductance(&p, omega(1500.0)));
        // And by roughly the flow ratio to the 0.8 power, not linearly.
        let ratio = head_conductance(&p, omega(3880.0)) / head_conductance(&p, omega(1940.0));
        assert!((ratio - 2f64.powf(0.8)).abs() < 1e-6, "{ratio}");
    }

    #[test]
    fn a_leaner_cylinder_runs_a_cooler_head() {
        // The other half of an injector fault signature. The exhaust of a lean
        // cylinder gets hotter while its head gets cooler, and a lumped head node
        // could not represent that at all.
        let p = engines::ae330();
        let nominal = heat_to_head(&p, 2.2e-3);
        let starved = heat_to_head(&p, 1.8e-3);
        assert!(starved < nominal);
    }

    #[test]
    fn the_thermostat_shuts_when_cold_and_opens_when_hot() {
        let p = engines::ae330();
        let c = &p.cooling;
        let at = |t| {
            thermostat(
                t,
                c.thermostat_open_k,
                c.thermostat_band_k,
                c.bypass_fraction,
            )
        };
        assert!((at(280.0) - c.bypass_fraction).abs() < 1e-12);
        assert!((at(400.0) - 1.0).abs() < 1e-12);
        assert!(at(280.0) < at(c.thermostat_open_k + 3.0));
    }

    #[test]
    fn radiator_flow_falls_with_the_root_of_density_at_constant_indicated_airspeed() {
        // The claim in the module header, checked rather than asserted in prose.
        // True airspeed rises as 1/sqrt(rho) for a fixed indicated airspeed, so the
        // product rho * V goes as sqrt(rho).
        let p = engines::ae330();
        let sl = atmosphere::isa(0.0);
        let alt = atmosphere::isa(25_000.0 * atmosphere::FT);
        let ias = 78.0 * 0.514_444;
        let tas = |rho: f64| ias * (sl.rho / rho).sqrt();
        let ratio = ram_air_flow(&p, 0.1, alt.rho, tas(alt.rho))
            / ram_air_flow(&p, 0.1, sl.rho, tas(sl.rho));
        assert!((ratio - (alt.rho / sl.rho).sqrt()).abs() < 1e-9, "{ratio}");
        assert!((0.6..0.75).contains(&ratio), "{ratio}");
    }

    #[test]
    fn a_hot_head_sheds_heat_and_a_cold_one_gains_it() {
        let p = engines::ae330();
        assert!(head_temperature_rate(&p, 500.0, 361.0, 0.0, omega(3000.0)) < 0.0);
        assert!(head_temperature_rate(&p, 300.0, 361.0, 2.2e-3, omega(3000.0)) > 0.0);
    }
}
