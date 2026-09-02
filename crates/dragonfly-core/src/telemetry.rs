//! The fused telemetry frame: one struct, all channels, one timestamp.
//!
//! Seven message types arrive from three nodes at three different rates. What
//! everything downstream wants instead is a single record per instant, so this
//! module holds the last value of every channel and emits a frame whenever the
//! engine status arrives, which is the fastest and the most important of them.
//!
//! # Staleness is a channel, not a footnote
//!
//! A trace that has frozen looks exactly like a trace that is steady. Every
//! source therefore carries the age of its last update, and the frame carries a
//! link flag. A consumer that shows a value without checking its age is showing
//! a number that may be minutes old with no indication, which on a health
//! monitor is worse than showing nothing.
//!
//! Field names follow the Parquet column names in the data plan, with the
//! per-cylinder channels as arrays rather than as `cht_1_k` through `cht_4_k`;
//! flattening happens where the Parquet is written.

use std::time::{Duration, Instant};

use dronecan_ice::{
    AuxiliaryStatus, CircuitStatus, EngineState, FuelTankStatus, IndicatedAirspeed,
    ReciprocatingStatus, StaticPressure, StaticTemperature,
};
use prognostics::Prognosis;
use serde::Serialize;
use twin_core::{Measurement, TwinOutput};

/// Number of cylinders reported. Matches the engine model.
pub const CYLINDERS: usize = 4;

/// A source is stale if nothing has arrived from it for this long.
///
/// 250 ms is five missed frames at the 20 Hz engine rate and one at the 5 Hz
/// ambient rate, which is long enough not to flicker on a scheduling hiccup and
/// short enough that an operator sees a dead bus within a glance.
pub const STALE_AFTER: Duration = Duration::from_millis(250);

/// A source publishing at the 5 Hz ambient rate is stale after this long.
///
/// [`STALE_AFTER`] is a single missed message for air data, which is what a
/// display wants and what a gate must not use: one lost ambient frame would take
/// the twin off screen. Three periods instead, and the outside air temperature
/// that holds errs by 0.06 K at the steepest climb any profile flies, against a
/// 0.8 K instrument sigma on the channel it reaches.
pub const SLOW_STALE_AFTER: Duration = Duration::from_millis(600);

/// How long ago each source last spoke, ms.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct SourceAges {
    /// Engine status, node 42.
    pub engine_ms: u64,
    /// Vendor auxiliary message, node 42.
    pub auxiliary_ms: u64,
    /// Fuel tank status, node 42.
    pub fuel_ms: u64,
    /// Air data computer, node 43: the oldest of pressure, temperature and
    /// airspeed. Three separate messages from one node do not go stale
    /// together, so this is the worst of the three rather than one standing
    /// in for all.
    pub air_data_ms: u64,
    /// Power module, node 44.
    pub power_ms: u64,
}

