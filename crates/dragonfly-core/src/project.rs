//! Flying the engine the twin currently believes in through the rest of a mission.
//!
//! This is the model run forward, never the filter. A filter stepped at a
//! compressed time base reads its own compression as a violent transient: the
//! covariance inflation pins at its ceiling, every sigma is multiplied, and the
//! answer is a residual band rather than a trajectory. The filter's only job here
//! is to supply the starting point, which is why a [`Seed`] is a state and a set
//! of health parameters and nothing else.
//!
//! **Measured at 2,062x real time** on the reference machine: one hour of mission
//! in 1.75 s, at the model's mandatory 200 Hz sub-step. Nothing here coarsens that
//! step to go faster. The turbocharger shaft is the stiff state and a step sized
//! to the sampling interval walks straight past its dynamics, which would produce
//! a boost trace that is smooth, plausible and wrong.

use dragonfly_sim::{mission::Profile, plant::Plant};
use engine_model::{EngineParams, Outputs, State, params::Limits};
use serde::Serialize;
use twin_core::health::{DESCRIPTORS, Health, PARAMS};

/// Samples in a projection, whatever its horizon.
///
/// Fixed rather than a rate, so the payload is bounded and a thirty hour leg costs
/// the same bytes as a five minute one. At 720 points a thousand pixel plot still
/// has more samples than it can draw.
const SAMPLES: usize = 720;

/// Seconds the controllers are settled for before sampling starts.
///
/// A [`Seed`] carries the engine's state and nothing about the controllers around
/// it, because the filter does not estimate them: the wastegate is a measured
/// input and the governor is not modelled at all. A fresh [`Plant`] starts its
/// boost controller and its propeller governor at their defaults, so without this
/// the first samples are those two converging on an operating point the engine is
/// already at, and every channel opens with a step that did not happen.
///
/// **The settled state is kept, not reset back to the seed.** Resetting reinstates
/// the step: the controller has by then found the manifold pressure its own model
/// holds, so re-imposing the filter's value only means it pulls away again, and
/// the step draws model-plant mismatch as an engine transient. The leg therefore
/// begins this many seconds in, which every horizon here rounds away.
const SETTLE_S: f64 = 20.0;

/// Shortest and longest horizon the endpoint will run.
///
/// The ceiling is wall clock rather than physics: at the measured rate eight hours
/// is about fourteen seconds of computation, and a request that takes a minute is
/// one a person has already given up on.
const MIN_HORIZON_S: f64 = 60.0;
/// Longest horizon, seconds.
const MAX_HORIZON_S: f64 = 8.0 * 3600.0;

/// Where a projection starts: the engine as the filter currently believes it to be.
#[derive(Clone, Copy, Debug)]
pub struct Seed {
    /// Mission time the estimate belongs to, seconds.
    pub t_s: f64,
    /// Engine state the filter believes in.
    pub state: State,
    /// Health parameter estimates, in [`twin_core::health::DESCRIPTORS`] order.
    pub theta: [f64; PARAMS],
}

/// One state and output pair a track reads from.
struct Sample<'a> {
    state: &'a State,
    outputs: &'a Outputs,
}

/// A projected channel, and the limit a conventional monitor would watch it against.
struct Track {
    name: &'static str,
    unit: &'static str,
    pick: fn(&Sample) -> f64,
    /// The certified limit, where the channel has one. `None` means the channel is
    /// context: it is drawn to explain the ones that do, and nothing alarms on it.
    limit: fn(&Limits) -> Option<f64>,
    /// Whether that limit is published or estimated.
    ///
    /// Carried rather than inferred from the channel name at the far end. A
    /// display that decides provenance from a string agrees with this table until
    /// a channel is renamed, and then presents an estimate as a certificate.
    published: bool,
}

