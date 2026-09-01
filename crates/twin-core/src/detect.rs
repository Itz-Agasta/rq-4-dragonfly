//! Anomaly detection on the residual vector, and the monitor it is measured against.
//!
//! Two tests, answering different questions. The Mahalanobis distance asks whether
//! *this* frame is unusual; the CUSUM asks whether the last few minutes have been
//! drifting. A degrading injector never makes any single frame unusual, so only the
//! second catches it: a detector that only thresholds is blind to a drift, and one
//! that only integrates is blind to a step.
//!
//! [`Redline`] is the conventional monitor, running on the same frames, so the lead
//! time is a difference between two timestamps this module produced.

use engine_model::params::Redline as RedlineLimits;
use serde::Serialize;

use crate::channels::{CHANNELS, Measurement, index as ch};

/// Slack, in standard deviations, the CUSUM absorbs before it accumulates.
///
/// Half the shift it is tuned to notice, the classical choice: with the decision
/// interval below it gives an in-control run length near 465 samples per channel.
/// Page, "Continuous Inspection Schemes", Biometrika 41(1/2), 1954.
/// <https://doi.org/10.1093/biomet/41.1-2.100>
const CUSUM_SLACK: f64 = 0.5;

/// Decision interval. The accumulated excursion at which a channel has drifted.
const CUSUM_LIMIT: f64 = 5.0;

/// Seconds averaged into the per-channel baseline before the CUSUM runs.
///
/// The residual against a healthy engine is **not zero-mean**: model-plant mismatch
/// leaves a standing offset, measured at -0.69 sigma on brake torque, and a CUSUM
/// fed a constant bias crosses its decision interval in under two seconds on a
/// healthy machine. So the engine is baselined as found and the CUSUM watches for a
/// change from that. **The limitation: a fault already present when the monitor
/// starts is not a change and this will not see it**, which is what the residual
/// magnitude, the indices and the signature matrix are for.
const BASELINE_S: f64 = 60.0;

/// False-alarm probability the Mahalanobis threshold is set at.
///
/// One frame in a thousand on a healthy engine, before the consecutive-frame
/// requirement, which is what makes the rate liveable at 20 Hz.
const ALPHA: f64 = 1.0e-3;

/// What the detector concluded about one frame.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Detection {
    /// Mahalanobis distance of the residual vector, in standard deviations.
    pub distance: f64,
    /// Distance a healthy engine exceeds with probability [`ALPHA`].
    pub distance_limit: f64,
    /// Largest accumulated CUSUM excursion across channels.
    pub cusum: f64,
    /// Value of that excursion at which a channel is declared drifted.
    pub cusum_limit: f64,
    /// Channel carrying the largest excursion.
    pub cusum_channel: &'static str,
    /// Whether the standing bias is still being measured. No excursion accumulates
    /// while this is true, so `drift` being false says nothing about the engine.
    pub calibrating: bool,
    /// Whether the residual vector is anomalous now.
    pub anomaly: bool,
    /// Whether some channel has drifted persistently.
    pub drift: bool,
    /// Time the drift alarm first latched, s. `None` until it does.
    pub drift_since: Option<f64>,
    /// Time the conventional redline monitor first tripped, s.
    pub redline_since: Option<f64>,
    /// Which limit that was, empty until one trips.
    pub redline_channel: &'static str,
    /// Seconds by which the drift alarm preceded the redline. `None` while either
    /// has yet to happen. Negative would mean the threshold won, which is the
    /// result that has to be reportable for the comparison to be worth anything.
    pub lead_time_s: Option<f64>,
}

impl Default for Detection {
    fn default() -> Self {
        Self {
            distance: 0.0,
            distance_limit: chi_square_quantile(CHANNELS as f64, 1.0 - ALPHA).sqrt(),
            cusum: 0.0,
            cusum_limit: CUSUM_LIMIT,
            cusum_channel: "",
            calibrating: true,
            anomaly: false,
            drift: false,
            drift_since: None,
            redline_since: None,
            redline_channel: "",
            lead_time_s: None,
        }
    }
}

