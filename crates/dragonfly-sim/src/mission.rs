//! Mission profiles: what the airframe is doing to the engine over time.
//!
//! A profile is a table of key points that the sim interpolates between, not a
//! script of events. That keeps a thirty-hour endurance leg and a ten-second
//! throttle slam in the same representation, and it means the twin can be handed
//! the same table to project forward.
//!
//! True airspeed rather than indicated is what reaches the engine model, because
//! it is mass flow that cools a radiator. At constant indicated airspeed, true
//! airspeed rises as the inverse square root of density, so the conversion is
//! done here where both are known.

use engine_model::atmosphere;

/// One point in a profile. Values between points are linearly interpolated.
#[derive(Clone, Copy, Debug)]
struct KeyPoint {
    t_s: f64,
    altitude_ft: f64,
    /// Deviation of outside air temperature from the standard atmosphere, K.
    isa_deviation_k: f64,
    indicated_airspeed_kt: f64,
    /// Fuelling demand, 0 to 1.
    fuel_cmd: f64,
    /// Speed the propeller governor holds, crankshaft rpm.
    rpm_cmd: f64,
}

/// The environment and commands at one instant.
#[derive(Clone, Copy, Debug)]
pub struct Condition {
    /// Pressure altitude, m.
    pub altitude_m: f64,
    /// Outside air temperature, K.
    pub oat_k: f64,
    /// Ambient static pressure, Pa.
    pub p_amb: f64,
    /// Ambient density, kg/m3.
    pub rho: f64,
    /// Indicated airspeed, m/s.
    pub ias_m_s: f64,
    /// True airspeed, m/s.
    pub tas_m_s: f64,
    /// Fuelling demand, 0 to 1.
    pub fuel_cmd: f64,
    /// Commanded crankshaft speed, rpm.
    pub rpm_cmd: f64,
}

/// The mission profiles the problem statement names, plus the canonical cruise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Profile {
    /// Steady high-altitude cruise. The operating point every mock is drawn at.
    Cruise,
    /// Sea level to above the critical altitude and back. Shows the boost knee.
    HighAltitude,
    /// A long low-power leg, the thirty-hour surveillance case.
    Endurance,
    /// Sea level at ISA+30 under full power. The binding thermal case.
    HotWeather,
    /// Repeated fuelling steps. Shows turbocharger lag and manifold filling.
    Transients,
}

const FT: f64 = 0.3048;
const KT: f64 = 0.514_444;

impl Profile {
    fn key_points(self) -> &'static [KeyPoint] {
        match self {
            // Held indefinitely: the last point repeats once the table runs out.
            Self::Cruise => &[KeyPoint {
                t_s: 0.0,
                altitude_ft: 22_400.0,
                isa_deviation_k: -1.6,
                indicated_airspeed_kt: 78.0,
                fuel_cmd: 0.35,
                rpm_cmd: 3720.0,
            }],
            Self::HighAltitude => &[
                KeyPoint {
                    t_s: 0.0,
                    altitude_ft: 2000.0,
                    isa_deviation_k: 0.0,
                    indicated_airspeed_kt: 90.0,
                    fuel_cmd: 1.0,
                    rpm_cmd: 3880.0,
                },
                KeyPoint {
                    t_s: 600.0,
                    altitude_ft: 30_000.0,
                    isa_deviation_k: 0.0,
                    indicated_airspeed_kt: 78.0,
                    fuel_cmd: 1.0,
                    rpm_cmd: 3880.0,
                },
                KeyPoint {
                    t_s: 1200.0,
                    altitude_ft: 2000.0,
                    isa_deviation_k: 0.0,
                    indicated_airspeed_kt: 90.0,
                    fuel_cmd: 1.0,
                    rpm_cmd: 3880.0,
                },
            ],
            Self::Endurance => &[
                KeyPoint {
                    t_s: 0.0,
                    altitude_ft: 18_000.0,
                    isa_deviation_k: 0.0,
                    indicated_airspeed_kt: 70.0,
                    fuel_cmd: 0.28,
                    rpm_cmd: 3200.0,
                },
                KeyPoint {
                    t_s: 108_000.0,
                    altitude_ft: 24_000.0,
                    isa_deviation_k: 0.0,
                    indicated_airspeed_kt: 70.0,
                    fuel_cmd: 0.24,
                    rpm_cmd: 3200.0,
                },
            ],
            Self::HotWeather => &[
                KeyPoint {
                    t_s: 0.0,
                    altitude_ft: 0.0,
                    isa_deviation_k: 30.0,
                    indicated_airspeed_kt: 75.0,
                    fuel_cmd: 1.0,
                    rpm_cmd: 3880.0,
                },
                KeyPoint {
                    t_s: 900.0,
                    altitude_ft: 6000.0,
                    isa_deviation_k: 30.0,
                    indicated_airspeed_kt: 75.0,
                    fuel_cmd: 1.0,
                    rpm_cmd: 3880.0,
                },
            ],
            Self::Transients => &[
                KeyPoint {
                    t_s: 0.0,
                    altitude_ft: 10_000.0,
                    isa_deviation_k: 0.0,
                    indicated_airspeed_kt: 85.0,
                    fuel_cmd: 0.2,
                    rpm_cmd: 3600.0,
                },
                // A step, not a ramp: two key points 0.1 s apart, which is faster
                // than the manifold and far faster than the turbocharger shaft.
                KeyPoint {
                    t_s: 20.0,
                    altitude_ft: 10_000.0,
                    isa_deviation_k: 0.0,
                    indicated_airspeed_kt: 85.0,
                    fuel_cmd: 0.2,
                    rpm_cmd: 3600.0,
                },
                KeyPoint {
                    t_s: 20.1,
                    altitude_ft: 10_000.0,
                    isa_deviation_k: 0.0,
                    indicated_airspeed_kt: 85.0,
                    fuel_cmd: 1.0,
                    rpm_cmd: 3600.0,
                },
                KeyPoint {
                    t_s: 40.0,
                    altitude_ft: 10_000.0,
                    isa_deviation_k: 0.0,
                    indicated_airspeed_kt: 85.0,
                    fuel_cmd: 1.0,
                    rpm_cmd: 3600.0,
                },
                KeyPoint {
                    t_s: 40.1,
                    altitude_ft: 10_000.0,
                    isa_deviation_k: 0.0,
                    indicated_airspeed_kt: 85.0,
                    fuel_cmd: 0.2,
                    rpm_cmd: 3600.0,
                },
            ],
        }
    }

    /// The condition at mission time `t_s`.
    ///
    /// Past the end of the table the last point is held, so a profile can be run
    /// for longer than it was written for without falling off a cliff.
    #[must_use]
    pub fn condition_at(self, t_s: f64) -> Condition {
        let points = self.key_points();
        let key = interpolate(points, t_s);

        let altitude_m = key.altitude_ft * FT;
        let isa = atmosphere::isa(altitude_m);
        let oat_k = isa.t + key.isa_deviation_k;
        // Pressure is a function of altitude alone; a hot day changes density
        // through temperature, not through pressure.
        let rho = isa.p / (atmosphere::R_AIR * oat_k);
        let ias_m_s = key.indicated_airspeed_kt * KT;
        let sea_level_rho = atmosphere::P_SL / (atmosphere::R_AIR * atmosphere::T_SL);

        Condition {
            altitude_m,
            oat_k,
            p_amb: isa.p,
            rho,
            ias_m_s,
            tas_m_s: ias_m_s * (sea_level_rho / rho).sqrt(),
            fuel_cmd: key.fuel_cmd,
            rpm_cmd: key.rpm_cmd,
        }
    }
}

