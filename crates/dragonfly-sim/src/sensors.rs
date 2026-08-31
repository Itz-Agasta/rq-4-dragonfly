//! What sits between the physics and the wire.
//!
//! The engine model deliberately has no sensors in it: it reports the quantity,
//! not what an instrument measuring the quantity would say. The difference is
//! not cosmetic. A sheathed thermocouple in an exhaust runner lags the gas by
//! seconds, so during a fuelling step a real exhaust reading trails the physics
//! by more than the fault this system exists to detect will ever move it. A twin
//! compared against an unlagged signal would report that lag as a fault.
//!
//! Three sim-side models live here that are not engine physics at all: the
//! vibration signal, the electrical bus, and the noise. They are marked as such,
//! and each carries its own note about what it does and does not represent.
//!
//! Everything is driven by one seeded generator, so a run is reproducible from
//! its seed alone. Replay depends on that, and so does being able to reproduce a
//! reported bug.

use std::f64::consts::TAU;

use engine_model::{CYLINDERS, EngineParams, Outputs, State};

/// Exhaust thermocouple time constant, s. **estimated** for a thin sheathed K-type
/// probe in a small-bore runner, where gas velocity lifts the film coefficient far
/// above a still-air rating. George & Muthuveerappan, Journal of Aerospace Sciences
/// and Technologies 71(4), 2023. <https://doi.org/10.61653/joast.v71i4.2019.174>
const EGT_TAU_S: f64 = 2.0;

/// Cylinder head sensor time constant, s. **estimated**: a head sensor is bonded
/// to metal that is already the slow node, so its own lag barely shows.
const CHT_TAU_S: f64 = 0.5;

/// Alternator regulation set point, V. **estimated** for a 28 V bus.
const BUS_REGULATED_V: f64 = 28.2;
/// Bus source resistance, ohm. **estimated**.
const BUS_RESISTANCE_OHM: f64 = 0.013;
/// Steady airframe load, A. **estimated**.
const BUS_LOAD_A: f64 = 30.0;
/// Crankshaft speed below which the alternator cannot hold the bus, rpm.
const ALTERNATOR_CUT_IN_RPM: f64 = 1200.0;
/// Bus voltage on the battery alone, V.
const BATTERY_V: f64 = 24.8;

/// Internal vibration sample rate, Hz.
///
/// Well above the highest firing-order harmonic synthesised, so the RMS and the
/// kurtosis are properties of the signal rather than of the sampling.
const VIBRATION_RATE_HZ: f64 = 2000.0;
/// Samples retained for the statistics, half a second at the internal rate.
const VIBRATION_WINDOW: usize = 1024;

/// A reproducible generator.
///
/// xorshift64\*, chosen because the alternative is a dependency for something
/// twenty lines long and because a fixed algorithm keeps a recorded mission
/// reproducible across crate updates.
#[derive(Clone, Copy, Debug)]
pub struct Rng(u64);

