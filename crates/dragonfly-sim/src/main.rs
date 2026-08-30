//! The stand-in for a real engine: simulation plus fault injection onto a CAN bus.
//!
//! Runs as a separate process and writes real DroneCAN frames to `vcan0`, so
//! `dragonfly-core` reads the same bytes it would read from an engine controller.
//! Swapping to hardware is a change of interface name, and it means the CAN rig
//! can be built in parallel without touching this code.
//!
//! Faults are physically grounded parameter perturbations inside `engine-model`,
//! never signal-level hacks. That is what makes residual-based diagnosis actually
//! work instead of merely appearing to. None are injectable yet;
//! `cylinder.injector_scale` is the parameter the first of them will act on.
//!
//! The whole run is a function of the seed and the profile. Nothing reads the
//! wall clock except the pacing, so two runs with the same arguments put the
//! same bytes on the bus, which is what makes a recorded mission replayable and
//! a reported problem reproducible.

mod fault;
mod mission;
mod plant;
mod publish;
mod sensors;

use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use dronecan_ice::{AuxiliaryStatus, Message};
use socketcan::{EmbeddedFrame, ExtendedId, tokio::CanSocket};
use tokio::time::{MissedTickBehavior, interval};

use fault::InjectorCoking;
use mission::Profile;
use plant::Plant;
use publish::Publisher;
use sensors::Sensors;

/// Telemetry rate, Hz. The engine integrates at 200 Hz underneath this.
const PUBLISH_HZ: u64 = 20;

#[derive(Parser, Debug)]
#[command(about = "Simulated aero piston engine publishing DroneCAN to a CAN interface")]
struct Args {
    /// CAN interface to publish on. `vcan0` for the virtual bus, `can0` for real hardware.
    #[arg(long, default_value = "vcan0")]
    iface: String,

    /// Mission profile to fly.
    #[arg(long, value_enum, default_value_t = Profile::Cruise)]
    profile: Profile,

    /// Simulated seconds per wall-clock second.
    #[arg(long, default_value_t = 1.0)]
    speed: f64,

    /// Seed for sensor noise and vibration. The same seed gives the same run.
    #[arg(long, default_value_t = 1)]
    seed: u64,

    /// Data type ID for the vendor message. The vendor range has no registry, so
    /// this must be changeable if it collides with something else on the bus.
    #[arg(long, default_value_t = AuxiliaryStatus::DEFAULT_DATA_TYPE_ID)]
    aux_dtid: u16,

    /// Stop after this many simulated seconds. Runs forever if unset.
    #[arg(long)]
    duration: Option<f64>,

    /// Cylinder to coke the injector on, 1 to 4. Healthy if unset.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=4))]
    fault_cylinder: Option<u8>,

    /// Simulated seconds before the injector fault begins.
    #[arg(long, default_value_t = 90.0)]
    fault_onset: f64,

    /// Simulated seconds the fault takes to reach its settled severity.
    #[arg(long, default_value_t = 240.0)]
    fault_ramp: f64,

    /// Injector flow scale the fault settles at, as a fraction of nominal.
    #[arg(long, default_value_t = 0.84)]
    fault_scale: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dragonfly_sim=info".into()),
        )
        .init();

    let args = Args::parse();
    let socket = CanSocket::open(&args.iface)
        .with_context(|| format!("opening CAN interface {}, is `just can` done?", args.iface))?;

    let params = engine_model::engines::ae330();
    let engine_name = params.name.clone();
    let mut condition = args.profile.condition_at(0.0);
    let mut plant = Plant::new(params, &condition);
    let mut sensors = Sensors::new(args.seed);
    let injector_fault = args.fault_cylinder.map(|c| InjectorCoking {
        cylinder: usize::from(c - 1),
        onset_s: args.fault_onset,
        ramp_s: args.fault_ramp,
        final_scale: args.fault_scale,
    });
    let mut publisher = Publisher::new(args.aux_dtid);

    tracing::info!(
        engine = %engine_name,
        iface = %args.iface,
        profile = ?args.profile,
        speed = args.speed,
        seed = args.seed,
        fault = ?injector_fault.map(|f| (f.cylinder + 1, f.onset_s, f.final_scale)),
        "publishing at {PUBLISH_HZ} Hz"
    );

    let wall_period = Duration::from_secs_f64(1.0 / PUBLISH_HZ as f64 / args.speed.max(1e-6));
    let mut ticker = interval(wall_period);
    // Delay rather than Burst: falling behind must slow the mission down, not
    // fire a backlog of ticks that publishes several frames with the same
    // timestamp and makes the receiver think the bus stuttered.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let dt = 1.0 / PUBLISH_HZ as f64;
    let mut t_s = 0.0f64;
    let mut published = 0u64;
    let mut ticks = 0u64;

    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = tokio::signal::ctrl_c() => {
                tracing::info!(frames = published, "stopping");
                return Ok(());
            }
        }

        condition = args.profile.condition_at(t_s);
        // Applied before the step, so the engine integrates with the degraded
        // parameter rather than one step behind it.
        if let Some(f) = injector_fault {
            plant.params.cylinder.injector_scale[f.cylinder] = f.scale_at(t_s);
        }
        let outputs = plant.advance(&condition, dt);
        let reading = sensors.sample(&plant.state, &outputs, dt);

        for frame in publisher.frames(&plant, &condition, &outputs, &reading, PUBLISH_HZ) {
            let id = ExtendedId::new(frame.id()).context("frame identifier exceeds 29 bits")?;
            let out = socketcan::CanFrame::new(id, frame.data())
                .context("frame payload exceeds 8 bytes")?;
            socket
                .write_frame(out)
                .await
                .with_context(|| format!("writing to {}", args.iface))?;
            published += 1;
        }

        if ticks.is_multiple_of(PUBLISH_HZ * 60) {
            tracing::info!(
                t_s = format!("{t_s:.1}"),
                alt_ft = format!("{:.0}", condition.altitude_m / 0.3048),
                rpm = format!("{:.0}", reading.rpm),
                map_hpa = format!("{:.0}", reading.map_pa / 100.0),
                power_hp = format!("{:.0}", outputs.power_brake_w / 745.7),
                egt_k = format!("{:.0}", reading.egt_k[0]),
                turbo_rpm = format!("{:.0}", reading.turbo_rpm),
                "telemetry"
            );
        }

        ticks += 1;
        t_s += dt;
        if args.duration.is_some_and(|limit| t_s >= limit) {
            tracing::info!(frames = published, "profile complete");
            return Ok(());
        }
    }
}
