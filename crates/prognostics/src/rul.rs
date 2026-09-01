//! Remaining useful life: how long until a parameter reaches the value at which its
//! subsystem stops meeting its duty.
//!
//! The physics half of the hybrid. A health parameter is extrapolated along its
//! fitted trend to its failure threshold, and the interval comes from the two
//! uncertainties the fit already produced. Nothing is trained and no data is needed,
//! which is why this ships whether or not the learned correction ever does.
//!
//! **The interval is covariance-derived and it is not conformal**, whatever
//! `design.md` says: conformal prediction earns its coverage from a calibration set
//! of run-to-failure trajectories and there is none until D12's recorder.
//!
//! Celaya, Saxena & Goebel, PHM Society 2012, for the forecast-to-threshold
//! formulation and the warning that a remaining life is a ratio and not normal.
//! <https://doi.org/10.36001/phmconf.2012.v4i1.2110>

use serde::Serialize;
use twin_core::health::{DESCRIPTORS, PARAMS};
use twin_core::indices::{INDICES, index as sub};

use crate::trend::{Trend, Trends};

/// Normal quantile at 10% and 90%, the band the display draws.
const Z_P10: f64 = 1.281_551_565_5;

/// Longest remaining life worth reporting, hours.
///
/// Beyond this the extrapolation is claiming to see further ahead than a 30-hour
/// mission and further than the trend window can support, and a five-figure number
/// on a display reads as broken rather than as reassuring.
const HORIZON_H: f64 = 1000.0;

/// Remaining life of one parameter or subsystem.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Rul {
    /// Median estimate, hours. `None` when nothing is degrading, which is not the
    /// same as zero and must never be rendered as zero.
    pub hours: Option<f64>,
    /// Lower bound, hours. The number a go/no-go decision is taken on.
    pub p10: Option<f64>,
    /// Upper bound, hours. `None` while the decline is too slow to bound above.
    pub p90: Option<f64>,
    /// Name of the parameter that produced it.
    pub driver: &'static str,
    /// How far that parameter has travelled from nominal towards failure, 0 to 1.
    pub consumed: f64,
    /// Rate of decline, per hour, as a positive number.
    pub rate_per_hour: f64,
}

/// Which parameters each subsystem's life is limited by.
///
/// The same grouping the health indices use, and it has to stay the same: a rail
/// reading FUEL 56 beside a life computed from a compressor is two displays
/// contradicting each other about one engine.
///
/// **The turbomachine efficiencies are deliberately absent.** They are not separately
/// identifiable, so either one alone shows a trend the pair does not have; measured
/// live, the index read 100.0 while a life from `eta_t` alone read 2.41 h on an air
/// path that was healthy throughout. Giving the pair a life needs the product's own
/// trend, which `Trends` does not carry.
const MEMBERS: [&[usize]; INDICES] = {
    use twin_core::health::index as th;
    let mut m: [&[usize]; INDICES] = [&[]; INDICES];
    m[sub::FUEL] = &[
        th::INJECTOR,
        th::INJECTOR + 1,
        th::INJECTOR + 2,
        th::INJECTOR + 3,
    ];
    m[sub::AIR_PATH] = &[th::ETA_VOL];
    m[sub::THERMAL] = &[th::RADIATOR, th::HEAD_CONDUCTANCE];
    m[sub::LUBRICATION] = &[th::OIL_SUPPLY];
    m
};

/// Remaining life of every parameter and every subsystem.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Prognosis {
    /// Per health parameter, in [`twin_core::health::DESCRIPTORS`] order.
    pub parameter: [Rul; PARAMS],
    /// Per subsystem, in [`twin_core::indices::NAMES`] order.
    pub subsystem: [Rul; INDICES],
    /// The subsystem that will reach its threshold first, if any will.
    pub limiting: Option<usize>,
}