/// Running detector state. One per twin.
#[derive(Clone, Debug)]
pub struct Detector {
    high: [f64; CHANNELS],
    low: [f64; CHANNELS],
    baseline: [f64; CHANNELS],
    baseline_sum: [f64; CHANNELS],
    baseline_n: u32,
    baseline_until: Option<f64>,
    calibrated: bool,
    consecutive: u32,
    drift_since: Option<f64>,
    redline_since: Option<f64>,
    redline_channel: &'static str,
    limit: f64,
}

impl Default for Detector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector {
    /// Frames the distance must stay above its limit before an anomaly is called.
    ///
    /// The 22 channels are tested jointly every 50 ms and are correlated, so the
    /// nominal false-alarm rate understates the real one. A fifth of a second costs
    /// nothing against a fault developing over hours and removes the single-frame
    /// outliers a lagging instrument produces at a fuelling step.
    const CONSECUTIVE: u32 = 4;

    /// A detector that has seen nothing.
    #[must_use]
    pub fn new() -> Self {
        Self {
            high: [0.0; CHANNELS],
            low: [0.0; CHANNELS],
            baseline: [0.0; CHANNELS],
            baseline_sum: [0.0; CHANNELS],
            baseline_n: 0,
            baseline_until: None,
            calibrated: false,
            consecutive: 0,
            drift_since: None,
            redline_since: None,
            redline_channel: "",
            limit: chi_square_quantile(CHANNELS as f64, 1.0 - ALPHA).sqrt(),
        }
    }

    /// Test one frame.
    ///
    /// `normalised` is the residual against a **healthy** engine, never the filter's
    /// innovation: an estimator carrying health parameters drives its own innovation
    /// to zero whether or not the machine is sick, so a detector fed the innovation
    /// cannot see degradation at all. `crate::nominal` produces the right quantity.
    pub fn update(
        &mut self,
        t_s: f64,
        normalised: &[f64; CHANNELS],
        m: &Measurement,
        limits: &RedlineLimits,
    ) -> Detection {
        // The distance test needs no baseline. It asks whether this frame is far
        // from what a healthy engine would read, and a standing bias of two thirds
        // of a sigma is a legitimate part of that answer.
        let distance = normalised.iter().map(|z| z * z).sum::<f64>().sqrt();
        if distance > self.limit {
            self.consecutive = self.consecutive.saturating_add(1);
        } else {
            self.consecutive = 0;
        }

        let calibrating = self.accumulate_baseline(t_s, normalised);

        let mut worst = 0.0;
        let mut worst_channel = "";
        for (i, (((&z, &bias), high), low)) in normalised
            .iter()
            .zip(&self.baseline)
            .zip(self.high.iter_mut())
            .zip(self.low.iter_mut())
            .enumerate()
        {
            if calibrating {
                continue;
            }
            let centred = z - bias;
            *high = (*high + centred - CUSUM_SLACK).max(0.0);
            *low = (*low - centred - CUSUM_SLACK).max(0.0);
            let excursion = high.max(*low);
            if excursion > worst {
                worst = excursion;
                worst_channel = crate::channels::TABLE[i].name;
            }
        }

        let drift = worst > CUSUM_LIMIT;
        if drift && self.drift_since.is_none() {
            self.drift_since = Some(t_s);
        }

        if self.redline_since.is_none()
            && let Some(name) = Redline::tripped(m, limits)
        {
            self.redline_since = Some(t_s);
            self.redline_channel = name;
        }

        Detection {
            distance,
            distance_limit: self.limit,
            cusum: worst,
            cusum_limit: CUSUM_LIMIT,
            cusum_channel: worst_channel,
            calibrating,
            anomaly: self.consecutive >= Self::CONSECUTIVE,
            drift,
            drift_since: self.drift_since,
            redline_since: self.redline_since,
            redline_channel: self.redline_channel,
            lead_time_s: match (self.drift_since, self.redline_since) {
                (Some(d), Some(r)) => Some(r - d),
                _ => None,
            },
        }
    }

