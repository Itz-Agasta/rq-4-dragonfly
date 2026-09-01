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
    /// Brake power as a fraction of the rating. Carried because the vibration
    /// baseline moves with load and a threshold without it false-alarms at power.
    pub load_fraction: f64,
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

/// Broadband vibration on a healthy engine at no load and at the rating, g RMS.
///
/// A **baseline**, not a limit, and the reason it is two numbers is that a
/// reciprocating engine's crankcase acceleration rises with the rate of cylinder
/// pressure rise, so a single figure is the nominal at exactly one power setting
/// and a false alarm everywhere else. Measured on the simulated channel at 0.75 g
/// unloaded and 2.04 g at the rating; `dragonfly-sim` pins both.
const VIB_BASELINE_G: [f64; 2] = [0.75, 2.04];
/// Vibration at which the mechanical condition is unacceptable, g RMS.
/// **estimated**, and an absolute machine limit rather than a departure from the
/// baseline, so the margin genuinely narrows as the engine is worked harder.
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
    out[index::AIR_PATH] = air_path(health);
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

/// Value of the turbomachine efficiency product at which the air path has failed.
///
/// One of the pair at its own limit with the other at nominal, which is the same
/// number both descriptors carry.
const TURBO_PRODUCT_FAILURE: f64 = 0.85;

