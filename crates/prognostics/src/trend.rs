//! The degradation trend of each health parameter, and how well it is known.
//!
//! The filter reports where a parameter is now; prognosis needs where it is going.
//! That is a different estimation problem: the estimator tuned to track the current
//! state most accurately is not the one that extrapolates best, because it spends
//! its freedom absorbing noise.
//! Keizers, Loendersloot & Tinga, IJPHM 12(2), 2021.
//! <https://doi.org/10.36001/ijphm.2021.v12i2.2943>
//!
//! Samples are decimated to 1 Hz into a fixed ring and fitted by ordinary least
//! squares. Recursive least squares would use no memory, and was not used: its
//! normal equations are a difference of two large numbers once the time base has
//! run for an hour, and re-centring them costs more code than the ring does.

use twin_core::health::{DESCRIPTORS, PARAMS};

/// Seconds between retained samples.
const DECIMATION_S: f64 = 1.0;

/// Retained samples per parameter. Thirty minutes at 1 Hz.
///
/// Long enough that an injector coking over the hours `crate::rul` projects has a
/// slope well clear of the parameter's own random walk, and short enough that a
/// fault which starts part way through the window is not averaged against the flat
/// stretch before it for longer than it takes to fly a circuit.
const WINDOW: usize = 1800;

/// Confidence a decline must reach before it is called a degradation.
///
/// The one-sided 99% point, not the 90% the reported interval uses: the interval
/// says how well a known decline is pinned down, this decides whether there is one
/// at all, and being wrong here grounds a serviceable aircraft. At 90% one healthy
/// parameter in ten drifts far enough to be called degrading, measured over ten
/// random walks rather than argued.
const Z_DEGRADING: f64 = 2.326_347_9;

/// Samples required before a slope is reported at all.
///
/// A line through three points fits anything. Five minutes is the shortest span
/// over which the walk noise averages down enough for the slope to mean something.
const MINIMUM: usize = 300;

/// One parameter's trend.
#[derive(Clone, Copy, Debug, Default)]
pub struct Trend {
    /// Current value, from the fitted line rather than the last sample, so a single
    /// noisy frame cannot move a remaining-life estimate.
    pub value: f64,
    /// Rate of change, per second. Negative for a parameter that is degrading.
    pub slope: f64,
    /// One standard deviation of the slope, per second, including the slope a
    /// random walk of the filter's own process noise would manufacture on its own.
    pub slope_sigma: f64,
    /// One standard deviation of the fitted current value.
    pub value_sigma: f64,
    /// Seconds the fit spans. Zero until there is enough to fit.
    pub span_s: f64,
    /// Whether the fit is based on enough samples to be reported.
    pub ready: bool,
}

/// A ring of decimated samples for one parameter.
#[derive(Clone, Debug)]
struct Ring {
    t: Box<[f64; WINDOW]>,
    y: Box<[f64; WINDOW]>,
    len: usize,
    cursor: usize,
}

impl Ring {
    fn new() -> Self {
        Self {
            t: Box::new([0.0; WINDOW]),
            y: Box::new([0.0; WINDOW]),
            len: 0,
            cursor: 0,
        }
    }

    fn push(&mut self, t: f64, y: f64) {
        self.t[self.cursor] = t;
        self.y[self.cursor] = y;
        self.cursor = (self.cursor + 1) % WINDOW;
        self.len = (self.len + 1).min(WINDOW);
    }