    /// Average the opening frames into the per-channel baseline.
    ///
    /// Returns whether the detector is still calibrating, during which no excursion
    /// accumulates. The window starts at the first frame seen rather than at a
    /// mission time, so a twin re-seeded mid-flight re-baselines rather than
    /// carrying an offset measured at a different operating point.
    fn accumulate_baseline(&mut self, t_s: f64, normalised: &[f64; CHANNELS]) -> bool {
        let until = *self.baseline_until.get_or_insert(t_s + BASELINE_S);
        if t_s >= until {
            self.calibrated = true;
            return false;
        }
        for (sum, z) in self.baseline_sum.iter_mut().zip(normalised) {
            *sum += z;
        }
        self.baseline_n += 1;
        let n = f64::from(self.baseline_n);
        for (bias, sum) in self.baseline.iter_mut().zip(&self.baseline_sum) {
            *bias = sum / n;
        }
        true
    }
}

/// The conventional threshold monitor, for comparison.
///
/// This is what the system in `ps.md` section C is asked to improve on, and it is
/// implemented rather than described so the improvement is a measurement. Limits
/// come from the parameter file; the oil and coolant ones are certificated.
///
/// The result worth knowing in advance: on the demonstration fault it **never
/// trips**. A coked injector delivers less fuel, so on a compression-ignition
/// engine running far lean of any temperature peak its cylinder runs *cooler*, and
/// every limit here is an upper bound except oil pressure. The twin is not merely
/// earlier than the threshold on this fault, it is the only thing that fires.
pub struct Redline;

impl Redline {
    /// The first limit this measurement violates, if any.
    #[must_use]
    pub fn tripped(m: &Measurement, l: &RedlineLimits) -> Option<&'static str> {
        if m.oil_t_k > l.oil_t_max_k {
            return Some("OIL T");
        }
        if m.oil_p_pa < l.oil_p_min_pa {
            return Some("OIL P LOW");
        }
        if m.oil_p_pa > l.oil_p_max_pa {
            return Some("OIL P HIGH");
        }
        if m.coolant_t_k > l.coolant_t_max_k {
            return Some("COOLANT");
        }
        for (i, t) in m.egt_k.iter().enumerate() {
            if *t > l.egt_max_k {
                return Some(cylinder_name(ch::EGT, i));
            }
        }
        for (i, t) in m.cht_k.iter().enumerate() {
            if *t > l.cht_max_k {
                return Some(cylinder_name(ch::CHT, i));
            }
        }
        None
    }
}

/// The channel table's own name for a per-cylinder channel, so an alarm and a
/// residual row cannot disagree about what a cylinder is called.
fn cylinder_name(base: usize, i: usize) -> &'static str {
    crate::channels::TABLE[base + i].name
}

/// Upper quantile of a chi-square distribution with `k` degrees of freedom.
///
/// Wilson-Hilferty: the cube root of a chi-square variate divided by its degrees of
/// freedom is close to normal. Good to better than half a percent for `k` above
/// about 10, which covers every use here, and it avoids taking a dependency on a
/// statistics crate for one quantile evaluated once at construction.
/// Wilson & Hilferty, PNAS 17(12), 1931. <https://doi.org/10.1073/pnas.17.12.684>
fn chi_square_quantile(k: f64, p: f64) -> f64 {
    let z = normal_quantile(p);
    let a = 2.0 / (9.0 * k);
    k * (1.0 - a + z * a.sqrt()).powi(3)
}

