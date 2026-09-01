//! Every figure published about the modelled engine, in one place, with its source.
//!
//! The engine is **locked**: Austro Engine E4P, sales name AE330, the 180 hp
//! variant of the E4 series and the engine TAPAS BH-201 flies from prototype AF-5.
//! `engines/ae330.toml` says why it is modelled rather than the indigenous
//! VRDE/Jayem 180 hp that replaces it.
//!
//! This module exists so that no future change can quietly move a published number.
//! The parameter file holds what the model *uses*; this holds what the manufacturer
//! and the certifying authority *state*, and the tests below hold the two together.
//! Every constant here is `published`. Nothing in this file is fitted or estimated.

/// Sales name of the modelled variant.
pub const NAME: &str = "Austro Engine E4P (AE330)";

/// Type certificate the ratings come from.
/// <https://www.easa.europa.eu/en/downloads/7617/en>
pub const TYPE_CERTIFICATE: &str = "EASA TCDS E.200";

/// Swept volume, m3. Shared with the E4; both variants are the same block.
pub const DISPLACEMENT_M3: f64 = 1.991e-3;

/// Bore and stroke, m, from the Mercedes-Benz OM640 the engine is based on.
pub const BORE_M: f64 = 0.0830;
/// Stroke, m. See [`BORE_M`].
pub const STROKE_M: f64 = 0.0920;

/// Cylinders, in line.
pub const CYLINDERS: usize = 4;

/// Crank revolutions per propeller revolution.
pub const GEARBOX_RATIO: f64 = 1.69;

/// Take-off power, W, at [`TAKEOFF_RPM_CRANK`].
pub const TAKEOFF_POWER_W: f64 = 132_000.0;

/// Crank speed at the take-off rating, rpm.
pub const TAKEOFF_RPM_CRANK: f64 = 3880.0;

/// Propeller speed at the take-off rating, rpm. This is the number the type
/// certificate limits, and the crank speed follows from [`GEARBOX_RATIO`].
pub const TAKEOFF_RPM_PROP: f64 = 2300.0;

/// Maximum continuous power, W, at [`CONTINUOUS_RPM_CRANK`]. Lower than take-off
/// on this variant, unlike the E4, where the two are equal.
pub const CONTINUOUS_POWER_W: f64 = 126_000.0;

/// Crank speed at maximum continuous power, rpm.
pub const CONTINUOUS_RPM_CRANK: f64 = 3720.0;

/// Maximum torque at the propeller shaft, N.m.
pub const MAX_TORQUE_PROP_NM: f64 = 550.0;

/// Maximum engine overspeed, crank rpm.
pub const OVERSPEED_RPM_CRANK: f64 = 4220.0;

/// Fuel consumption at 100% power, litres per hour.
pub const FUEL_LPH_AT_FULL: f64 = 39.0;

/// Fuel consumption at 60% power, litres per hour.
pub const FUEL_LPH_AT_60_PCT: f64 = 21.0;

/// Turbocharger speed containment has been demonstrated to, rpm. Not an operating
/// limit: it is what the rotor is shown to survive once, so the model's supervisor
/// holds well below it.
pub const TURBO_CONTAINMENT_RPM: f64 = 178_000.0;

/// Maximum certified operating altitude, ft.
///
/// The model is deliberately swept above this to 32,000 ft, because that is the
/// MALE mission envelope; `docs/model_validation.md` marks everything above this
/// line as extrapolation. Do not quote the sweep above it as certified performance.
pub const CERTIFIED_CEILING_FT: f64 = 20_000.0;

/// Dry mass, kg. The certificate says 185; the manufacturer factsheet says 186 and
/// is probably quoting the wet mass. The certificate wins.
pub const DRY_MASS_KG: f64 = 185.0;

/// Certified operating limits, TCDS E.200 section IV, **for the E4P**.
///
/// The E4 is more permissive on both temperatures, 140 C oil and 105 C coolant, so
/// quoting those against this model would be the wrong variant by a usefully large
/// margin. The certificate publishes no exhaust or head limit for either model.
pub mod limits {
    /// Maximum oil temperature, K. 139 C.
    pub const OIL_T_MAX_K: f64 = 412.15;
    /// Minimum oil pressure at maximum continuous power, Pa. 2.5 bar.
    pub const OIL_P_MIN_PA: f64 = 2.5e5;
    /// Maximum oil pressure, Pa. 6.5 bar.
    pub const OIL_P_MAX_PA: f64 = 6.5e5;
    /// Maximum coolant temperature, K. 100 C.
    pub const COOLANT_T_MAX_K: f64 = 373.15;
    /// Minimum oil pressure at idle, Pa. 0.9 bar. Not monitored: the twin never
    /// runs the engine at idle, and a limit no run can reach is a limit that only
    /// makes a display look busier.
    pub const OIL_P_MIN_IDLE_PA: f64 = 0.9e5;
}

/// Overall length, width and height, m. Carried for the engine schematic, which
/// has to be drawn to a real aspect ratio rather than an invented one.
pub const ENVELOPE_M: [f64; 3] = [0.738, 0.855, 0.574];