impl Rng {
    /// Seed the generator. Zero is remapped, since xorshift is stuck there.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform on `[0, 1)`.
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Standard normal, by the Box-Muller transform.
    pub fn normal(&mut self) -> f64 {
        let u1 = self.uniform().max(f64::MIN_POSITIVE);
        let u2 = self.uniform();
        (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
    }
}

/// One standard deviation of measurement noise per channel.
///
/// **estimated** from the resolution and repeatability a production engine
/// controller quotes. Deliberately not zero: a twin tuned against a noiseless
/// signal reports a residual band far tighter than anything achievable on a real
/// bus, and every threshold derived from it is then wrong.
#[derive(Clone, Copy, Debug)]
pub struct NoiseLevels {
    /// Crankshaft speed, rpm.
    pub rpm: f64,
    /// Manifold pressure, Pa.
    pub pressure_pa: f64,
    /// Gas and metal temperatures, K.
    pub temperature_k: f64,
    /// Air mass flow, kg/s.
    pub mass_flow: f64,
    /// Oil gallery pressure, Pa.
    pub oil_pressure_pa: f64,
    /// Turbocharger shaft speed, rpm.
    pub turbo_rpm: f64,
    /// Bus voltage, V.
    pub bus_v: f64,
}

impl Default for NoiseLevels {
    fn default() -> Self {
        Self {
            rpm: 1.5,
            pressure_pa: 250.0,
            temperature_k: 0.8,
            mass_flow: 3.0e-4,
            oil_pressure_pa: 1500.0,
            turbo_rpm: 200.0,
            bus_v: 0.02,
        }
    }
}

/// What a set of instruments on this engine would report.
#[derive(Clone, Copy, Debug)]
pub struct Reading {
    /// Crankshaft speed, rpm.
    pub rpm: f64,
    /// Intake manifold pressure, Pa.
    pub map_pa: f64,
    /// Intake manifold temperature, K.
    pub mat_k: f64,
    /// Air mass flow, kg/s.
    pub maf_kgps: f64,
    /// Cylinder head temperature per cylinder, K.
    pub cht_k: [f64; CYLINDERS],
    /// Exhaust gas temperature per cylinder, K, with thermocouple lag applied.
    pub egt_k: [f64; CYLINDERS],
    /// Excess air ratio per cylinder.
    pub lambda: [f64; CYLINDERS],
    /// Injection duration per cylinder, ms.
    pub injection_ms: [f64; CYLINDERS],
    /// Oil gallery pressure, Pa above ambient.
    pub oil_p_pa: f64,
    /// Oil temperature, K.
    pub oil_t_k: f64,
    /// Coolant temperature, K.
    pub coolant_t_k: f64,
    /// Turbocharger shaft speed, rpm.
    pub turbo_rpm: f64,
    /// Bus voltage, V.
    pub bus_v: f64,
    /// Broadband vibration, g RMS.
    pub vib_rms_g: f64,
    /// Kurtosis of the vibration signal.
    pub vib_kurtosis: f64,
}

/// The instrument set: lag states, noise, and the two synthesised channels.
#[derive(Debug)]
pub struct Sensors {
    rng: Rng,
    noise: NoiseLevels,
    egt_lagged: [f64; CYLINDERS],
    cht_lagged: [f64; CYLINDERS],
    vibration: VibrationChannel,
    initialised: bool,
}

impl Sensors {
    /// Instruments seeded for a reproducible run.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed),
            noise: NoiseLevels::default(),
            egt_lagged: [0.0; CYLINDERS],
            cht_lagged: [0.0; CYLINDERS],
            vibration: VibrationChannel::new(Rng::new(seed ^ 0xA5A5_A5A5)),
            initialised: false,
        }
    }

    /// Sample every channel after `dt` seconds of engine time.
    pub fn sample(
        &mut self,
        params: &EngineParams,
        state: &State,
        outputs: &Outputs,
        dt: f64,
    ) -> Reading {
        if !self.initialised {
            // Start the lag states on the truth rather than at zero, or the first
            // seconds of every run show a warm-up ramp that is an artefact.
            self.egt_lagged = outputs.t_egt;
            self.cht_lagged = state.t_cht;
            self.initialised = true;
        }
        for i in 0..CYLINDERS {
            self.egt_lagged[i] = first_order(self.egt_lagged[i], outputs.t_egt[i], EGT_TAU_S, dt);
            self.cht_lagged[i] = first_order(self.cht_lagged[i], state.t_cht[i], CHT_TAU_S, dt);
        }

        let rpm = state.rpm();
        let load = fraction_of_full_load(outputs);
        self.vibration.advance(rpm, load, dt);

        Reading {
            rpm: self.jitter(rpm, self.noise.rpm),
            map_pa: self.jitter(state.p_im, self.noise.pressure_pa),
            mat_k: self.jitter(outputs.t_intake, self.noise.temperature_k),
            maf_kgps: self.jitter(outputs.w_air, self.noise.mass_flow),
            cht_k: std::array::from_fn(|i| {
                self.jitter(self.cht_lagged[i], self.noise.temperature_k)
            }),
            egt_k: std::array::from_fn(|i| {
                self.jitter(self.egt_lagged[i], self.noise.temperature_k * 2.0)
            }),
            lambda: outputs.lambda_cylinder,
            injection_ms: injection_durations(params, outputs),
            oil_p_pa: self.jitter(outputs.p_oil, self.noise.oil_pressure_pa),
            oil_t_k: self.jitter(state.t_oil, self.noise.temperature_k),
            coolant_t_k: self.jitter(state.t_coolant, self.noise.temperature_k),
            turbo_rpm: self.jitter(state.turbo_rpm(), self.noise.turbo_rpm),
            bus_v: self.jitter(bus_voltage(rpm), self.noise.bus_v),
            vib_rms_g: self.vibration.rms(),
            vib_kurtosis: self.vibration.kurtosis(),
        }
    }

    fn jitter(&mut self, value: f64, sigma: f64) -> f64 {
        sigma.mul_add(self.rng.normal(), value)
    }
}