/// Every projected channel.
///
/// The limit-bearing ones are the conventional threshold monitor's whole set, so a
/// projection answers the question that monitor would be asked. The oil pressure
/// **lower** bound is deliberately absent: it is published at maximum continuous
/// power, and a projection that flags it during a low-power endurance leg would be
/// reporting the operating point rather than the engine.
const TRACK: &[Track] = &[
    Track {
        name: "RPM",
        unit: "rpm",
        pick: |s| s.state.rpm(),
        limit: |l| Some(l.rpm_max),
        published: true,
    },
    Track {
        name: "POWER",
        unit: "hp",
        pick: |s| s.outputs.power_brake_w / 745.7,
        limit: |_| None,
        published: false,
    },
    Track {
        name: "BOOST",
        unit: "kPa",
        pick: |s| s.state.p_im / 1000.0,
        limit: |_| None,
        published: false,
    },
    Track {
        name: "LAMBDA",
        unit: "",
        pick: |s| s.outputs.lambda,
        limit: |_| None,
        published: false,
    },
    Track {
        name: "FUEL",
        unit: "kg/h",
        pick: |s| s.outputs.w_fuel * 3600.0,
        limit: |_| None,
        published: false,
    },
    Track {
        name: "OIL T",
        unit: "K",
        pick: |s| s.state.t_oil,
        limit: |l| Some(l.redline.oil_t_max_k),
        published: true,
    },
    Track {
        name: "OIL P",
        unit: "bar",
        pick: |s| s.outputs.p_oil / 1e5,
        limit: |l| Some(l.redline.oil_p_max_pa / 1e5),
        published: true,
    },
    Track {
        name: "COOLANT",
        unit: "K",
        pick: |s| s.state.t_coolant,
        limit: |l| Some(l.redline.coolant_t_max_k),
        published: true,
    },
    Track {
        name: "EGT 1",
        unit: "K",
        pick: |s| s.outputs.t_egt[0],
        limit: |l| Some(l.redline.egt_max_k),
        published: false,
    },
    Track {
        name: "EGT 2",
        unit: "K",
        pick: |s| s.outputs.t_egt[1],
        limit: |l| Some(l.redline.egt_max_k),
        published: false,
    },
    Track {
        name: "EGT 3",
        unit: "K",
        pick: |s| s.outputs.t_egt[2],
        limit: |l| Some(l.redline.egt_max_k),
        published: false,
    },
    Track {
        name: "EGT 4",
        unit: "K",
        pick: |s| s.outputs.t_egt[3],
        limit: |l| Some(l.redline.egt_max_k),
        published: false,
    },
    Track {
        name: "CHT 1",
        unit: "K",
        pick: |s| s.state.t_cht[0],
        limit: |l| Some(l.redline.cht_max_k),
        published: false,
    },
    Track {
        name: "CHT 2",
        unit: "K",
        pick: |s| s.state.t_cht[1],
        limit: |l| Some(l.redline.cht_max_k),
        published: false,
    },
    Track {
        name: "CHT 3",
        unit: "K",
        pick: |s| s.state.t_cht[2],
        limit: |l| Some(l.redline.cht_max_k),
        published: false,
    },
    Track {
        name: "CHT 4",
        unit: "K",
        pick: |s| s.state.t_cht[3],
        limit: |l| Some(l.redline.cht_max_k),
        published: false,
    },
];

/// One projected channel over the horizon.
#[derive(Debug, Serialize)]
pub struct Series {
    /// Short name, as it appears in a readout.
    pub name: &'static str,
    /// Engineering unit, empty for a dimensionless channel.
    pub unit: &'static str,
    /// The certified limit, or `None` for a context channel.
    pub limit: Option<f64>,
    /// Whether that limit is published or estimated. Meaningless without a limit.
    pub published: bool,
    /// One value per sample.
    pub values: Vec<f32>,
}

/// A limit this projection says will be crossed, and when.
#[derive(Debug, Serialize)]
pub struct Exceedance {
    /// Channel that crosses.
    pub channel: &'static str,
    /// The limit crossed, in the channel's own unit.
    pub limit: f64,
    /// Mission time of the first sample past it, seconds. Absolute, so it reads
    /// against the same clock every other screen shows.
    pub t_s: f64,
    /// Seconds from the seed to that crossing, which is what an operator acts on.
    pub in_s: f64,
    /// Highest value reached over the horizon.
    pub peak: f64,
    /// Whether the limit crossed is published or estimated.
    pub published: bool,
}