    /// Ordinary least squares, with time centred on the window's own mean.
    ///
    /// Centring is what keeps the normal equations well conditioned: uncentred, the
    /// determinant is a difference between two numbers of order `t^2` and loses most
    /// of its significant digits after an hour of mission time.
    fn fit(&self) -> Trend {
        if self.len < MINIMUM {
            return Trend::default();
        }
        let n = self.len as f64;
        let t = &self.t[..self.len];
        let y = &self.y[..self.len];

        let t_mean = t.iter().sum::<f64>() / n;
        let y_mean = y.iter().sum::<f64>() / n;
        let mut stt = 0.0;
        let mut sty = 0.0;
        for (ti, yi) in t.iter().zip(y) {
            let dt = ti - t_mean;
            stt += dt * dt;
            sty += dt * (yi - y_mean);
        }
        if stt <= 0.0 {
            return Trend::default();
        }

        let slope = sty / stt;
        // Residual variance about the line, which is the scatter the walk and the
        // filter's own noise leave behind. Two parameters were fitted.
        let mut residual = 0.0;
        for (ti, yi) in t.iter().zip(y) {
            let predicted = slope.mul_add(ti - t_mean, y_mean);
            residual += (yi - predicted).powi(2);
        }
        let variance = residual / (n - 2.0);
        let latest = t[(self.cursor + WINDOW - 1) % WINDOW];
        // Two independent reasons the slope could be wrong, added in quadrature:
        // the scatter about the line, and the slope a parameter doing nothing but
        // the wandering the filter permits would produce anyway.
        let from_scatter = variance / stt;
        let from_walk = walk_slope_variance(t, t_mean, stt, walk_step(y));

        Trend {
            value: slope.mul_add(latest - t_mean, y_mean),
            slope,
            slope_sigma: (from_scatter + from_walk).sqrt(),
            // The fitted value's variance at the end of the window, which is where
            // the extrapolation starts and where the line is least well known.
            value_sigma: (variance * (1.0 / n + (latest - t_mean).powi(2) / stt)).sqrt(),
            span_s: latest - t[if self.len < WINDOW { 0 } else { self.cursor }],
            ready: true,
        }
    }
}

/// How far the parameter wanders between samples, trend and white noise removed.
///
/// The series is a walk plus independent noise, and only the walk part belongs in
/// the slope's error bar; white scatter is already in the least squares term. For a
/// walk increment `w` under noise `s`, `var(y[k+L] - y[k]) = L w^2 + 2 s^2`, so the
/// variogram is a line in `L` whose **slope** is the walk variance. Fitting the line
/// over many lags rather than differencing two of them is what makes it usable: one
/// pair is at the mercy of its own sampling error, which manufactures a walk large
/// enough to hide a real decline. Every lag is taken about its own mean, which is
/// what removes the trend. A negative slope means the series is anti-correlated
/// rather than a walk, and reports no walk rather than an imaginary deviation.
fn walk_step(y: &[f64]) -> f64 {
    /// Lags the variogram is fitted over. Twenty is long enough for the line to be
    /// well determined and short enough that every lag still has most of the window
    /// behind it.
    const LAGS: usize = 20;

    if y.len() < LAGS * 4 {
        return 0.0;
    }
    let spread = |lag: usize| -> f64 {
        let d: Vec<f64> = (lag..y.len()).map(|k| y[k] - y[k - lag]).collect();
        let mean = d.iter().sum::<f64>() / d.len() as f64;
        d.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (d.len() - 1) as f64
    };

    let lag_mean = (LAGS + 1) as f64 / 2.0;
    let v: Vec<f64> = (1..=LAGS).map(spread).collect();
    let v_mean = v.iter().sum::<f64>() / v.len() as f64;
    let mut sll = 0.0;
    let mut slv = 0.0;
    for (k, vk) in v.iter().enumerate() {
        let dl = (k + 1) as f64 - lag_mean;
        sll += dl * dl;
        slv += dl * (vk - v_mean);
    }
    if sll <= 0.0 {
        return 0.0;
    }
    (slv / sll).max(0.0).sqrt()
}

/// Variance a driftless random walk alone puts on the fitted slope.
///
/// Ordinary least squares assumes independent residuals. A health parameter estimate
/// is a **random walk**, whose excursions are a smooth curve rather than scatter, so
/// the textbook slope standard error is understated by orders of magnitude and a
/// healthy engine is handed a remaining life. Two corrections were tried and are
/// deliberately not here: an empirical AR(1) inflation needs a cap on the correlation
/// it will believe, so the answer depends on the cap; the filter's configured walk
/// sigmas say how fast a parameter is *permitted* to wander with no measurements at
/// all, which buries a real decline under a ceiling it never approaches.
///
/// For a walk observed at the sample times the slope variance is
/// `sigma^2 * sum_m S_m^2 / S_tt^2`, with `S_m` the tail sum of the centred times
/// from `m` on; `min(i, j) = sum_m [m<=i][m<=j]` gets it in one pass, not two.
fn walk_slope_variance(t: &[f64], t_mean: f64, stt: f64, walk_per_sample: f64) -> f64 {
    if walk_per_sample <= 0.0 || stt <= 0.0 {
        return 0.0;
    }
    let mut tail = 0.0;
    let mut total = 0.0;
    for ti in t.iter().rev() {
        tail += ti - t_mean;
        total += tail * tail;
    }
    walk_per_sample * walk_per_sample * total / (stt * stt)
}