/// Project every parameter to its threshold.
#[must_use]
pub fn evaluate(trends: &Trends) -> Prognosis {
    let parameter: [Rul; PARAMS] =
        std::array::from_fn(|i| project(i, trends.get(i), trends.is_degrading(i)));

    let subsystem: [Rul; INDICES] = std::array::from_fn(|s| soonest(MEMBERS[s], &parameter));

    // The mission is limited by whichever subsystem runs out first, on the lower
    // bound rather than the median: a dispatch decision taken on a median is taken
    // on a coin flip.
    let limiting = (0..INDICES)
        .filter(|s| subsystem[*s].p10.is_some())
        .min_by(|a, b| {
            subsystem[*a]
                .p10
                .unwrap_or(f64::INFINITY)
                .total_cmp(&subsystem[*b].p10.unwrap_or(f64::INFINITY))
        });

    Prognosis {
        parameter,
        subsystem,
        limiting,
    }
}

/// Extrapolate one parameter to its failure threshold.
///
/// Every parameter degrades downward, so the distance to failure is the value less
/// the threshold and the rate is the negated slope. A parameter already past its
/// threshold reports zero rather than a negative life, because "overdue" and "three
/// hours ago" are the same operational statement and only one of them is readable.
fn project(i: usize, trend: &Trend, degrading: bool) -> Rul {
    let d = &DESCRIPTORS[i];
    let mut out = Rul {
        driver: d.name,
        consumed: ((d.nominal - trend.value) / (d.nominal - d.failure)).max(0.0),
        ..Rul::default()
    };
    if !trend.ready {
        out.consumed = 0.0;
        return out;
    }
    if !degrading {
        return out;
    }

    let rate = -trend.slope;
    out.rate_per_hour = rate * 3600.0;
    let distance = trend.value - d.failure;
    if distance <= 0.0 {
        out.hours = Some(0.0);
        out.p10 = Some(0.0);
        out.p90 = Some(0.0);
        return out;
    }

    let seconds = distance / rate;
    // First-order propagation through the ratio. The slope term dominates far ahead
    // of the window, correctly: a slope known to ten percent cannot place a crossing
    // better than that however precisely the current value is known.
    let variance =
        (trend.value_sigma / rate).powi(2) + (distance * trend.slope_sigma / (rate * rate)).powi(2);
    let sigma = variance.sqrt();

    out.hours = Some(cap(seconds / 3600.0));
    out.p10 = Some(cap((seconds - Z_P10 * sigma).max(0.0) / 3600.0));
    // Above the horizon the upper bound is the absence of a number, not a large one:
    // a cone drawn to a capped value claims precision the fit does not have.
    let upper = (seconds + Z_P10 * sigma) / 3600.0;
    out.p90 = (upper <= HORIZON_H).then_some(upper);
    out
}

/// The member that runs out first, on the lower bound.
fn soonest(members: &[usize], parameter: &[Rul; PARAMS]) -> Rul {
    members
        .iter()
        .map(|i| parameter[*i])
        .filter(|r| r.p10.is_some())
        .min_by(|a, b| {
            a.p10
                .unwrap_or(f64::INFINITY)
                .total_cmp(&b.p10.unwrap_or(f64::INFINITY))
        })
        .unwrap_or_else(|| {
            // Nothing here is degrading. The subsystem still reports which member is
            // furthest along, because "nothing is failing and this is the one to
            // watch" is more useful than an empty row.
            members
                .iter()
                .map(|i| parameter[*i])
                .max_by(|a, b| a.consumed.total_cmp(&b.consumed))
                .unwrap_or_default()
        })
}

/// Clamp a projection to the horizon it can honestly support.
fn cap(hours: f64) -> f64 {
    hours.min(HORIZON_H)
}

#[cfg(test)]
mod tests {
    use super::*;
    use twin_core::health::index as th;