/// One instant of fused telemetry, with the twin's reading of it.
///
/// Everything above the `twin` field is a measurement or is derived from
/// measurements alone. The twin's block is carried in the same frame rather than
/// published separately so that a consumer cannot pair a prediction with a
/// measurement from a different instant, which is the one way a residual display
/// can lie without anything looking wrong.
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    /// Monotonic sequence number. A gap means the consumer fell behind.
    pub seq: u64,
    /// Seconds since ingest started.
    pub t_s: f64,
    /// False when nothing has arrived from the engine within [`STALE_AFTER`].
    pub link_ok: bool,
    /// Age of each source, ms.
    pub ages: SourceAges,

    /// Pressure altitude, m, from static pressure through the standard atmosphere.
    pub altitude_m: f32,
    /// Outside air temperature, K.
    pub oat_k: f32,
    /// Ambient static pressure, Pa.
    pub p_amb_pa: f32,
    /// Indicated airspeed, m/s.
    pub ias_ms: f32,
    /// Deviation from the standard atmosphere at this altitude, K.
    pub isa_deviation_k: f32,
    /// Fuelling demand, percent.
    pub throttle_pct: f32,
    /// Brake load estimate, percent of rating.
    pub load_pct: f32,

    /// Crankshaft speed, rpm.
    pub rpm: f32,
    /// Intake manifold absolute pressure, Pa.
    pub map_pa: f32,
    /// Intake manifold temperature, K.
    pub mat_k: f32,
    /// Boost, Pa. Manifold pressure less ambient, and nothing else.
    pub boost_pa: f32,
    /// Air mass flow, kg/s.
    pub maf_kgs: f32,
    /// Fuel flow, kg/h.
    pub fuel_flow_kgh: f32,
    /// Fuel flow, litres/hour, which is what a fuel gauge reads.
    pub fuel_flow_lph: f32,
    /// Mean excess air ratio.
    pub lambda: f32,
    /// Excess air ratio per cylinder.
    pub lambda_k: [f32; CYLINDERS],
    /// Cylinder head temperature per cylinder, K.
    pub cht_k: [f32; CYLINDERS],
    /// Exhaust gas temperature per cylinder, K.
    pub egt_k: [f32; CYLINDERS],
    /// Injection duration per cylinder, ms.
    pub injection_ms: [f32; CYLINDERS],
    /// Oil gallery pressure, Pa.
    pub oil_p_pa: f32,
    /// Oil temperature, K.
    pub oil_t_k: f32,
    /// Coolant temperature, K.
    pub coolant_t_k: f32,
    /// Turbocharger shaft speed, rpm.
    pub tc_rpm: f32,
    /// Bus voltage, V.
    pub bus_v: f32,
    /// Broadband vibration, g RMS.
    pub vib_rms_g: f32,
    /// Kurtosis of the vibration signal.
    pub vib_kurtosis: f32,

    /// Wastegate position, 0 shut to 1 open. Needed to run the twin open loop.
    pub wastegate: f32,
    /// Fuel remaining, percent.
    pub fuel_remaining_pct: f32,
    /// Engine state as the controller reports it.
    pub engine_state: &'static str,
    /// Raw status flag bitmask, for a consumer that wants the detail.
    pub flags: u32,

    /// What the twin makes of this instant, or `None` before it has an estimate.
    pub twin: Option<TwinOutput>,

    /// Remaining useful life per parameter and per subsystem.
    ///
    /// Separate from `twin` and updated on its own schedule: the trend it comes
    /// from is fitted once a second over half an hour, so attaching it to every
    /// 20 Hz frame would repeat one answer twenty times. `None` until the trend
    /// window has enough samples to fit, which is five minutes of flight.
    pub prognosis: Option<Prognosis>,
}

impl Frame {
    /// Lay this frame out as the twin's input.
    ///
    /// Torque is not on the bus. What is broadcast is engine load as a whole
    /// percent of the rating, so the torque handed to the twin is that percentage
    /// turned back into a power and divided by the measured speed. The rounding is
    /// declared as measurement noise on that channel rather than hidden.
    #[must_use]
    pub fn measurement(&self, rated_power_w: f64) -> Measurement {
        let omega_e = f64::from(self.rpm) * std::f64::consts::TAU / 60.0;
        let torque_nm = if omega_e > 0.0 {
            f64::from(self.load_pct) / 100.0 * rated_power_w / omega_e
        } else {
            f64::NAN
        };
        Measurement {
            t_s: self.t_s,
            p_amb_pa: f64::from(self.p_amb_pa),
            oat_k: f64::from(self.oat_k),
            ias_m_s: f64::from(self.ias_ms),
            wastegate: f64::from(self.wastegate),
            // Every cylinder is commanded the same duration; the mean is taken so a
            // single dropped cylinder status does not move the fuelling input.
            injection_ms: mean_finite(&self.injection_ms),
            rpm: f64::from(self.rpm),
            map_pa: f64::from(self.map_pa),
            mat_k: f64::from(self.mat_k),
            maf_kg_s: f64::from(self.maf_kgs),
            turbo_rpm: f64::from(self.tc_rpm),
            torque_nm,
            fuel_flow_kg_h: f64::from(self.fuel_flow_kgh),
            oil_p_pa: f64::from(self.oil_p_pa),
            oil_t_k: f64::from(self.oil_t_k),
            coolant_t_k: f64::from(self.coolant_t_k),
            egt_k: std::array::from_fn(|i| f64::from(self.egt_k[i])),
            cht_k: std::array::from_fn(|i| f64::from(self.cht_k[i])),
            lambda: std::array::from_fn(|i| f64::from(self.lambda_k[i])),
            // Blanked when the power module falls silent. Bus voltage is not a
            // filter channel, so a frozen one can only score the electrical index
            // as though it were live, and threshold monitoring cannot notice.
            bus_v: if self.ages.power_ms < SLOW_STALE_AFTER.as_millis() as u64 {
                f64::from(self.bus_v)
            } else {
                f64::NAN
            },
            vib_rms_g: f64::from(self.vib_rms_g),
            vib_kurtosis: f64::from(self.vib_kurtosis),
        }
    }
}

