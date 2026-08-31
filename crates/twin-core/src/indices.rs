//! Subsystem health indices, and the arithmetic behind each one.
//!
//! An index is a number between 0 and 100 with a stated reason. Every one here is a
//! linear fraction of the distance a named quantity has travelled from where it
//! sits on a healthy engine to where it stops meeting its duty, which is why each
//! carries the quantity and the value that produced it. An operator asking why
//! lubrication reads 61 gets an answer in the units the engine is designed in
//! rather than a score.
//!
//! # Two of the seven are not model based, and say so
//!
//! Bus voltage and vibration have no counterpart in the engine model: nothing in a
//! mean value model knows about an alternator or a crankcase accelerometer, so
//! there is no parameter for the filter to estimate and no residual to score. Those
//! two are ordinary threshold monitoring against a nominal band, which is exactly
//! what this system exists to move beyond, and they are marked so nothing downstream
//! presents them as inferred.

use engine_model::CYLINDERS;

use crate::channels::index as ch;
use crate::health::{DESCRIPTORS, Health, index as th};

/// How many subsystems are scored.
pub const INDICES: usize = 7;

/// Position of each subsystem in the index array.
pub mod index {
    /// Combustion quality.
    pub const COMBUSTION: usize = 0;
    /// Cooling circuit and heat rejection.
    pub const THERMAL: usize = 1;
    /// Lubrication circuit.
    pub const LUBRICATION: usize = 2;
    /// Air path, compressor and turbine.
    pub const AIR_PATH: usize = 3;
    /// Fuel delivery and injection.
    pub const FUEL: usize = 4;
    /// Electrical bus.
    pub const ELECTRICAL: usize = 5;
    /// Mechanical condition, from vibration.
    pub const MECHANICAL: usize = 6;
}

/// Display name of each subsystem, in index order.
pub const NAMES: [&str; INDICES] = [
    "COMBUSTION",
    "THERMAL",
    "LUBRICATION",
    "AIR PATH",
    "FUEL / INJECTION",
    "ELECTRICAL",
    "MECHANICAL",
];

/// Whether each index comes from the estimated health parameters or from a
/// threshold on a raw channel.
pub const MODEL_BASED: [bool; INDICES] = [true, true, true, true, true, false, false];

/// Channels the engine model does not describe.
#[derive(Clone, Copy, Debug, Default)]
pub struct Auxiliary {
    /// Electrical bus voltage, V.
    pub bus_v: f64,
    /// Broadband vibration, g RMS.
    pub vib_rms_g: f64,
    /// Kurtosis of the vibration signal.
    pub vib_kurtosis: f64,
}

/// One index and the quantity that set it.
#[derive(Clone, Copy, Debug)]
pub struct Scored {
    /// The index, 0 to 100.
    pub value: f64,
    /// Name of the quantity that drove it.
    pub driver: &'static str,
    /// Current value of that quantity.
    pub driver_value: f64,
    /// Value of that quantity at which the subsystem fails.
    pub driver_limit: f64,
}

/// Bus voltage on a healthy engine-driven alternator, V. **estimated** for a 28 V
/// system: the regulator set point less the drop across the source resistance.
const BUS_NOMINAL_V: f64 = 27.8;
/// Departure from the nominal bus at which the electrical system has failed, V.
const BUS_LIMIT_V: f64 = 2.5;

/// Broadband vibration on a healthy engine at cruise, g RMS. **estimated**.
const VIB_NOMINAL_G: f64 = 1.2;
/// Vibration at which the mechanical condition is unacceptable, g RMS. **estimated**.
const VIB_LIMIT_G: f64 = 4.0;
/// Kurtosis of a healthy signal, which is dominated by firing-order tones and so
/// sits below the 3.0 of a Gaussian. **estimated**; measured at 2.34 on this
/// engine's synthesised channel at cruise.
const KURTOSIS_NOMINAL: f64 = 2.4;
/// Kurtosis at which the signal has become impulsive enough to mean a defect.
const KURTOSIS_LIMIT: f64 = 6.0;

/// Per-cylinder residual dispersion, in standard deviations, at which combustion is
/// scored as failed. Four is beyond anything measurement noise produces on four
/// cylinders and beyond what the injector estimates leave behind.
const DISPERSION_LIMIT: f64 = 4.0;

