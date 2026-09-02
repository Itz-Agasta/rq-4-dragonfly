//! Generate a recorded mission offline, on mission time rather than wall time.
//!
//! Degradation runs over hours and a demonstration runs over minutes, so a
//! remaining life fitted to the shipped two-minute ramp is arithmetically right
//! and operationally meaningless. `engine-model` is pure, so the whole pipeline
//! runs far faster than real time and the twin sees a realistic decline.
//!
//! # A compressed time base is not a compressed wall clock
//!
//! `dragonfly-sim --speed 20` keeps its own mission clock correct, but the daemon
//! stamps frames from `Instant::now()`, so the filter is handed twenty seconds of
//! change labelled as one and reads a steady cruise as a violent transient.
//! [`generate`] never consults the wall clock: `Fusion` is driven with mission
//! time, so the filter integrates the `dt` it would in flight.
//!
//! Plant, instruments, DroneCAN encode, decode, fusion, filter and trends are the
//! shipping modules in the order the daemon calls them. Only the socket and the
//! clock are absent.
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use dragonfly_core::ingest::{Decoded, Ingest};
use dragonfly_core::record::Recorder;
use dragonfly_core::telemetry::Fusion;
use dragonfly_sim::fault::Faults;
use dragonfly_sim::mission::Profile;
use dragonfly_sim::plant::Plant;
use dragonfly_sim::publish::Publisher;
use dragonfly_sim::sensors::Sensors;
use twin_core::Twin;
use twin_core::health::PARAMS;

/// Telemetry rate, Hz.
///
/// The rate the engine controller publishes at, which is what a recording has to
/// match for a replay to run at the speed the mission was flown.
pub const PUBLISH_HZ: u64 = 20;

/// What to fly, and what to break while flying it.
#[derive(Debug)]
pub struct Config {
    /// Where to write the Parquet recording. `None` runs the mission without
    /// recording it, which is what the measurements in the tests want.
    pub out: Option<PathBuf>,
    /// Mission profile.
    pub profile: Profile,
    /// Mission length, hours.
    pub hours: f64,
    /// Instrument noise seed, so a run reproduces exactly.
    pub seed: u64,
    /// Data type ID the vendor message uses.
    pub aux_dtid: u16,
    /// Faults injected, on the simulator's own severity curves.
    pub faults: Faults,
}

/// The state of the engine and the estimate at the end of a generated mission.
#[derive(Clone, Debug)]
pub struct Summary {
    /// Frames recorded.
    pub rows: u64,
    /// Mission time at the last frame, s.
    pub t_s: f64,
    /// Wall seconds the generation took.
    pub wall_s: f64,
    /// Whether the twin held its lock at the end.
    pub locked: bool,
    /// Health parameter estimates at the last frame.
    pub theta: [f64; PARAMS],
    /// Remaining life of each parameter, hours. `None` where nothing is
    /// declining, which is not zero and must never be shown as zero.
    pub rul_hours: [Option<f64>; PARAMS],
    /// Lower bound of each, hours. The number a dispatch decision is taken on.
    pub rul_p10: [Option<f64>; PARAMS],
    /// Upper bound of each, hours.
    pub rul_p90: [Option<f64>; PARAMS],
    /// Seconds the slope was fitted over, per parameter.
    pub fit_span_s: [f64; PARAMS],
}