/// Mean of the finite entries, or `NaN` if there are none.
fn mean_finite(values: &[f32; CYLINDERS]) -> f64 {
    let (sum, count) = values
        .iter()
        .filter(|v| v.is_finite())
        .fold((0.0f64, 0u32), |(s, n), v| (s + f64::from(*v), n + 1));
    if count == 0 {
        f64::NAN
    } else {
        sum / f64::from(count)
    }
}

/// Holds the last value of every channel and builds frames from them.
#[derive(Debug)]
pub struct Fusion {
    started: Instant,
    seq: u64,
    engine: Option<(Instant, ReciprocatingStatus)>,
    auxiliary: Option<(Instant, AuxiliaryStatus)>,
    fuel: Option<(Instant, FuelTankStatus)>,
    pressure: Option<(Instant, StaticPressure)>,
    temperature: Option<(Instant, StaticTemperature)>,
    airspeed: Option<(Instant, IndicatedAirspeed)>,
    bus: Option<(Instant, CircuitStatus)>,
}

impl Default for Fusion {
    fn default() -> Self {
        Self::new()
    }
}

impl Fusion {
    /// An empty fusion state, timing from now.
    #[must_use]
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            seq: 0,
            engine: None,
            auxiliary: None,
            fuel: None,
            pressure: None,
            temperature: None,
            airspeed: None,
            bus: None,
        }
    }

    /// Record an engine status. This is what triggers a frame.
    pub fn engine(&mut self, at: Instant, status: ReciprocatingStatus) {
        self.engine = Some((at, status));
    }

    /// Record the vendor auxiliary message.
    pub fn auxiliary(&mut self, at: Instant, aux: AuxiliaryStatus) {
        self.auxiliary = Some((at, aux));
    }

    /// Record fuel tank status.
    pub fn fuel(&mut self, at: Instant, fuel: FuelTankStatus) {
        self.fuel = Some((at, fuel));
    }

    /// Record static pressure.
    pub fn pressure(&mut self, at: Instant, pressure: StaticPressure) {
        self.pressure = Some((at, pressure));
    }

    /// Record static temperature.
    pub fn temperature(&mut self, at: Instant, temperature: StaticTemperature) {
        self.temperature = Some((at, temperature));
    }

    /// Record indicated airspeed.
    pub fn airspeed(&mut self, at: Instant, airspeed: IndicatedAirspeed) {
        self.airspeed = Some((at, airspeed));
    }

    /// Record electrical bus status.
    pub fn bus(&mut self, at: Instant, bus: CircuitStatus) {
        self.bus = Some((at, bus));
    }

    /// Build a frame from everything held, or `None` before the first engine status.
    ///
    /// A frame is only worth emitting once the engine has spoken: air data alone
    /// describes the weather, not the engine.
    pub fn frame(&mut self, now: Instant) -> Option<Frame> {
        let (engine_at, status) = self.engine.as_ref()?;
        let engine_age = now.saturating_duration_since(*engine_at);

        let aux = self.auxiliary.as_ref().map(|(_, a)| *a).unwrap_or_default();
        let fuel = self.fuel.as_ref().map(|(_, f)| *f).unwrap_or_default();

        let p_amb_pa = self
            .pressure
            .as_ref()
            .map_or(status.atmospheric_pressure_kpa * 1000.0, |(_, p)| {
                p.static_pressure
            });
        let oat_k = self
            .temperature
            .as_ref()
            .map_or(f32::NAN, |(_, t)| t.static_temperature);
        let ias_ms = self
            .airspeed
            .as_ref()
            .map_or(f32::NAN, |(_, a)| a.indicated_airspeed);
        let bus_v = self.bus.as_ref().map_or(f32::NAN, |(_, b)| b.voltage);

        let altitude_m = pressure_altitude_m(p_amb_pa);
        let map_pa = status.intake_manifold_pressure_kpa * 1000.0;
        let fuel_flow_lph = status.fuel_consumption_rate_cm3pm * 60.0 / 1000.0;

        let cylinder = |f: fn(&dronecan_ice::CylinderStatus) -> f32| -> [f32; CYLINDERS] {
            std::array::from_fn(|i| status.cylinder_status.get(i).map_or(f32::NAN, f))
        };
        let lambda_k = cylinder(|c| c.lambda_coefficient);
        let (sum, count) = lambda_k
            .iter()
            .filter(|v| v.is_finite())
            .fold((0.0f32, 0u32), |(sum, count), v| (sum + v, count + 1));
        let lambda = if count == 0 {
            f32::NAN
        } else {
            sum / count as f32
        };

        self.seq += 1;
        Some(Frame {
            seq: self.seq,
            t_s: now.saturating_duration_since(self.started).as_secs_f64(),
            link_ok: engine_age < STALE_AFTER,
            ages: SourceAges {
                engine_ms: age_ms(now, Some(*engine_at)),
                auxiliary_ms: age_ms(now, self.auxiliary.as_ref().map(|(t, _)| *t)),
                fuel_ms: age_ms(now, self.fuel.as_ref().map(|(t, _)| *t)),
                air_data_ms: age_ms(now, self.pressure.as_ref().map(|(t, _)| *t))
                    .max(age_ms(now, self.temperature.as_ref().map(|(t, _)| *t)))
                    .max(age_ms(now, self.airspeed.as_ref().map(|(t, _)| *t))),
                power_ms: age_ms(now, self.bus.as_ref().map(|(t, _)| *t)),
            },

            altitude_m,
            oat_k,
            p_amb_pa,
            ias_ms,
            isa_deviation_k: isa_deviation_k(altitude_m, oat_k),
            throttle_pct: f32::from(status.throttle_position_percent),
            load_pct: f32::from(status.engine_load_percent),

            rpm: status.engine_speed_rpm as f32,
            map_pa,
            mat_k: status.intake_manifold_temperature,
            boost_pa: map_pa - p_amb_pa,
            maf_kgs: aux.mass_air_flow_kgps,
            // The message reports a volumetric rate; mass flow needs the fuel
            // density, which the engine controller does not broadcast, so this
            // uses the value the parameter file records for the fuel in the tank.
            fuel_flow_kgh: status.fuel_consumption_rate_cm3pm * 60.0e-6 * FUEL_DENSITY_KG_M3,
            fuel_flow_lph,
            lambda,
            lambda_k,
            cht_k: cylinder(|c| c.cylinder_head_temperature),
            egt_k: cylinder(|c| c.exhaust_gas_temperature),
            injection_ms: cylinder(|c| c.injection_time_ms),
            oil_p_pa: status.oil_pressure * 1000.0,
            oil_t_k: status.oil_temperature,
            coolant_t_k: status.coolant_temperature,
            tc_rpm: aux.turbocharger_speed_rpm,
            bus_v,
            vib_rms_g: aux.vibration_rms_g,
            vib_kurtosis: aux.vibration_kurtosis,

            wastegate: aux.wastegate_position,
            fuel_remaining_pct: f32::from(fuel.available_fuel_volume_percent),
            engine_state: state_name(status.state),
            flags: status.flags,
            twin: None,
            prognosis: None,
        })
    }
}