    /// Drive the trend estimator with a parameter declining at a stated rate.
    fn declining(param: usize, from: f64, per_hour: f64, seconds: usize, noise: f64) -> Trends {
        let mut trends = Trends::new();
        let mut seed = 0x1234_5678u64;
        let mut jitter = || {
            seed ^= seed >> 12;
            seed ^= seed << 25;
            seed ^= seed >> 27;
            let u = (seed.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64;
            (u - 0.5) * 2.0 * noise
        };
        for k in 0..seconds * 20 {
            let t = k as f64 * 0.05;
            let mut theta: [f64; PARAMS] = std::array::from_fn(|i| DESCRIPTORS[i].nominal);
            theta[param] = (t / 3600.0).mul_add(per_hour, from) + jitter();
            trends.observe(t, &theta);
        }
        trends
    }

    /// The arithmetic, checked by hand, and the projection starts from where the
    /// parameter is **now** rather than where it started. Injector 3 enters at
    /// 0.90 and declines for the 900 s window, so it is at 0.895 when the fit ends;
    /// against a 0.62 threshold that is 0.275 to go at 0.02 an hour, or 13.75 h.
    /// Getting 14.0 would mean the extrapolation begins at the start of the window,
    /// which is a quarter of an hour of credit the engine has already spent.
    #[test]
    fn a_declining_injector_reaches_its_threshold_when_arithmetic_says_it_will() {
        let i = th::INJECTOR + 2;
        let p = evaluate(&declining(i, 0.90, -0.02, 900, 0.0));
        let hours = p.parameter[i]
            .hours
            .expect("a declining parameter has a life");
        assert!((hours - 13.75).abs() < 0.05, "{hours} h");
        assert_eq!(p.parameter[i].driver, "injector-3 Cd");
        assert!((p.parameter[i].rate_per_hour - 0.02).abs() < 1e-6);
    }

    /// A healthy engine has no remaining life to report, and `None` is not zero.
    /// A display that renders this as 0 h grounds a serviceable aircraft.
    #[test]
    fn a_healthy_engine_reports_no_remaining_life_rather_than_none_left() {
        let p = evaluate(&declining(th::INJECTOR + 2, 0.966, 0.0, 900, 0.003));
        for r in &p.parameter {
            assert!(r.hours.is_none(), "{} reported {:?}", r.driver, r.hours);
            assert!(r.p10.is_none());
        }
        assert!(p.limiting.is_none());
    }

    /// The interval has to bracket the estimate and be wider when the parameter
    /// wanders more, or it is decoration.
    #[test]
    fn the_interval_brackets_the_estimate_and_widens_with_wander() {
        let i = th::INJECTOR + 2;
        let clean = evaluate(&declining(i, 0.90, -0.02, 900, 1.0e-7)).parameter[i];
        let noisy = evaluate(&declining(i, 0.90, -0.02, 900, 2.0e-6)).parameter[i];

        for r in [clean, noisy] {
            let (h, lo) = (r.hours.unwrap(), r.p10.unwrap());
            assert!(lo <= h, "p10 {lo} above the median {h}");
            if let Some(hi) = r.p90 {
                assert!(hi >= h, "p90 {hi} below the median {h}");
            }
        }
        let clean_width = clean.hours.unwrap() - clean.p10.unwrap();
        let noisy_width = noisy.hours.unwrap() - noisy.p10.unwrap();
        assert!(
            noisy_width > clean_width,
            "{noisy_width} against {clean_width}"
        );
    }

    /// A subsystem is as short-lived as its worst member, and it must report which
    /// member that was so the rail and the prognosis name the same thing.
    #[test]
    fn a_subsystem_takes_the_life_of_its_soonest_member() {
        let p = evaluate(&declining(th::INJECTOR + 1, 0.90, -0.05, 900, 0.0));
        let fuel = p.subsystem[sub::FUEL];
        assert_eq!(fuel.driver, "injector-2 Cd");
        assert_eq!(p.limiting, Some(sub::FUEL));
        assert!(fuel.hours.unwrap() < 6.0, "{:?}", fuel.hours);
    }

    /// A parameter already past its threshold is overdue, not negative.
    #[test]
    fn a_parameter_past_its_threshold_reports_nothing_left() {
        let i = th::INJECTOR + 2;
        let p = evaluate(&declining(i, 0.60, -0.02, 900, 0.0)).parameter[i];
        assert_eq!(p.hours, Some(0.0));
        assert!(p.consumed > 1.0, "{}", p.consumed);
    }

    /// A slow decline projects beyond any useful horizon, and the upper bound then
    /// has to be absent rather than a large invented number. An interval drawn to
    /// a capped value claims a precision the fit does not have.
    #[test]
    fn a_very_slow_decline_has_no_upper_bound() {
        let i = th::RADIATOR;
        let p = evaluate(&declining(i, 0.999, -0.000_2, 1800, 0.000_02)).parameter[i];
        if let Some(h) = p.hours {
            assert!(h <= HORIZON_H);
            assert!(p.p90.is_none(), "{:?}", p.p90);
        }
    }

    /// Nothing degrading still names the member worth watching, because an empty
    /// row tells an operator less than a quiet one.
    #[test]
    fn a_quiet_subsystem_still_names_its_most_worn_member() {
        let mut trends = Trends::new();
        for k in 0..900 * 20 {
            let t = k as f64 * 0.05;
            let mut theta: [f64; PARAMS] = std::array::from_fn(|i| DESCRIPTORS[i].nominal);
            theta[th::INJECTOR + 3] = 0.94;
            trends.observe(t, &theta);
        }
        let p = evaluate(&trends);
        assert_eq!(p.subsystem[sub::FUEL].driver, "injector-4 Cd");
        assert!(p.subsystem[sub::FUEL].hours.is_none());
        assert!(p.subsystem[sub::FUEL].consumed > 0.0);
    }

    /// The turbo pair must stay out of the air path's life, or the prognosis
    /// contradicts the index beside it. On a live coking run the index read 100.0
    /// while a life computed from `eta_t` alone read 2.41 h, on an air path that
    /// was healthy the whole time.
    #[test]
    fn the_air_path_life_ignores_the_ridge_the_filter_wanders_along() {
        use twin_core::health::index as th;
        assert!(!MEMBERS[sub::AIR_PATH].contains(&th::ETA_COMPRESSOR));
        assert!(!MEMBERS[sub::AIR_PATH].contains(&th::ETA_TURBINE));

        // A turbine estimate drifting on its own must not produce a life at all.
        let p = evaluate(&declining(th::ETA_TURBINE, 1.0, -0.02, 900, 1.0e-7));
        assert!(
            p.subsystem[sub::AIR_PATH].hours.is_none(),
            "{:?}",
            p.subsystem[sub::AIR_PATH].hours
        );
        // Volumetric efficiency is identifiable against mass air flow, and still is.
        let p = evaluate(&declining(th::ETA_VOL, 1.0, -0.02, 900, 1.0e-7));
        assert!(p.subsystem[sub::AIR_PATH].hours.is_some());
        assert_eq!(p.subsystem[sub::AIR_PATH].driver, "eta_vol");
    }

    /// Every subsystem the health rail scores that has parameters behind it must
    /// have them listed here, or a rail row exists with no prognosis under it.
    #[test]
    fn every_model_based_subsystem_has_members() {
        use twin_core::indices::MODEL_BASED;
        for (s, members) in MEMBERS.iter().enumerate() {
            assert_eq!(
                !members.is_empty(),
                MODEL_BASED[s] && s != sub::COMBUSTION,
                "subsystem {s}"
            );
        }
        // Combustion is scored from the innovation and has no parameter of its
        // own, deliberately; `twin_core::health` records why one was removed.
        assert!(MEMBERS[sub::COMBUSTION].is_empty());
    }
}