/// Exponential approach to `target`, exact for a constant target over the step.
fn first_order(current: f64, target: f64, tau_s: f64, dt: f64) -> f64 {
    let alpha = 1.0 - (-dt / tau_s).exp();
    alpha.mul_add(target - current, current)
}

/// Injection duration per cylinder, ms.
///
/// A rate-based conversion from injected mass, not a measurement: the engine model
/// works in milligrams per cycle and the field on the bus is a duration.
///
/// Derived from the **commanded** quantity, not the delivered one. A common-rail
/// controller without per-cylinder feedback cannot know that a nozzle is passing
/// less than it is asked for, so it holds the same pulse on every cylinder and the
/// shortfall shows up downstream in exhaust temperature and excess air ratio. A
/// duration computed from the delivered mass would instead broadcast a fault
/// signature on a channel that physically cannot carry one.
fn injection_durations(p: &EngineParams, outputs: &Outputs) -> [f64; CYLINDERS] {
    let ms = outputs.u_f_mg / p.cylinder.injector_flow_g_per_s;
    [ms; CYLINDERS]
}

/// Brake power as a fraction of the rating, clamped, for scaling the vibration.
fn fraction_of_full_load(outputs: &Outputs) -> f64 {
    const RATED_W: f64 = 132_000.0;
    (outputs.power_brake_w / RATED_W).clamp(0.0, 1.2)
}

/// Bus voltage from an engine-driven alternator with a regulator.
///
/// A sim-side model, not engine physics: nothing in the engine model knows about
/// the electrical system. Above the cut-in speed the regulator holds its set
/// point less the drop across the source resistance; below it the battery
/// carries the load alone.
fn bus_voltage(rpm: f64) -> f64 {
    if rpm >= ALTERNATOR_CUT_IN_RPM {
        BUS_LOAD_A.mul_add(-BUS_RESISTANCE_OHM, BUS_REGULATED_V)
    } else {
        BATTERY_V
    }
}

/// A synthesised vibration signal and its statistics.
///
/// Firing-order harmonics on a broadband floor. A four-cylinder four-stroke
/// fires twice per revolution, so the fundamental is twice crankshaft speed and
/// the harmonics above it are what a combustion fault modulates.
///
/// It carries no bearing defect frequencies and no envelope spectrum: those come
/// with the fault library, and until they do this channel can show a load trend
/// but cannot show a bearing.
#[derive(Debug)]
struct VibrationChannel {
    rng: Rng,
    phase: f64,
    samples: Vec<f64>,
    cursor: usize,
}

impl VibrationChannel {
    fn new(rng: Rng) -> Self {
        Self {
            rng,
            phase: 0.0,
            samples: vec![0.0; VIBRATION_WINDOW],
            cursor: 0,
        }
    }