/// Density of the heavy fuel this engine burns, kg/m3. **published** for Jet A-1
/// at 15 C.
pub const FUEL_DENSITY_KG_M3: f32 = 804.0;

/// Age of a source, clamped so it survives the trip into JavaScript.
///
/// A source that has never spoken reports the ceiling rather than `u64::MAX`:
/// MessagePack carries the full 64 bits, and anything above 2^53 either becomes
/// a `BigInt` or loses precision on the other side, so every comparison against
/// it in the browser then misbehaves. The ceiling is 49 days, which is
/// indistinguishable from never for anything that reads this.
fn age_ms(now: Instant, at: Option<Instant>) -> u64 {
    const CEILING: u64 = u32::MAX as u64;
    at.map_or(CEILING, |t| {
        (now.saturating_duration_since(t).as_millis() as u64).min(CEILING)
    })
}

const fn state_name(state: EngineState) -> &'static str {
    match state {
        EngineState::Stopped => "STOPPED",
        EngineState::Starting => "STARTING",
        EngineState::Running => "RUNNING",
        EngineState::Fault => "FAULT",
    }
}

/// Pressure altitude from static pressure, inverting the troposphere layer of
/// the standard atmosphere.
///
/// Derived here rather than taken from the bus because no standard DroneCAN
/// message carries altitude: an air data computer publishes the pressure it
/// measures and leaves the atmosphere model to the consumer.
fn pressure_altitude_m(p_amb_pa: f32) -> f32 {
    use engine_model::atmosphere::{LAPSE, P_SL, T_SL};
    if !p_amb_pa.is_finite() || p_amb_pa <= 0.0 {
        return f32::NAN;
    }
    let ratio = f64::from(p_amb_pa) / P_SL;
    let t = T_SL * ratio.powf(1.0 / 5.2561);
    ((T_SL - t) / LAPSE) as f32
}