/// Score every subsystem.
///
/// `unexplained` is the filter's **innovation** in standard deviations, not the
/// residual against a healthy engine. The distinction is the whole of the
/// combustion index: the residual against a healthy engine contains every fault,
/// including the ones already attributed to a health parameter, so scoring
/// combustion on it makes a coked injector degrade combustion as well as fuel and
/// the diagnosis names two subsystems for one fault. The innovation is what is left
/// after the parameters have explained what they can, which is the only thing that
/// can honestly be called unexplained.
#[must_use]
pub fn evaluate(health: &Health, unexplained: &[f64], aux: &Auxiliary) -> [Scored; INDICES] {
    let mut out = [Scored {
        value: f64::NAN,
        driver: "",
        driver_value: f64::NAN,
        driver_limit: f64::NAN,
    }; INDICES];

    out[index::FUEL] = worst_parameter(
        health,
        &[
            th::INJECTOR,
            th::INJECTOR + 1,
            th::INJECTOR + 2,
            th::INJECTOR + 3,
        ],
    );
    out[index::AIR_PATH] =
        worst_parameter(health, &[th::ETA_VOL, th::ETA_COMPRESSOR, th::ETA_TURBINE]);
    out[index::THERMAL] = worst_parameter(health, &[th::RADIATOR, th::HEAD_CONDUCTANCE]);
    out[index::LUBRICATION] = worst_parameter(health, &[th::OIL_SUPPLY]);
    out[index::COMBUSTION] = combustion(unexplained);
    out[index::ELECTRICAL] = electrical(aux);
    out[index::MECHANICAL] = mechanical(aux);
    out
}

/// The lowest score among a group of health parameters.
///
/// Lowest rather than an average: a subsystem is as healthy as its worst component,
/// and averaging four injectors would let three good ones hide one that is failing,
/// which is the whole failure this system exists to catch.
fn worst_parameter(health: &Health, members: &[usize]) -> Scored {
    let mut worst = Scored {
        value: 100.0,
        driver: "",
        driver_value: f64::NAN,
        driver_limit: f64::NAN,
    };
    for &i in members {
        let value = (100.0 * (1.0 - health.consumed(i))).clamp(0.0, 100.0);
        if value <= worst.value {
            worst = Scored {
                value,
                driver: DESCRIPTORS[i].name,
                driver_value: health.values[i],
                driver_limit: DESCRIPTORS[i].failure,
            };
        }
    }
    worst
}

/// Combustion quality, from what the health parameters could not explain.
///
/// The filter absorbs a restricted injector into that injector's discharge
/// coefficient, so a coked nozzle leaves the per-cylinder innovations flat and this
/// index high, which is correct: the fuel system is degraded and combustion is not.
/// What it catches is a cylinder whose exhaust and excess air ratio disagree with
/// its neighbours in a way no estimate can account for, which is misfire and cyclic
/// variability.
///
/// Misfire is scored here rather than from a parameter of its own **on purpose**,
/// and `crate::health` has the measurement behind that: a per-cylinder combustion
/// efficiency is not identifiable against one total fuel flow, and carrying it cost
/// more on the injector estimates than it bought. The consequence to know is that
/// the filter also spends injector and turbine efficiency partly explaining a
/// misfire, so the rail names more than one subsystem for it. Telling misfire from
/// coking is a residual pattern, not an index.
fn combustion(unexplained: &[f64]) -> Scored {
    let dispersion = |base: usize| -> f64 {
        let mean: f64 =
            (0..CYLINDERS).map(|i| unexplained[base + i]).sum::<f64>() / CYLINDERS as f64;
        (0..CYLINDERS)
            .map(|i| (unexplained[base + i] - mean).abs())
            .fold(0.0, f64::max)
    };
    let worst = dispersion(ch::EGT).max(dispersion(ch::LAMBDA));
    Scored {
        value: (100.0 * (1.0 - worst / DISPERSION_LIMIT)).clamp(0.0, 100.0),
        driver: "cylinder spread",
        driver_value: worst,
        driver_limit: DISPERSION_LIMIT,
    }
}

/// Electrical bus condition. Threshold monitoring, not a model residual.
///
/// Both directions count. A sagging bus is a failing alternator and an over-volt
/// one is a failing regulator, which boils the battery, so the score is on the
/// absolute departure. The limit reported alongside is the one on the side the bus
/// has actually gone, because a readout pairing an over-volt reading with the
/// under-volt limit reads as healthy at exactly the moment it is not.
fn electrical(aux: &Auxiliary) -> Scored {
    let departure = aux.bus_v - BUS_NOMINAL_V;
    Scored {
        value: (100.0 * (1.0 - departure.abs() / BUS_LIMIT_V)).clamp(0.0, 100.0),
        driver: "bus",
        driver_value: aux.bus_v,
        driver_limit: BUS_NOMINAL_V + BUS_LIMIT_V.copysign(departure),
    }
}

