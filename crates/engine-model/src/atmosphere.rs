//! ISA standard atmosphere: the ambient conditions the engine breathes.
//!
//! Every flow, every efficiency and every cooling term downstream reads pressure,
//! temperature and density from here, so an error in this file is an error
//! everywhere. Turbocharged altitude behaviour in particular is entirely a
//! consequence of how ambient pressure falls with height.
//!
//! ISO 2533:1975 / ICAO Doc 7488, hydrostatic ideal-gas atmosphere.

/// Sea-level standard temperature, K. **published** ISO 2533
pub const T_SL: f64 = 288.15;
/// Sea-level standard pressure, Pa. **published** ISO 2533
pub const P_SL: f64 = 101_325.0;
/// Tropospheric temperature lapse rate, K/m. **published** ISO 2533
pub const LAPSE: f64 = 0.0065;
/// Geopotential altitude of the tropopause, m. **published** ISO 2533
pub const H_TROPOPAUSE: f64 = 11_000.0;
/// Standard gravity, m/s^2. **published** ISO 2533
pub const G0: f64 = 9.806_65;
/// Specific gas constant of dry air, J/(kg K). **published** ISO 2533
pub const R_AIR: f64 = 287.052_87;
/// Metres per foot. **published** international foot
pub const FT: f64 = 0.3048;

/// Ambient state at one altitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ambient {
    /// Static temperature, K.
    pub t: f64,
    /// Static pressure, Pa.
    pub p: f64,
    /// Density, kg/m^3.
    pub rho: f64,
}

/// ISA conditions at a geopotential altitude in metres.
///
/// We do not convert geometric to geopotential altitude: the two differ by 0.15% at
/// the 32,000 ft ceiling, which is far inside the uncertainty of every estimated
/// engine parameter downstream, and pretending otherwise would be false precision.
///
/// Above the tropopause the atmosphere is isothermal, so the power law is replaced
/// by an exponential. The engine ceiling is 9754 m and never reaches it, but without
/// the branch the power law returns a negative temperature above 11,000 m and the
/// twin would emit `NaN` into a chart rather than an obvious error.
#[must_use]
pub fn isa(altitude_m: f64) -> Ambient {
    // g0 / (L R): the barometric exponent, 5.2559. Derived, not typed in, because a
    // literal here silently disagrees with R_AIR if anyone ever edits it.
    let exponent = G0 / (LAPSE * R_AIR);

    let (t, p) = if altitude_m <= H_TROPOPAUSE {
        let t = T_SL - LAPSE * altitude_m;
        (t, P_SL * (t / T_SL).powf(exponent))
    } else {
        let t_trop = T_SL - LAPSE * H_TROPOPAUSE;
        let p_trop = P_SL * (t_trop / T_SL).powf(exponent);
        let dh = altitude_m - H_TROPOPAUSE;
        (t_trop, p_trop * (-G0 * dh / (R_AIR * t_trop)).exp())
    };

    Ambient {
        t,
        p,
        rho: p / (R_AIR * t),
    }
}

/// ISA temperature deviation at an altitude, K. Positive is hotter than standard.
///
/// Hot and cold day cases are conventionally expressed as an ISA offset. Derive the
/// deviation from altitude and measured OAT rather than storing it, so the two can
/// never disagree.
#[must_use]
pub fn isa_deviation(altitude_m: f64, oat_k: f64) -> f64 {
    oat_k - isa(altitude_m).t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, rel: f64) {
        assert!((a - b).abs() / b.abs() < rel, "{a} vs {b}");
    }

    #[test]
    fn sea_level_matches_the_defining_values() {
        let a = isa(0.0);
        close(a.t, 288.15, 1e-12);
        close(a.p, 101_325.0, 1e-12);
        close(a.rho, 1.225, 1e-4);
    }

    #[test]
    fn tropopause_matches_the_published_table() {
        // ISO 2533 tabulates 216.65 K and 22632.06 Pa at 11 km.
        let a = isa(11_000.0);
        close(a.t, 216.65, 1e-9);
        close(a.p, 22_632.06, 1e-4);
    }

    #[test]
    fn eleven_thousand_feet_is_the_critical_altitude_point() {
        // The rated critical altitude of the reference engine. Hand-computed:
        // 3352.8 m, 266.36 K, 67.02 kPa.
        let a = isa(11_000.0 * FT);
        close(a.t, 266.357, 1e-5);
        close(a.p, 67_019.0, 1e-4);
    }

    #[test]
    fn typical_male_cruise_altitude() {
        // 22,400 ft with an OAT of -31 C.
        let a = isa(22_400.0 * FT);
        close(a.p, 42_067.7, 1e-4);
        // Which is very close to a standard day, not a hot one.
        let dev = isa_deviation(22_400.0 * FT, 273.15 - 31.0);
        assert!((dev - (-1.6)).abs() < 0.1, "ISA deviation {dev}");
    }

    #[test]
    fn stratosphere_branch_is_continuous_at_the_tropopause() {
        let below = isa(H_TROPOPAUSE - 1e-6);
        let above = isa(H_TROPOPAUSE + 1e-6);
        close(below.p, above.p, 1e-9);
        close(below.t, above.t, 1e-9);
    }

    #[test]
    fn ceiling_stays_physical() {
        // 32,000 ft is the rated ceiling of the reference engine; nothing here may
        // go negative or NaN.
        let a = isa(32_000.0 * FT);
        assert!(a.t > 0.0 && a.p > 0.0 && a.rho > 0.0);
        assert!(a.p < isa(0.0).p);
    }
}