/// Every table is a non-empty literal in this file, so indexing is total.
fn interpolate(points: &[KeyPoint], t_s: f64) -> KeyPoint {
    let first = points[0];
    if t_s <= first.t_s {
        return first;
    }
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if t_s < b.t_s {
            let span = (b.t_s - a.t_s).max(f64::EPSILON);
            let k = (t_s - a.t_s) / span;
            return KeyPoint {
                t_s,
                altitude_ft: lerp(a.altitude_ft, b.altitude_ft, k),
                isa_deviation_k: lerp(a.isa_deviation_k, b.isa_deviation_k, k),
                indicated_airspeed_kt: lerp(a.indicated_airspeed_kt, b.indicated_airspeed_kt, k),
                fuel_cmd: lerp(a.fuel_cmd, b.fuel_cmd, k),
                rpm_cmd: lerp(a.rpm_cmd, b.rpm_cmd, k),
            };
        }
    }
    points[points.len() - 1]
}

fn lerp(a: f64, b: f64, k: f64) -> f64 {
    k.mul_add(b - a, a)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The canonical operating point, cross-checked against the standard
    /// atmosphere: 420.7 hPa at 22,400 ft, and an outside air temperature of
    /// -31 C there is a standard day rather than a hot or a cold one.
    #[test]
    fn cruise_sits_at_the_canonical_operating_point() {
        let c = Profile::Cruise.condition_at(0.0);
        assert!((c.p_amb - 42_070.0).abs() < 200.0, "{} Pa", c.p_amb);
        assert!((c.oat_k - 242.15).abs() < 1.0, "{} K", c.oat_k);
        assert!(
            c.tas_m_s > c.ias_m_s,
            "true airspeed exceeds indicated aloft"
        );
    }

    /// The square-root density relation, which is the correction that makes
    /// radiator flow at altitude two thirds of its sea-level value rather than
    /// the two fifths a naive constant-true-airspeed assumption would give.
    #[test]
    fn true_airspeed_rises_as_density_falls() {
        let low = Profile::HighAltitude.condition_at(0.0);
        let high = Profile::HighAltitude.condition_at(600.0);
        let ratio = (high.tas_m_s / high.ias_m_s) / (low.tas_m_s / low.ias_m_s);
        let expected = (low.rho / high.rho).sqrt();
        assert!((ratio - expected).abs() < 1e-6, "{ratio} vs {expected}");
    }

    #[test]
    fn a_profile_holds_its_last_point_rather_than_ending() {
        let end = Profile::Transients.condition_at(1e6);
        assert!((end.fuel_cmd - 0.2).abs() < 1e-9);
    }

    #[test]
    fn the_transient_step_is_faster_than_the_turbocharger() {
        let before = Profile::Transients.condition_at(20.0);
        let after = Profile::Transients.condition_at(20.2);
        assert!((before.fuel_cmd - 0.2).abs() < 1e-9);
        assert!((after.fuel_cmd - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_hot_day_lowers_density_without_lowering_pressure() {
        let hot = Profile::HotWeather.condition_at(0.0);
        let standard = engine_model::atmosphere::isa(0.0);
        assert!((hot.p_amb - standard.p).abs() < 1.0);
        assert!(hot.rho < standard.rho);
    }
}