/// Upper quantile of the standard normal.
///
/// Acklam's rational approximation, both branches: the central one is accurate to
/// about 1.15e-9 in relative error and the tail one is needed here because `ALPHA`
/// puts the evaluation at p = 0.999, outside the central region's validity.
/// <https://web.archive.org/web/20151030215612/http://home.online.no/~pjacklam/notes/invnorm/>
fn normal_quantile(p: f64) -> f64 {
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239e0,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838e0,
        -2.549_732_539_343_734e0,
        4.374_664_141_464_968e0,
        2.938_163_982_698_78e0,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996e0,
        3.754_408_661_907_42e0,
    ];
    /// Where the central branch stops being the accurate one.
    const BREAK: f64 = 0.02425;

    let tail = |q: f64| {
        let num = ((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5];
        let den = (((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0;
        num / den
    };

    if p < BREAK {
        tail((-2.0 * p.ln()).sqrt())
    } else if p > 1.0 - BREAK {
        -tail((-2.0 * (1.0 - p).ln()).sqrt())
    } else {
        let q = p - 0.5;
        let r = q * q;
        let num = (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q;
        let den = ((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0;
        num / den
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_model::engines;

    fn limits() -> RedlineLimits {
        engines::ae330().limits.redline
    }

    /// A healthy engine at the canonical cruise point, well inside every limit.
    /// Only the channels the redline monitor reads have to be right; the detector
    /// itself never looks at a measurement, it looks at the residual vector.
    fn healthy_measurement() -> Measurement {
        Measurement {
            t_s: 0.0,
            p_amb_pa: 4.03e4,
            oat_k: 242.15,
            ias_m_s: 40.1,
            wastegate: 0.35,
            injection_ms: 1.51,
            rpm: 3720.0,
            map_pa: 1.302e5,
            mat_k: 315.0,
            maf_kg_s: 0.095,
            turbo_rpm: 118_400.0,
            torque_nm: 128.0,
            fuel_flow_kg_h: 11.4,
            oil_p_pa: 4.4e5,
            oil_t_k: 358.0,
            coolant_t_k: 361.0,
            egt_k: [1010.0; 4],
            cht_k: [412.0; 4],
            lambda: [2.04; 4],
            bus_v: 27.8,
            vib_rms_g: 1.2,
            vib_kurtosis: 2.2,
        }
    }

    /// Frames the calibration window swallows before any excursion accumulates.
    const CALIBRATION_FRAMES: usize = (BASELINE_S / 0.05) as usize;

    /// Run the calibration window on a zero residual, so the baseline is zero and a
    /// test can reason about the CUSUM arithmetic directly.
    fn calibrated() -> Detector {
        let mut d = Detector::new();
        let zero = [0.0; CHANNELS];
        let m = healthy_measurement();
        let l = limits();
        for k in 0..=CALIBRATION_FRAMES {
            d.update(k as f64 * 0.05, &zero, &m, &l);
        }
        d
    }

    /// Feed `frames` further frames after calibration has finished.
    fn feed(d: &mut Detector, frames: usize, z: &[f64; CHANNELS]) -> Detection {
        let mut last = Detection::default();
        let m = healthy_measurement();
        let l = limits();
        for k in 0..frames {
            last = d.update(BASELINE_S + k as f64 * 0.05, z, &m, &l);
        }
        last
    }

    /// The failure the first live run found, pinned so it cannot come back.
    ///
    /// A real residual carries a standing offset from model-plant mismatch, and on
    /// the bus brake torque sits at -0.69 sigma indefinitely. Before the baseline
    /// existed this crossed the decision interval in 1.3 s on a healthy engine.
    #[test]
    fn a_standing_model_bias_is_not_a_drift() {
        let mut d = Detector::new();
        let mut seed = 0x2545_F491u64;
        let mut noise = || {
            seed ^= seed >> 12;
            seed ^= seed << 25;
            seed ^= seed >> 27;
            let u = (seed.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64;
            (u - 0.5) * 0.6
        };
        let m = healthy_measurement();
        let l = limits();
        // Twenty minutes at 20 Hz, every channel biased and none of it a fault.
        for k in 0..24_000 {
            let z: [f64; CHANNELS] = std::array::from_fn(|i| {
                let bias = if i == ch::TORQUE { -0.69 } else { 0.31 };
                bias + noise()
            });
            let out = d.update(k as f64 * 0.05, &z, &m, &l);
            assert!(!out.anomaly, "frame {k}: distance {}", out.distance);
            assert!(
                !out.drift,
                "frame {k}: cusum {} on {}",
                out.cusum, out.cusum_channel
            );
        }
        assert!(
            (d.baseline[ch::TORQUE] + 0.69).abs() < 0.05,
            "{}",
            d.baseline[ch::TORQUE]
        );
    }

    /// And a change on top of that bias still has to be caught, or the baseline
    /// has bought a quiet detector by making it deaf.
    #[test]
    fn a_change_on_top_of_a_standing_bias_is_still_caught() {
        let mut d = Detector::new();
        let m = healthy_measurement();
        let l = limits();
        let mut biased = [0.0; CHANNELS];
        biased[ch::TORQUE] = -0.69;
        for k in 0..=CALIBRATION_FRAMES {
            d.update(k as f64 * 0.05, &biased, &m, &l);
        }

        // The engine now moves a further 1.5 sigma on one exhaust channel.
        let mut shifted = biased;
        shifted[ch::EGT + 2] = -1.5;
        let out = feed(&mut d, 30, &shifted);
        assert!(out.drift, "cusum {}", out.cusum);
        assert_eq!(out.cusum_channel, "EGT 3");
    }

    /// Nothing accumulates while the bias is being measured, or the baseline
    /// window itself would trip the alarm it exists to prevent.
    #[test]
    fn no_excursion_accumulates_during_calibration() {
        let mut d = Detector::new();
        let m = healthy_measurement();
        let l = limits();
        let mut z = [0.0; CHANNELS];
        z[ch::EGT + 2] = 4.0;
        for k in 0..CALIBRATION_FRAMES {
            let out = d.update(k as f64 * 0.05, &z, &m, &l);
            assert!(!out.drift, "frame {k}");
            assert!(out.cusum < 1e-12);
            assert!(out.calibrating, "frame {k}");
        }
        assert!(!d.update(BASELINE_S, &z, &m, &l).calibrating);
    }

    /// The whole argument for a CUSUM. A shift far too small to make any single
    /// frame unusual still accumulates, and the distance test never notices it.
    #[test]
    fn a_drift_below_the_single_frame_threshold_is_still_caught() {
        let mut z = [0.0; CHANNELS];
        z[ch::EGT + 2] = 1.0;
        let mut d = calibrated();
        let out = feed(&mut d, 40, &z);

        assert!(out.drift, "cusum {}", out.cusum);
        assert_eq!(out.cusum_channel, "EGT 3");
        assert!(
            out.distance < out.distance_limit,
            "one sigma on one channel is not an outlier: {} against {}",
            out.distance,
            out.distance_limit
        );
        assert!(!out.anomaly);
    }

    /// A one-sigma shift needs 5 / (1 - 0.5) = 10 frames to reach the decision
    /// interval, and arithmetic that quietly stops matching this is a retuning
    /// nobody asked for.
    #[test]
    fn the_cusum_reaches_its_limit_when_the_slack_arithmetic_says_it_should() {
        let mut z = [0.0; CHANNELS];
        z[ch::EGT + 2] = 1.0;
        let mut d = calibrated();
        assert!(!feed(&mut d, 10, &z).drift);
        let mut d = calibrated();
        assert!(feed(&mut d, 11, &z).drift);
    }

    /// Sign is not severity. A channel falling is as much a fault as one rising,
    /// and the coked injector this build is arranged around moves EGT downward.
    #[test]
    fn the_cusum_is_two_sided() {
        let mut up = [0.0; CHANNELS];
        up[ch::EGT + 2] = 1.5;
        let mut down = [0.0; CHANNELS];
        down[ch::EGT + 2] = -1.5;

        let mut a = calibrated();
        let mut b = calibrated();
        let ra = feed(&mut a, 20, &up);
        let rb = feed(&mut b, 20, &down);
        assert!((ra.cusum - rb.cusum).abs() < 1e-12);
        assert!(rb.drift);
    }

    /// A step large enough to be an outlier has to be caught by the distance test
    /// rather than waited for, and it must persist before it is called.
    #[test]
    fn a_large_step_is_called_only_once_it_persists() {
        let mut z = [0.0; CHANNELS];
        z[ch::TORQUE] = 12.0;
        let mut d = calibrated();
        assert!(!feed(&mut d, 3, &z).anomaly);
        let mut d = calibrated();
        let out = feed(&mut d, 4, &z);
        assert!(out.anomaly, "distance {}", out.distance);
    }

    /// The comparison the demonstration rests on. The demonstration fault runs a
    /// cylinder cool, and every certificated limit that could see it is an upper
    /// bound, so the conventional monitor has nothing to trip on at all.
    #[test]
    fn the_conventional_monitor_cannot_see_a_cylinder_going_cold() {
        let l = limits();
        let mut m = healthy_measurement();
        assert!(Redline::tripped(&m, &l).is_none());

        m.egt_k[2] -= 70.0;
        m.cht_k[2] -= 9.0;
        assert!(
            Redline::tripped(&m, &l).is_none(),
            "a coked injector must not trip an over-temperature limit"
        );

        // It is a real monitor, not a stub: an over-temperature does trip it.
        m.egt_k[2] = l.egt_max_k + 1.0;
        assert_eq!(Redline::tripped(&m, &l), Some("EGT 3"));
    }

    /// Low oil pressure is the one limit on the list that is a lower bound, and
    /// getting its direction wrong would silently disable the only alarm that can
    /// catch a lubrication failure.
    #[test]
    fn the_conventional_monitor_watches_oil_pressure_from_below() {
        let l = limits();
        let mut m = healthy_measurement();
        m.oil_p_pa = l.oil_p_min_pa - 1.0;
        assert_eq!(Redline::tripped(&m, &l), Some("OIL P LOW"));
        m.oil_p_pa = l.oil_p_max_pa + 1.0;
        assert_eq!(Redline::tripped(&m, &l), Some("OIL P HIGH"));
    }

    /// Lead time is a difference between two latched timestamps, and it stays
    /// unavailable rather than becoming zero while either is missing.
    #[test]
    fn lead_time_is_none_until_both_alarms_have_happened() {
        let mut z = [0.0; CHANNELS];
        z[ch::COOLANT] = 1.0;
        let mut d = calibrated();
        let l = limits();
        let m = healthy_measurement();

        let mut out = Detection::default();
        for k in 0..40 {
            out = d.update(BASELINE_S + k as f64 * 0.05, &z, &m, &l);
        }
        assert!(out.drift);
        assert!(out.lead_time_s.is_none());

        let mut hot = m;
        hot.coolant_t_k = l.coolant_t_max_k + 1.0;
        let out = d.update(300.0, &z, &hot, &l);
        assert_eq!(out.redline_channel, "COOLANT");
        // The drift latched shortly after the 60 s calibration window closed, so
        // the lead is measured from there and not from the first frame.
        let lead = out.lead_time_s.expect("both alarms have fired");
        assert!((235.0..245.0).contains(&lead), "{lead} s");
    }

    /// The threshold has to be a chi-square quantile and not a round number
    /// somebody liked, or the false-alarm rate is unstated.
    #[test]
    fn the_distance_limit_matches_the_chi_square_quantile() {
        // Tabulated: chi-square, 22 degrees of freedom, upper 0.1% point is 48.27.
        let q = chi_square_quantile(22.0, 0.999);
        assert!((q - 48.27).abs() < 0.4, "{q}");
        assert!((Detector::new().limit - q.sqrt()).abs() < 1e-12);
    }

    /// The normal quantile underneath it, against tabulated points.
    #[test]
    fn the_normal_quantile_matches_the_table() {
        assert!((normal_quantile(0.975) - 1.959_964).abs() < 1e-6);
        assert!((normal_quantile(0.999) - 3.090_232).abs() < 1e-6);
        assert!((normal_quantile(0.001) + 3.090_232).abs() < 1e-6);
        assert!(normal_quantile(0.5).abs() < 1e-12);
        // Either side of the branch boundary at 0.97575, against values computed
        // from the error function. Both branches have to be right there, not just
        // in the middle of their own ranges.
        assert!((normal_quantile(0.975_74) - 1.972_785_551).abs() < 1e-6);
        assert!((normal_quantile(0.975_76) - 1.973_136_612).abs() < 1e-6);
    }
}