/// Deviation of the outside air temperature from the standard atmosphere.
fn isa_deviation_k(altitude_m: f32, oat_k: f32) -> f32 {
    if !altitude_m.is_finite() || !oat_k.is_finite() {
        return f32::NAN;
    }
    engine_model::atmosphere::isa_deviation(f64::from(altitude_m), f64::from(oat_k)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running_status() -> ReciprocatingStatus {
        ReciprocatingStatus {
            state: EngineState::Running,
            engine_speed_rpm: 3720,
            atmospheric_pressure_kpa: 42.07,
            intake_manifold_pressure_kpa: 118.0,
            coolant_temperature: 361.0,
            oil_pressure: 420.0,
            oil_temperature: 358.0,
            fuel_consumption_rate_cm3pm: 255.0,
            throttle_position_percent: 35,
            cylinder_status: vec![dronecan_ice::CylinderStatus::default(); CYLINDERS],
            ..ReciprocatingStatus::default()
        }
    }

    /// Boost is manifold pressure less ambient, and the standard atmosphere puts
    /// ambient at 22,400 ft at 420.7 hPa. Any other definition of boost makes
    /// every number on a display disagree with the one beside it.
    #[test]
    fn boost_is_manifold_pressure_less_ambient() {
        let mut fusion = Fusion::new();
        fusion.engine(Instant::now(), running_status());
        let frame = fusion.frame(Instant::now()).expect("a frame");
        assert!(
            (frame.boost_pa - 75_930.0).abs() < 100.0,
            "{}",
            frame.boost_pa
        );
        assert!(
            (frame.altitude_m - 6828.0).abs() < 30.0,
            "{} m",
            frame.altitude_m
        );
    }

    #[test]
    fn no_frame_before_the_engine_has_spoken() {
        let mut fusion = Fusion::new();
        fusion.pressure(
            Instant::now(),
            StaticPressure {
                static_pressure: 42_070.0,
                static_pressure_variance: 25.0,
            },
        );
        assert!(fusion.frame(Instant::now()).is_none());
    }

    /// A frozen trace looks identical to a steady one, so the flag has to move
    /// on its own once the engine stops talking.
    #[test]
    fn the_link_goes_stale_when_the_engine_stops_talking() {
        let mut fusion = Fusion::new();
        let long_ago = Instant::now() - Duration::from_secs(1);
        fusion.engine(long_ago, running_status());
        let frame = fusion.frame(Instant::now()).expect("a frame");
        assert!(!frame.link_ok);
        assert!(frame.ages.engine_ms >= 1000);
    }

    /// Pressure, temperature and airspeed are three separate messages from the
    /// air data node; one going quiet while the others keep talking must not
    /// read as fresh.
    #[test]
    fn air_data_is_only_as_fresh_as_its_stalest_message() {
        let mut fusion = Fusion::new();
        let now = Instant::now();
        let long_ago = now - Duration::from_secs(1);
        fusion.engine(now, running_status());
        fusion.pressure(
            now,
            StaticPressure {
                static_pressure: 42_070.0,
                static_pressure_variance: 25.0,
            },
        );
        fusion.temperature(
            long_ago,
            StaticTemperature {
                static_temperature: 242.15,
                static_temperature_variance: 0.25,
            },
        );
        fusion.airspeed(
            now,
            IndicatedAirspeed {
                indicated_airspeed: 40.1,
                indicated_airspeed_variance: 0.5,
            },
        );
        let frame = fusion.frame(now).expect("a frame");
        assert!(frame.ages.air_data_ms >= 1000, "{}", frame.ages.air_data_ms);
    }

    /// A frozen bus voltage would score the electrical health index as though it
    /// were live, and threshold monitoring has nothing in it to notice the value
    /// has stopped moving.
    #[test]
    fn a_stale_bus_voltage_does_not_reach_the_twin() {
        let now = Instant::now();
        let mut fusion = Fusion::new();
        let volts = CircuitStatus {
            circuit_id: 1,
            voltage: 27.8,
            current: 30.0,
            error_flags: 0,
        };

        fusion.engine(now, running_status());
        fusion.bus(now, volts);
        let fresh = fusion.frame(now).expect("a frame");
        assert!(fresh.measurement(132_000.0).bus_v.is_finite());

        fusion.bus(now - SLOW_STALE_AFTER, volts);
        let stale = fusion.frame(now).expect("a frame");
        assert_eq!(stale.bus_v, 27.8, "the frame keeps the reading and its age");
        assert!(stale.measurement(132_000.0).bus_v.is_nan());
    }

    /// `u64::MAX` does not survive MessagePack into a JavaScript number, so a
    /// source that has never reported must not be described with it.
    #[test]
    fn the_age_of_a_silent_source_stays_inside_a_javascript_number() {
        let mut fusion = Fusion::new();
        fusion.engine(Instant::now(), running_status());
        let frame = fusion.frame(Instant::now()).expect("a frame");
        assert_eq!(frame.ages.power_ms, u64::from(u32::MAX));
        assert!((frame.ages.power_ms as f64) < 2f64.powi(53));
    }

    #[test]
    fn sequence_numbers_are_monotonic() {
        let mut fusion = Fusion::new();
        fusion.engine(Instant::now(), running_status());
        let a = fusion.frame(Instant::now()).expect("a frame").seq;
        let b = fusion.frame(Instant::now()).expect("a frame").seq;
        assert_eq!(b, a + 1);
    }

    /// Every one of the twenty measured channels has to be present and finite
    /// once all three nodes have reported, or a screen built against this shows
    /// a blank cell it cannot explain.
    #[test]
    fn a_full_frame_has_no_holes() {
        let now = Instant::now();
        let mut fusion = Fusion::new();
        fusion.engine(
            now,
            ReciprocatingStatus {
                cylinder_status: vec![
                    dronecan_ice::CylinderStatus {
                        ignition_timing_deg: f32::NAN,
                        injection_time_ms: 1.5,
                        cylinder_head_temperature: 412.0,
                        exhaust_gas_temperature: 843.0,
                        lambda_coefficient: 1.68,
                    };
                    CYLINDERS
                ],
                ..running_status()
            },
        );
        fusion.auxiliary(
            now,
            AuxiliaryStatus {
                turbocharger_speed_rpm: 118_400.0,
                mass_air_flow_kgps: 0.084,
                wastegate_position: 0.2,
                vibration_rms_g: 2.4,
                vibration_kurtosis: 2.1,
            },
        );
        fusion.temperature(
            now,
            StaticTemperature {
                static_temperature: 242.15,
                static_temperature_variance: 0.25,
            },
        );
        fusion.airspeed(
            now,
            IndicatedAirspeed {
                indicated_airspeed: 40.1,
                indicated_airspeed_variance: 0.5,
            },
        );
        fusion.bus(
            now,
            CircuitStatus {
                circuit_id: 1,
                voltage: 27.8,
                current: 30.0,
                error_flags: 0,
            },
        );

        let f = fusion.frame(now).expect("a frame");
        let scalars = [
            f.rpm,
            f.map_pa,
            f.boost_pa,
            f.maf_kgs,
            f.fuel_flow_kgh,
            f.lambda,
            f.oil_p_pa,
            f.oil_t_k,
            f.coolant_t_k,
            f.tc_rpm,
            f.bus_v,
            f.vib_rms_g,
            f.vib_kurtosis,
            f.oat_k,
            f.ias_ms,
            f.altitude_m,
            f.isa_deviation_k,
            f.wastegate,
        ];
        for (i, v) in scalars.iter().enumerate() {
            assert!(v.is_finite(), "scalar {i} is {v}");
        }
        for i in 0..CYLINDERS {
            assert!(f.cht_k[i].is_finite() && f.egt_k[i].is_finite() && f.lambda_k[i].is_finite());
        }
        assert!(f.link_ok);
        assert_eq!(f.engine_state, "RUNNING");
    }
}