/// Air path condition, scored on the combination the measurements can actually
/// resolve rather than on the individual estimates.
///
/// Compressor and turbine efficiency are **not separately identifiable here**. The
/// turbocharger shaft settles where the two powers balance, and that balance moves
/// with the product of the efficiencies; the ratio leaves it almost unchanged. So
/// the filter is nearly indifferent along the ridge `eta_c` up and `eta_t` down,
/// settles anywhere on it, and scoring the worst of the two individually reported a
/// healthy engine at 91 on one run and 100 on the next, with nothing wrong either
/// time. Scoring the product reports what was measured and discards what was not.
///
/// The consequence to know: this cannot say **which** of the two has degraded, and
/// it should not, because the instrumentation on this bus cannot either. Separating
/// them needs a compressor outlet temperature, which no message here carries.
/// Volumetric efficiency stays separate; it is identifiable against mass air flow.
fn air_path(health: &Health) -> Scored {
    let vol = worst_parameter(health, &[th::ETA_VOL]);
    let product = health.values[th::ETA_COMPRESSOR] * health.values[th::ETA_TURBINE];
    let consumed = ((1.0 - product) / (1.0 - TURBO_PRODUCT_FAILURE)).max(0.0);
    let turbo = Scored {
        value: (100.0 * (1.0 - consumed)).clamp(0.0, 100.0),
        driver: "eta_c x eta_t",
        driver_value: product,
        driver_limit: TURBO_PRODUCT_FAILURE,
    };
    if turbo.value <= vol.value { turbo } else { vol }
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

/// Broadband vibration a healthy engine produces at this load, g RMS.
///
/// Linear between the two measured endpoints. The true curve is a hyperbola,
/// because the level is the tones and the noise floor added in quadrature, but it
/// departs from the chord by at most 1.1% over the whole load range, which is
/// three hundredths of an index point.
fn vibration_baseline(load_fraction: f64) -> f64 {
    let l = load_fraction.clamp(0.0, 1.0);
    VIB_BASELINE_G[0] + l * (VIB_BASELINE_G[1] - VIB_BASELINE_G[0])
}

/// Mechanical condition from the vibration channel. Threshold monitoring.
///
/// Both features are scored and the worse one wins, because kurtosis rises for an
/// impulsive defect while the broadband level is still flat: a few large excursions
/// move a fourth moment long before they move a second one.
///
/// The broadband level is scored against a **load-scheduled** baseline. Against one
/// fixed nominal this index read 72 on a healthy engine at full power, because the
/// nominal was the cruise level and a healthy climb sits near 2.0 g: a false alarm
/// on every full-power beat of a demonstration. Widening the limit would have hidden
/// a real defect at cruise instead, so the baseline moves and the limit does not.
fn mechanical(aux: &Auxiliary) -> Scored {
    let nominal = vibration_baseline(aux.load_fraction);
    let rms = 1.0 - (aux.vib_rms_g - nominal).max(0.0) / (VIB_LIMIT_G - nominal);
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

    /// A healthy engine at the cruise load the reference channels are quoted at.
    fn healthy_aux() -> Auxiliary {
        let load = 0.35;
        Auxiliary {
            bus_v: 27.8,
            vib_rms_g: vibration_baseline(load),
            vib_kurtosis: KURTOSIS_NOMINAL,
            load_fraction: load,
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

    /// The false alarm this schedule exists to remove. A healthy engine at full
    /// power sits near 2.0 g, and against the old fixed 1.2 g cruise nominal that
    /// scored 70: an alarm on every full-power beat of the demonstration.
    #[test]
    fn a_healthy_engine_is_not_a_mechanical_alarm_at_any_load() {
        for load in [0.0, 0.35, 0.7, 1.0] {
            let aux = Auxiliary {
                vib_rms_g: vibration_baseline(load),
                load_fraction: load,
                ..healthy_aux()
            };
            let scored = mechanical(&aux);
            assert!(
                (scored.value - 100.0).abs() < 1e-9,
                "load {load}: {}",
                scored.value
            );
        }

        // A fixed cruise nominal is what produced the false alarm, and the
        // arithmetic is here so nobody reintroduces it thinking it was harmless.
        let at_rating = vibration_baseline(1.0);
        let against_cruise_nominal =
            100.0 * (1.0 - (at_rating - vibration_baseline(0.35)) / (VIB_LIMIT_G - 1.2));
        assert!(
            (65.0..75.0).contains(&against_cruise_nominal),
            "{against_cruise_nominal}"
        );
    }

    /// A real defect still has to show, and the load schedule must not swallow it.
    #[test]
    fn a_rough_engine_still_scores_badly_at_full_power() {
        let aux = Auxiliary {
            vib_rms_g: 3.0,
            load_fraction: 1.0,
            ..healthy_aux()
        };
        let scored = mechanical(&aux);
        assert_eq!(scored.driver, "vibration");
        assert!(scored.value < 55.0, "{}", scored.value);
    }

    /// Compressor and turbine efficiency trade off against each other along a
    /// ridge the shaft power balance barely sees, so the filter settles anywhere
    /// on it. Scoring them individually reported 91 and then 100 on two runs of a
    /// healthy engine; scoring the product reports what was actually measured.
    #[test]
    fn the_air_path_ignores_the_ridge_the_filter_wanders_along() {
        let mut health = Health::nominal();
        health.values[th::ETA_COMPRESSOR] = 0.9865;
        health.values[th::ETA_TURBINE] = 1.0135;
        let scored = air_path(&health);
        assert!(scored.value > 99.0, "{}", scored.value);
        assert_eq!(scored.driver, "eta_c x eta_t");

        // Scored one at a time, that same estimate calls a healthy engine 91.
        let individually =
            worst_parameter(&health, &[th::ETA_VOL, th::ETA_COMPRESSOR, th::ETA_TURBINE]);
        assert!(
            (90.0..93.0).contains(&individually.value),
            "{}",
            individually.value
        );
    }

    /// What the product cannot do is say which of the pair moved, and it must not
    /// pretend to. What it must do is notice that the pair moved together.
    #[test]
    fn the_air_path_still_sees_a_turbocharger_losing_efficiency() {
        let mut health = Health::nominal();
        health.values[th::ETA_COMPRESSOR] = 0.90;
        let scored = air_path(&health);
        assert!((30.0..35.0).contains(&scored.value), "{}", scored.value);
        assert!((scored.driver_value - 0.90).abs() < 1e-12);
    }

    /// Volumetric efficiency is identifiable against mass air flow, so it stays a
    /// parameter of its own and can still be the thing that sets the index.
    #[test]
    fn volumetric_efficiency_is_reported_separately_from_the_turbocharger() {
        let mut health = Health::nominal();
        health.values[th::ETA_VOL] = 0.90;
        let scored = air_path(&health);
        assert_eq!(scored.driver, "eta_vol");
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