/// One health parameter the projection was seeded with.
///
/// The screen's whole claim rests on this: what is flown forward is the engine the
/// estimator has been watching, not a healthy one. Carrying the seed makes that
/// checkable on the screen rather than asserted by it.
#[derive(Debug, Serialize)]
pub struct SeedParam {
    /// Short name, as it appears in a readout.
    pub name: &'static str,
    /// The filter's estimate.
    pub value: f64,
    /// Value at which the subsystem no longer meets its duty. The denominator
    /// OPS's health rail shows a parameter against, so both screens read one.
    pub failure: f64,
    /// How far this parameter has travelled from nominal towards failure, 0 to 1.
    pub consumed: f64,
}

/// What one projection produced.
#[derive(Debug, Serialize)]
pub struct Projection {
    /// Profile flown, as it is named on the wire.
    pub profile: &'static str,
    /// Mission time the seed was taken at, seconds.
    pub from_t_s: f64,
    /// How far forward it was flown, seconds.
    pub horizon_s: f64,
    /// Interval between samples, seconds.
    pub sample_s: f64,
    /// Mission time of each sample, absolute.
    pub t_s: Vec<f32>,
    /// Pressure altitude at each sample, feet. The profile's own shape, drawn as
    /// context under the channels rather than as a channel.
    pub altitude_ft: Vec<f32>,
    /// Every projected channel.
    pub series: Vec<Series>,
    /// Limits crossed, soonest first. Empty means the engine holds the leg.
    pub exceedances: Vec<Exceedance>,
    /// Fuel burned over the horizon, litres.
    pub fuel_burn_l: f64,
    /// How much faster than real time this ran. Measured on the request, not
    /// quoted from a benchmark, because it is the claim the screen makes.
    pub speed_x: f64,
    /// Wall clock the projection cost, milliseconds.
    pub wall_ms: f64,
    /// The health estimate this was seeded with, worst first.
    pub seed_health: Vec<SeedParam>,
}

/// Fly `seed`'s engine through `profile` for `horizon_s` seconds.
///
/// Pure and synchronous, and it holds a core for the whole horizon. The caller
/// runs it off the async runtime.
///
/// `horizon_s` is clamped to [`MIN_HORIZON_S`]..=[`MAX_HORIZON_S`]. A non-finite
/// horizon becomes the minimum rather than propagating: `f64::clamp` returns NaN
/// for a NaN input, and every derived quantity here is a multiple of the horizon,
/// so one NaN would reach a browser as a whole projection of them. The HTTP layer
/// rejects it before this, and this stays total for any other caller.
#[must_use]
pub fn project(seed: &Seed, base: &EngineParams, profile: Profile, horizon_s: f64) -> Projection {
    let horizon_s = if horizon_s.is_nan() {
        MIN_HORIZON_S
    } else {
        horizon_s.clamp(MIN_HORIZON_S, MAX_HORIZON_S)
    };
    let sample_s = horizon_s / SAMPLES as f64;
    let health = Health::from_slice(&seed.theta);
    let params = health.apply(base);

    // The profile's clock starts at the seed, not at the mission's start: the
    // question is what the next leg does to this engine, and a profile replayed
    // from its own T+0 is that leg. Absolute mission time is carried alongside so
    // the readouts agree with every other screen.
    let mut plant = Plant::new(params, &profile.condition_at(0.0));
    plant.state = seed.state;

    let started = std::time::Instant::now();
    plant.advance(&profile.condition_at(0.0), SETTLE_S);
    plant.fuel_burnt_m3 = 0.0;

    let mut t_s = Vec::with_capacity(SAMPLES);
    let mut altitude_ft = Vec::with_capacity(SAMPLES);
    let mut values: Vec<Vec<f32>> = TRACK.iter().map(|_| Vec::with_capacity(SAMPLES)).collect();

    for i in 0..SAMPLES {
        // Offset by the settle, because those seconds were flown. Reporting a
        // crossing as sooner than it is by the length of our own warm-up would be
        // an error in the direction that matters.
        let t = SETTLE_S + (i + 1) as f64 * sample_s;
        let condition = profile.condition_at(t);
        let outputs = plant.advance(&condition, sample_s);
        let sample = Sample {
            state: &plant.state,
            outputs: &outputs,
        };
        t_s.push((seed.t_s + t) as f32);
        altitude_ft.push((condition.altitude_m / 0.3048) as f32);
        for (track, column) in TRACK.iter().zip(&mut values) {
            column.push((track.pick)(&sample) as f32);
        }
    }
    let wall = started.elapsed().as_secs_f64();

    let series: Vec<Series> = TRACK
        .iter()
        .zip(values)
        .map(|(track, values)| Series {
            name: track.name,
            unit: track.unit,
            limit: (track.limit)(&plant.params.limits),
            published: track.published,
            values,
        })
        .collect();
    let exceedances = exceedances(&series, &t_s, seed.t_s);

    Projection {
        profile: name_of(profile),
        from_t_s: seed.t_s,
        horizon_s,
        sample_s,
        t_s,
        altitude_ft,
        series,
        exceedances,
        fuel_burn_l: plant.fuel_burnt_m3 * 1000.0,
        speed_x: horizon_s / wall,
        wall_ms: wall * 1000.0,
        seed_health: seed_health(&health),
    }
}