/// Mechanical condition from the vibration channel. Threshold monitoring.
///
/// Both features are scored and the worse one wins, because kurtosis rises for an
/// impulsive defect while the broadband level is still flat: a few large excursions
/// move a fourth moment long before they move a second one.
fn mechanical(aux: &Auxiliary) -> Scored {
    let rms = 1.0 - (aux.vib_rms_g - VIB_NOMINAL_G).max(0.0) / (VIB_LIMIT_G - VIB_NOMINAL_G);
    let kurtosis =
        1.0 - (aux.vib_kurtosis - KURTOSIS_NOMINAL).max(0.0) / (KURTOSIS_LIMIT - KURTOSIS_NOMINAL);
    if kurtosis < rms {
        Scored {
            value: (100.0 * kurtosis).clamp(0.0, 100.0),
            driver: "kurtosis",
            driver_value: aux.vib_kurtosis,
            driver_limit: KURTOSIS_LIMIT,
        }
    } else {
        Scored {
            value: (100.0 * rms).clamp(0.0, 100.0),
            driver: "vibration",
            driver_value: aux.vib_rms_g,
            driver_limit: VIB_LIMIT_G,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::CHANNELS;

    fn healthy_aux() -> Auxiliary {
        Auxiliary {
            bus_v: 27.8,
            vib_rms_g: 1.2,
            vib_kurtosis: KURTOSIS_NOMINAL,
        }
    }

    #[test]
    fn a_healthy_engine_scores_a_hundred_everywhere() {
        let scored = evaluate(&Health::nominal(), &[0.0; CHANNELS], &healthy_aux());
        for (i, s) in scored.iter().enumerate() {
            assert!((s.value - 100.0).abs() < 1e-9, "{}: {}", NAMES[i], s.value);
        }
    }

    /// The story this system is built around: one coked injector, everything else
    /// nominal. Exactly one index may move, or a single-fault diagnosis is being
    /// contradicted by the display beside it.
    #[test]
    fn a_coked_injector_degrades_fuel_and_nothing_else() {
        let mut health = Health::nominal();
        health.values[th::INJECTOR + 2] = 0.811;
        let scored = evaluate(&health, &[0.0; CHANNELS], &healthy_aux());

        assert!(
            (54.0..57.0).contains(&scored[index::FUEL].value),
            "fuel {}",
            scored[index::FUEL].value
        );
        assert_eq!(scored[index::FUEL].driver, "injector-3 Cd");
        for i in 0..INDICES {
            if i != index::FUEL {
                assert!((scored[i].value - 100.0).abs() < 1e-9, "{}", NAMES[i]);
            }
        }
    }

    /// Three healthy injectors must not average away a failing fourth.
    #[test]
    fn a_subsystem_is_as_healthy_as_its_worst_member() {
        let mut health = Health::nominal();
        health.values[th::INJECTOR + 1] = DESCRIPTORS[th::INJECTOR + 1].failure;
        let scored = evaluate(&health, &[0.0; CHANNELS], &healthy_aux());
        assert!(scored[index::FUEL].value.abs() < 1e-9);
        assert_eq!(scored[index::FUEL].driver, "injector-2 Cd");
    }

    /// A cylinder out of step with its neighbours in a way no injector estimate
    /// absorbed is what combustion scores, and a uniform offset on all four is not.
    #[test]
    fn combustion_scores_dispersion_and_ignores_a_common_offset() {
        let mut residual = [0.0; CHANNELS];
        for i in 0..CYLINDERS {
            residual[ch::EGT + i] = 2.5;
        }
        assert!((combustion(&residual).value - 100.0).abs() < 1e-9);

        residual[ch::EGT + 2] = 2.5 + 3.0;
        let scored = combustion(&residual);
        assert!(scored.value < 45.0, "{}", scored.value);
    }

    #[test]
    fn a_sagging_bus_and_an_impulsive_signal_each_show_up() {
        let sagging = Auxiliary {
            bus_v: 26.55,
            ..healthy_aux()
        };
        let scored = evaluate(&Health::nominal(), &[0.0; CHANNELS], &sagging)[index::ELECTRICAL];
        assert!((scored.value - 50.0).abs() < 1e-9);
        assert!(
            (scored.driver_limit - 25.3).abs() < 1e-9,
            "{}",
            scored.driver_limit
        );

        // Same score for the same departure the other way, but the limit shown
        // beside it has to be the one the bus is heading towards.
        let boiling = Auxiliary {
            bus_v: 29.05,
            ..healthy_aux()
        };
        let scored = evaluate(&Health::nominal(), &[0.0; CHANNELS], &boiling)[index::ELECTRICAL];
        assert!((scored.value - 50.0).abs() < 1e-9);
        assert!(
            (scored.driver_limit - 30.3).abs() < 1e-9,
            "{}",
            scored.driver_limit
        );

        let impulsive = Auxiliary {
            vib_kurtosis: 4.2,
            ..healthy_aux()
        };
        let scored = evaluate(&Health::nominal(), &[0.0; CHANNELS], &impulsive);
        assert_eq!(scored[index::MECHANICAL].driver, "kurtosis");
        assert!((scored[index::MECHANICAL].value - 50.0).abs() < 1e-9);
    }

    #[test]
    fn every_subsystem_has_a_name_and_a_stated_provenance() {
        assert_eq!(NAMES.len(), INDICES);
        assert_eq!(MODEL_BASED.len(), INDICES);
        assert!(!MODEL_BASED[index::ELECTRICAL]);
        assert!(!MODEL_BASED[index::MECHANICAL]);
        assert_eq!(MODEL_BASED.iter().filter(|m| **m).count(), 5);
    }
}