/// Trends for every health parameter.
#[derive(Clone, Debug)]
pub struct Trends {
    rings: Vec<Ring>,
    next_sample_s: f64,
    fitted: [Trend; PARAMS],
}

impl Default for Trends {
    fn default() -> Self {
        Self::new()
    }
}

impl Trends {
    /// Trends that have seen nothing.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rings: (0..PARAMS).map(|_| Ring::new()).collect(),
            next_sample_s: f64::NEG_INFINITY,
            fitted: [Trend::default(); PARAMS],
        }
    }

    /// Offer one frame's health estimate.
    ///
    /// Cheap on the frames it discards, which is nineteen in twenty, so this sits
    /// in the ingest loop without a thread.
    pub fn observe(&mut self, t_s: f64, theta: &[f64; PARAMS]) {
        if t_s < self.next_sample_s {
            return;
        }
        self.next_sample_s = t_s + DECIMATION_S;
        for (ring, value) in self.rings.iter_mut().zip(theta) {
            ring.push(t_s, *value);
        }
        for (i, ring) in self.rings.iter().enumerate() {
            self.fitted[i] = ring.fit();
        }
    }

    /// The fitted trend of one parameter.
    ///
    /// # Panics
    /// If `i` is not a parameter index.
    #[must_use]
    pub fn get(&self, i: usize) -> &Trend {
        assert!(i < PARAMS, "no parameter {i}");
        &self.fitted[i]
    }

    /// Whether a parameter is degrading rather than wandering, at 99% confidence.
    ///
    /// Every parameter here degrades downward, so a positive slope is a machine
    /// that is improving and is reported as no trend at all rather than as a
    /// negative remaining life.
    #[must_use]
    pub fn is_degrading(&self, i: usize) -> bool {
        let t = &self.fitted[i];
        t.ready
            && t.slope < -Z_DEGRADING * t.slope_sigma
            && DESCRIPTORS[i].failure < DESCRIPTORS[i].nominal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use twin_core::health::index as th;

    /// Feed a straight line plus reproducible noise.
    fn ramp(trends: &mut Trends, seconds: usize, start: f64, per_hour: f64, noise: f64) {
        let mut seed = 0x9E37_79B9u64;
        let mut jitter = || {
            seed ^= seed >> 12;
            seed ^= seed << 25;
            seed ^= seed >> 27;
            let u = (seed.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64;
            (u - 0.5) * 2.0 * noise
        };
        for k in 0..seconds * 20 {
            let t = k as f64 * 0.05;
            let mut theta = [1.0; PARAMS];
            theta[th::INJECTOR + 2] = (t / 3600.0).mul_add(per_hour, start) + jitter();
            trends.observe(t, &theta);
        }
    }

    #[test]
    fn nothing_is_reported_until_there_is_enough_to_fit() {
        let mut short = Trends::new();
        ramp(&mut short, 200, 0.966, -0.01, 0.0);
        assert!(!short.get(th::INJECTOR + 2).ready);

        let mut long = Trends::new();
        ramp(&mut long, 400, 0.966, -0.01, 0.0);
        assert!(long.get(th::INJECTOR + 2).ready);
    }

    /// A clean ramp has to come back as the slope it was given, in the units it
    /// was given in. Getting this wrong by 3600 is the classic way a remaining
    /// life estimate becomes an hour when it should be a minute.
    #[test]
    fn a_clean_ramp_recovers_its_own_slope() {
        let mut t = Trends::new();
        ramp(&mut t, 900, 0.966, -0.02, 0.0);
        let fit = t.get(th::INJECTOR + 2);
        let per_hour = fit.slope * 3600.0;
        assert!((per_hour + 0.02).abs() < 1e-6, "{per_hour} per hour");
        assert!(fit.slope_sigma < 1e-9, "{}", fit.slope_sigma);
    }

    /// The fitted value is the line at the newest sample, not the mean of the
    /// window, or every extrapolation starts fifteen minutes in the past.
    #[test]
    fn the_reported_value_is_where_the_line_is_now() {
        let mut t = Trends::new();
        ramp(&mut t, 900, 1.0, -0.36, 0.0);
        let fit = t.get(th::INJECTOR + 2);
        // 900 s at 0.36 per hour is 0.09 consumed.
        assert!((fit.value - 0.91).abs() < 1e-3, "{}", fit.value);
    }

    /// Noise has to widen the slope's own error bar, or the interval on the
    /// remaining life is decoration.
    #[test]
    fn noise_shows_up_in_the_slopes_uncertainty() {
        let mut clean = Trends::new();
        ramp(&mut clean, 900, 0.966, -0.02, 0.0);
        let mut noisy = Trends::new();
        ramp(&mut noisy, 900, 0.966, -0.02, 0.004);

        assert!(noisy.get(th::INJECTOR + 2).slope_sigma > clean.get(th::INJECTOR + 2).slope_sigma);
        assert!(noisy.get(th::INJECTOR + 2).value_sigma > clean.get(th::INJECTOR + 2).value_sigma);
    }

    /// The failure the first live run found, pinned so it cannot come back.
    ///
    /// A health parameter is a filtered random walk, not a line plus white noise.
    /// Its excursions are smooth, so an ordinary least squares fit over half an
    /// hour finds a slope that looks overwhelmingly significant when the machine is
    /// doing nothing at all. On the bus this reported a healthy compressor
    /// efficiency as declining and gave the air path a remaining life of 4.56 hours.
    #[test]
    fn a_random_walk_is_not_a_degradation_trend() {
        let i = th::ETA_COMPRESSOR;
        fn step(seed: &mut u64) -> f64 {
            *seed ^= *seed >> 12;
            *seed ^= *seed << 25;
            *seed ^= *seed >> 27;
            let u = (seed.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64;
            (u - 0.5) * 4.0e-6
        }

        // Ten walks. Some wander downward and none of them is a fault, so any of
        // them being called degrading is a false alarm on a healthy engine.
        for run in 0..10 {
            let mut seed = 0xDEAD_BEEFu64 + run;
            let mut trends = Trends::new();
            let mut walk = 1.0;
            for k in 0..1800 * 20 {
                walk += step(&mut seed);
                let mut theta: [f64; PARAMS] = std::array::from_fn(|j| DESCRIPTORS[j].nominal);
                theta[i] = walk;
                trends.observe(k as f64 * 0.05, &theta);
            }
            assert!(trends.get(i).ready);
            assert!(
                !trends.is_degrading(i),
                "run {run}: walk ended at {walk}, slope {} per hour called significant",
                trends.get(i).slope * 3600.0
            );
        }
    }

    /// A flat parameter with noise on it must not be called a degradation. This is
    /// the false alarm that would put a maintenance advisory on a healthy engine.
    #[test]
    fn a_healthy_parameter_is_not_degrading() {
        let mut t = Trends::new();
        ramp(&mut t, 900, 0.966, 0.0, 0.004);
        assert!(t.get(th::INJECTOR + 2).ready);
        assert!(!t.is_degrading(th::INJECTOR + 2));
    }

    /// And a real one is, at a rate an injector actually cokes at.
    #[test]
    fn a_coking_injector_is_degrading() {
        let mut t = Trends::new();
        ramp(&mut t, 900, 0.90, -0.02, 0.002);
        assert!(t.is_degrading(th::INJECTOR + 2));
    }

    /// A parameter recovering is not a negative remaining life.
    #[test]
    fn an_improving_parameter_is_not_degrading() {
        let mut t = Trends::new();
        ramp(&mut t, 900, 0.90, 0.02, 0.001);
        assert!(!t.is_degrading(th::INJECTOR + 2));
    }

    /// The ring must wrap without the fit noticing, and the span must stop growing
    /// once it is full rather than reporting the whole mission.
    #[test]
    fn the_window_stops_growing_once_it_is_full() {
        let mut t = Trends::new();
        ramp(&mut t, 3000, 0.966, -0.02, 0.0);
        let fit = t.get(th::INJECTOR + 2);
        assert!(
            (fit.span_s - (WINDOW - 1) as f64 * DECIMATION_S).abs() < 2.0,
            "{}",
            fit.span_s
        );
        let per_hour = fit.slope * 3600.0;
        assert!((per_hour + 0.02).abs() < 1e-6, "{per_hour}");
    }

    /// Frames arrive at 20 Hz and only one in twenty is kept, which is what makes
    /// this cheap enough to sit in the ingest loop.
    #[test]
    fn frames_are_decimated_rather_than_all_retained() {
        let mut t = Trends::new();
        ramp(&mut t, 400, 0.966, -0.02, 0.0);
        assert!(t.get(th::INJECTOR + 2).ready);
        assert!(t.rings[0].len <= 401, "{}", t.rings[0].len);
    }
}