/// Fly the mission, recording it if `config.out` says where.
///
/// Runs as fast as the machine allows. Measured at 34x real time on the
/// reference machine, which is the filter's cost rather than the model's: a
/// projection wanting the 500x the model alone can do must run the model and not
/// this.
pub fn generate(config: &Config) -> Result<Summary> {
    let base = engine_model::engines::ae330();
    let rated_power_w = base.limits.rated_power_w;
    let mut condition = config.profile.condition_at(0.0);
    let mut plant = Plant::new(base.clone(), &condition);
    let mut sensors = Sensors::new(config.seed, config.faults);
    let mut publisher = Publisher::new(config.aux_dtid);
    let mut ingest = Ingest::new(config.aux_dtid);
    let mut fusion = Fusion::new();
    let mut twin = Twin::new(base.clone());
    let mut trends = prognostics::Trends::new();
    let mut recorder = config
        .out
        .as_ref()
        .map(|path| Recorder::create(path, rated_power_w))
        .transpose()?;

    // `Fusion` measures its time base from the instant it was built, so handing
    // it `origin + mission_time` puts the mission clock on every frame rather
    // than the wall clock. Nothing here ever reads the real time.
    let origin = Instant::now();
    let wall_started = Instant::now();
    let dt = 1.0 / PUBLISH_HZ as f64;
    let ticks = (config.hours * 3600.0 / dt).round() as u64;

    let mut summary = Summary {
        rows: 0,
        t_s: 0.0,
        wall_s: 0.0,
        locked: false,
        theta: [f64::NAN; PARAMS],
        rul_hours: [None; PARAMS],
        rul_p10: [None; PARAMS],
        rul_p90: [None; PARAMS],
        fit_span_s: [0.0; PARAMS],
    };
    let mut t_s = 0.0f64;

    for tick in 0..ticks {
        condition = config.profile.condition_at(t_s);
        config.faults.apply(&mut plant.params, &base, t_s);
        let outputs = plant.advance(&condition, dt);
        let reading = sensors.sample(&plant.params, &plant.state, &outputs, t_s, dt);

        let now = origin + Duration::from_secs_f64(t_s);
        for frame in publisher.frames(&plant, &condition, &outputs, &reading, PUBLISH_HZ) {
            if let Some(decoded) = ingest.accept(frame.id(), frame.data()) {
                match decoded {
                    Decoded::Engine(status) => fusion.engine(now, *status),
                    Decoded::Auxiliary(aux) => fusion.auxiliary(now, aux),
                    Decoded::Fuel(fuel) => fusion.fuel(now, fuel),
                    Decoded::Pressure(p) => fusion.pressure(now, p),
                    Decoded::Temperature(t) => fusion.temperature(now, t),
                    Decoded::Airspeed(a) => fusion.airspeed(now, a),
                    Decoded::Bus(b) => fusion.bus(now, b),
                }
            }
        }

        if let Some(mut frame) = fusion.frame(now) {
            match twin.update(&frame.measurement(rated_power_w)) {
                Ok(Some(output)) => {
                    trends.observe(frame.t_s, &output.theta);
                    let prognosis = prognostics::evaluate(&trends);
                    summary.locked = output.locked;
                    summary.theta = output.theta;
                    for i in 0..PARAMS {
                        summary.rul_hours[i] = prognosis.parameter[i].hours;
                        summary.rul_p10[i] = prognosis.parameter[i].p10;
                        summary.rul_p90[i] = prognosis.parameter[i].p90;
                        summary.fit_span_s[i] = prognosis.parameter[i].fit_span_s;
                    }
                    frame.prognosis = Some(prognosis);
                    frame.twin = Some(output.clone());
                }
                Ok(None) => {}
                Err(error) => tracing::warn!(%error, t_s, "twin lost its estimate, re-seeding"),
            }
            if let Some(recorder) = recorder.as_mut() {
                recorder.push(&frame)?;
            }
            summary.rows += 1;
            summary.t_s = frame.t_s;
        }

        if tick.is_multiple_of(PUBLISH_HZ * 600) {
            tracing::info!(
                t_h = format!("{:.2}", t_s / 3600.0),
                alt_ft = format!("{:.0}", condition.altitude_m / 0.3048),
                egt_3_k = format!("{:.0}", reading.egt_k[2]),
                "generating"
            );
        }
        t_s += dt;
    }

    if let Some(recorder) = recorder {
        recorder.finish()?;
    }
    summary.wall_s = wall_started.elapsed().as_secs_f64();
    Ok(summary)
}