/// The seed's health parameters, most consumed first.
fn seed_health(health: &Health) -> Vec<SeedParam> {
    let mut all: Vec<SeedParam> = (0..PARAMS)
        .map(|i| SeedParam {
            name: DESCRIPTORS[i].name,
            value: health.values[i],
            failure: DESCRIPTORS[i].failure,
            consumed: health.consumed(i),
        })
        .collect();
    all.sort_by(|a, b| b.consumed.total_cmp(&a.consumed));
    all
}

/// First crossing of each limit, soonest first.
///
/// First rather than worst, because the number an operator acts on is when the
/// leg stops being flyable, and reporting the peak instead would put the alarm at
/// whatever moment happened to be hottest.
fn exceedances(series: &[Series], t_s: &[f32], from_t_s: f64) -> Vec<Exceedance> {
    let mut found: Vec<Exceedance> = series
        .iter()
        .filter_map(|s| {
            let limit = s.limit?;
            let peak = s
                .values
                .iter()
                .fold(f64::NEG_INFINITY, |a, &v| a.max(f64::from(v)));
            let first = s.values.iter().position(|&v| f64::from(v) > limit)?;
            let at = f64::from(t_s[first]);
            Some(Exceedance {
                channel: s.name,
                limit,
                t_s: at,
                in_s: at - from_t_s,
                peak,
                published: s.published,
            })
        })
        .collect();
    found.sort_by(|a, b| a.t_s.total_cmp(&b.t_s));
    found
}

/// The profile's name on the wire.
///
/// Written out rather than derived from the `Debug` formatting, which is a
/// rendering of a Rust identifier and would change under a rename that has nothing
/// to do with the API.
fn name_of(profile: Profile) -> &'static str {
    match profile {
        Profile::Cruise => "cruise",
        Profile::HighAltitude => "high-altitude",
        Profile::Endurance => "endurance",
        Profile::HotWeather => "hot-weather",
        Profile::Transients => "transients",
    }
}