/// The sibling variant, present only so the two are never confused.
///
/// The E4 (sales name AE300) is the same block at a lower rating, and it is the
/// engine most public writing about TAPAS names. Anything here that differs from
/// the constants above is a figure that must not be quoted for this model.
pub mod ae300 {
    /// Take-off and maximum continuous power, W. The E4 rates them equal.
    pub const TAKEOFF_POWER_W: f64 = 123_500.0;
    /// Maximum torque at the propeller shaft, N.m.
    pub const MAX_TORQUE_PROP_NM: f64 = 512.0;
    /// Fuel consumption at 100% power, litres per hour.
    pub const FUEL_LPH_AT_FULL: f64 = 35.1;
    /// Maximum certified operating altitude, ft.
    pub const CERTIFIED_CEILING_FT: f64 = 18_000.0;
    /// Demonstrated turbocharger containment speed, rpm.
    pub const TURBO_CONTAINMENT_RPM: f64 = 172_000.0;
}

/// The two variants must stay distinguishable, checked at compile time.
///
/// A future edit that "corrects" this model to 168 hp has confused the E4 with the
/// E4P, and this refuses to build rather than shipping a plausible wrong number.
const _: () = {
    assert!(TAKEOFF_POWER_W > ae300::TAKEOFF_POWER_W);
    assert!(MAX_TORQUE_PROP_NM > ae300::MAX_TORQUE_PROP_NM);
    assert!(FUEL_LPH_AT_FULL > ae300::FUEL_LPH_AT_FULL);
    assert!(CERTIFIED_CEILING_FT > ae300::CERTIFIED_CEILING_FT);
    assert!(TURBO_CONTAINMENT_RPM > ae300::TURBO_CONTAINMENT_RPM);
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{engines, params::EngineParams};

    fn shipped() -> EngineParams {
        engines::ae330()
    }

    /// The parameter file and the published set describe one engine. They are
    /// separate because one is what the model uses and the other is what the
    /// certificate states, and this is the assertion that they have not drifted.
    #[test]
    fn the_shipped_parameters_carry_the_published_geometry() {
        let p = shipped();
        assert!((p.geometry.displacement_m3 - DISPLACEMENT_M3).abs() < 1e-12);
        assert!((p.geometry.bore_m - BORE_M).abs() < 1e-12);
        assert!((p.geometry.stroke_m - STROKE_M).abs() < 1e-12);
        assert!((p.geometry.gearbox_ratio - GEARBOX_RATIO).abs() < 1e-12);
        assert_eq!(p.geometry.n_cyl as usize, CYLINDERS);
        assert!((p.limits.rpm_max - OVERSPEED_RPM_CRANK).abs() < 1e-9);
        assert!((p.limits.rated_power_w - TAKEOFF_POWER_W).abs() < 1e-9);
    }

    /// The redlines the conventional monitor alarms on are certificated numbers,
    /// and a transcription error in them would understate the twin's lead time
    /// rather than overstate it, which is the direction nobody checks.
    #[test]
    fn the_shipped_redlines_are_the_certificated_ones() {
        let r = shipped().limits.redline;
        assert!((r.oil_t_max_k - limits::OIL_T_MAX_K).abs() < 1e-9);
        assert!((r.oil_p_min_pa - limits::OIL_P_MIN_PA).abs() < 1e-9);
        assert!((r.oil_p_max_pa - limits::OIL_P_MAX_PA).abs() < 1e-9);
        assert!((r.coolant_t_max_k - limits::COOLANT_T_MAX_K).abs() < 1e-9);
    }

    /// Bore and stroke have to reproduce the stated swept volume, or one of the
    /// three is wrong. Four cylinders of pi/4 d^2 L.
    #[test]
    fn bore_and_stroke_reproduce_the_stated_displacement() {
        let swept = CYLINDERS as f64 * std::f64::consts::FRAC_PI_4 * BORE_M * BORE_M * STROKE_M;
        assert!(
            (swept - DISPLACEMENT_M3).abs() / DISPLACEMENT_M3 < 2.0e-3,
            "{swept} m3 against a published {DISPLACEMENT_M3}"
        );
    }

    /// The published torque and power imply a gearbox with no losses, and the
    /// model follows them rather than inventing an efficiency they contradict.
    /// 550 N.m at the propeller through 1.69 is 325.4 N.m at the crank; 132 kW at
    /// 3880 rpm is 324.9 N.m. If these ever disagree by more than a percent, one
    /// of the two published figures has been transcribed wrongly.
    #[test]
    fn the_published_power_and_torque_are_mutually_consistent() {
        let omega = TAKEOFF_RPM_CRANK * std::f64::consts::TAU / 60.0;
        let from_power = TAKEOFF_POWER_W / omega;
        let from_torque = MAX_TORQUE_PROP_NM / GEARBOX_RATIO;
        assert!(
            (from_power - from_torque).abs() / from_torque < 0.01,
            "{from_power} N.m from power against {from_torque} from torque"
        );
    }

    /// The gearbox ratio has to carry the crank speed to the certificated
    /// propeller speed, which is the limit the certificate actually states.
    #[test]
    fn the_gearbox_ratio_lands_on_the_certificated_propeller_speed() {
        let prop = TAKEOFF_RPM_CRANK / GEARBOX_RATIO;
        assert!((prop - TAKEOFF_RPM_PROP).abs() < 5.0, "{prop} rpm");
    }
}