    fn advance(&mut self, rpm: f64, load: f64, dt: f64) {
        // Amplitudes are **estimated**: 0.8 g at idle rising to about 2.4 g at
        // the rating, which is the order of magnitude quoted for a four-cylinder
        // diesel measured on the crankcase.
        let amplitude = 0.8f64.mul_add(1.0, 1.6 * load);
        let fundamental = 2.0 * rpm / 60.0;
        let count = ((dt * VIBRATION_RATE_HZ).round() as usize).min(VIBRATION_WINDOW);

        for _ in 0..count {
            self.phase = (self.phase + fundamental / VIBRATION_RATE_HZ).fract();
            let theta = TAU * self.phase;
            let tone = 0.30f64.mul_add(
                (3.0 * theta).sin(),
                0.55f64.mul_add((2.0 * theta).sin(), theta.sin()),
            );
            let sample = amplitude.mul_add(tone, 0.35 * self.rng.normal());
            self.samples[self.cursor] = sample;
            self.cursor = (self.cursor + 1) % VIBRATION_WINDOW;
        }
    }

    fn rms(&self) -> f64 {
        let n = self.samples.len() as f64;
        (self.samples.iter().map(|s| s * s).sum::<f64>() / n).sqrt()
    }

    /// Kurtosis, the classical early indicator: it rises for an impulsive defect
    /// while RMS is still flat, because a few large excursions move a fourth
    /// moment long before they move a second one.
    fn kurtosis(&self) -> f64 {
        let n = self.samples.len() as f64;
        let mean = self.samples.iter().sum::<f64>() / n;
        let m2 = self.samples.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / n;
        if m2 <= f64::EPSILON {
            return 0.0;
        }
        let m4 = self.samples.iter().map(|s| (s - mean).powi(4)).sum::<f64>() / n;
        m4 / (m2 * m2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generator_is_reproducible_from_its_seed() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        let mut c = Rng::new(8);
        assert_eq!(a.next_u64(), b.next_u64());
        assert_ne!(a.next_u64(), c.next_u64());
    }

    #[test]
    fn the_normal_generator_has_the_right_moments() {
        let mut rng = Rng::new(1);
        let n = 20_000;
        let samples: Vec<f64> = (0..n).map(|_| rng.normal()).collect();
        let mean = samples.iter().sum::<f64>() / f64::from(n);
        let var = samples.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / f64::from(n);
        assert!(mean.abs() < 0.05, "mean {mean}");
        assert!((var - 1.0).abs() < 0.05, "variance {var}");
    }

    /// The lag is the whole reason this module exists, so it is asserted rather
    /// than assumed: a step in gas temperature must not appear instantly.
    #[test]
    fn the_exhaust_thermocouple_lags_the_gas() {
        let step_from = 640.0;
        let step_to = 840.0;
        let after_one_tau = first_order(step_from, step_to, EGT_TAU_S, EGT_TAU_S);
        let expected = (step_to - step_from).mul_add(1.0 - (-1.0f64).exp(), step_from);
        assert!((after_one_tau - expected).abs() < 1e-9);

        // One telemetry frame in, the reading has barely moved.
        let after_a_frame = first_order(step_from, step_to, EGT_TAU_S, 0.05);
        assert!(after_a_frame - step_from < 6.0, "{after_a_frame} K");
    }

    /// A Gaussian has a kurtosis of 3, so a signal that is mostly tone plus a
    /// little noise must sit below that. If this ever reads 3.0 exactly the
    /// harmonics have stopped being generated.
    #[test]
    fn the_vibration_channel_produces_plausible_statistics() {
        let mut vib = VibrationChannel::new(Rng::new(3));
        for _ in 0..40 {
            vib.advance(3720.0, 0.35, 0.05);
        }
        let rms = vib.rms();
        assert!((0.5..6.0).contains(&rms), "{rms} g RMS");
        let kurtosis = vib.kurtosis();
        assert!((1.0..3.0).contains(&kurtosis), "{kurtosis}");
    }

    #[test]
    fn the_bus_falls_back_to_the_battery_below_cut_in() {
        assert!(bus_voltage(3720.0) > 27.0);
        assert!((bus_voltage(600.0) - BATTERY_V).abs() < 1e-9);
    }
}