/// Parse a profile from the name the API uses.
///
/// # Errors
///
/// Returns the unrecognised name.
pub fn profile_named(name: &str) -> Result<Profile, &str> {
    match name {
        "cruise" => Ok(Profile::Cruise),
        "high-altitude" => Ok(Profile::HighAltitude),
        "endurance" => Ok(Profile::Endurance),
        "hot-weather" => Ok(Profile::HotWeather),
        "transients" => Ok(Profile::Transients),
        other => Err(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed() -> Seed {
        let params = engine_model::engines::ae330();
        let condition = Profile::Cruise.condition_at(0.0);
        let mut plant = Plant::new(params, &condition);
        // Settled, so the projection starts from an engine that is running rather
        // than one still swinging through its initial transient.
        plant.advance(&condition, 60.0);
        Seed {
            t_s: 1000.0,
            state: plant.state,
            theta: Health::nominal().values,
        }
    }

    #[test]
    fn a_healthy_cruise_crosses_nothing() {
        let base = engine_model::engines::ae330();
        let p = project(&seed(), &base, Profile::Cruise, 3600.0);
        assert!(
            p.exceedances.is_empty(),
            "a healthy engine at the rating point should hold a cruise: {:?}",
            p.exceedances
        );
        assert_eq!(p.t_s.len(), SAMPLES);
        // Absolute mission time, and the settle is part of the leg rather than
        // free: reporting a crossing sooner than it is by the length of our own
        // warm-up is an error in the direction that matters.
        let first = f64::from(p.t_s[0]);
        assert!(
            (first - (1000.0 + SETTLE_S + p.sample_s)).abs() < 1.0,
            "first sample at {first}, expected the seed plus the settle plus one sample"
        );
    }

    /// The whole claim of the screen. A cooling system the filter believes is
    /// degraded must project hotter than one it believes is well, on the same
    /// profile from the same state.
    #[test]
    fn a_degraded_radiator_projects_hotter_than_a_healthy_one() {
        let base = engine_model::engines::ae330();
        let mut sick = seed();
        sick.theta[twin_core::health::index::RADIATOR] *= 0.6;

        let well = project(&seed(), &base, Profile::HotWeather, 600.0);
        let ill = project(&sick, &base, Profile::HotWeather, 600.0);

        let coolant = |p: &Projection| {
            *p.series
                .iter()
                .find(|s| s.name == "COOLANT")
                .expect("coolant is projected")
                .values
                .last()
                .expect("samples exist")
        };
        assert!(
            coolant(&ill) > coolant(&well) + 1.0,
            "degraded {} should exceed healthy {}",
            coolant(&ill),
            coolant(&well)
        );
    }

    #[test]
    fn the_horizon_is_clamped_rather_than_refused() {
        let base = engine_model::engines::ae330();
        let p = project(&seed(), &base, Profile::Cruise, 1e9);
        assert!((p.horizon_s - MAX_HORIZON_S).abs() < f64::EPSILON);
        let p = project(&seed(), &base, Profile::Cruise, f64::INFINITY);
        assert!((p.horizon_s - MAX_HORIZON_S).abs() < f64::EPSILON);
        let p = project(&seed(), &base, Profile::Cruise, -1.0);
        assert!((p.horizon_s - MIN_HORIZON_S).abs() < f64::EPSILON);
    }

    /// `f64::clamp` passes NaN straight through, and every number in a projection
    /// is derived from the horizon, so one NaN horizon is a whole projection of
    /// them on a browser's canvas.
    #[test]
    fn a_non_finite_horizon_produces_no_non_finite_numbers() {
        let base = engine_model::engines::ae330();
        let p = project(&seed(), &base, Profile::Cruise, f64::NAN);

        assert!(p.horizon_s.is_finite(), "horizon {}", p.horizon_s);
        assert!(p.sample_s.is_finite(), "sample {}", p.sample_s);
        assert!(p.speed_x.is_finite(), "speed {}", p.speed_x);
        assert!(p.fuel_burn_l.is_finite(), "fuel {}", p.fuel_burn_l);
        assert!(
            p.t_s.iter().all(|t| t.is_finite()),
            "a sample time is not finite"
        );
        for series in &p.series {
            assert!(
                series.values.iter().all(|v| v.is_finite()),
                "{} carries a non-finite value",
                series.name
            );
        }
    }

    #[test]
    fn every_profile_name_round_trips() {
        for name in [
            "cruise",
            "high-altitude",
            "endurance",
            "hot-weather",
            "transients",
        ] {
            let profile = profile_named(name).expect("a known profile");
            assert_eq!(name_of(profile), name);
        }
        assert!(profile_named("banana").is_err());
    }
}
