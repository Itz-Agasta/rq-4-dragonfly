//! The stand-in for a real engine: simulation plus fault injection onto a CAN bus.
//!
//! Runs as a separate process and writes real DroneCAN frames to `vcan0`, so
//! `dragonfly-core` reads the same bytes it would read from an engine controller.
//! Swapping to hardware is a change of interface name, and it means the CAN rig
//! can be built in parallel without touching this code.
//!
//! Faults are physically grounded parameter perturbations inside `engine-model`,
//! never signal-level hacks. That is what makes residual-based diagnosis actually
//! work instead of merely appearing to. Injector coking acts on
//! `cylinder.injector_scale`; see `fault`.
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

use anyhow::{Context, Result, ensure};
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

/// Reject fault arguments the injector model cannot honour.
///
/// clap has ranged parsers for integers only, and these are the arguments that
/// otherwise fail quietly rather than loudly: a negative ramp clamps the growth
/// to zero and a scale of 1.0 or more never removes any fuel, so either one
/// publishes a healthy engine for a run whose command line says a fault was
/// injected. Non-finite values are worse still, propagating through the state
/// integration until every channel on the bus is NaN.
fn validate_fault(args: &Args) -> Result<()> {
    if args.fault_cylinder.is_none() {
        return Ok(());
    }
    ensure!(
        args.fault_onset.is_finite() && args.fault_onset >= 0.0,
        "--fault-onset is a simulated time in seconds, so it must be finite and not negative, got {}",
        args.fault_onset
    );
    ensure!(
        args.fault_ramp.is_finite() && args.fault_ramp >= 0.0,
        "--fault-ramp is a duration in seconds, so it must be finite and not negative, got {}. Use 0 for a step change.",
        args.fault_ramp
    );
    ensure!(
        args.fault_scale.is_finite() && (0.0..1.0).contains(&args.fault_scale),
        "--fault-scale is the fraction of nominal injector flow the fault settles at, so it must be at least 0.0 and below 1.0 to remove any fuel at all, got {}",
        args.fault_scale
    );
    Ok(())
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
    validate_fault(&args)?;
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
        let reading = sensors.sample(&plant.params, &plant.state, &outputs, dt);

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

#[cfg(test)]
mod tests {
    use super::*;

    /// `try_parse_from`, not `parse_from`: the latter exits the process on a
    /// parse error, which takes the whole test binary with it.
    fn args(extra: &[&str]) -> Args {
        let mut argv = vec!["dragonfly-sim"];
        argv.extend_from_slice(extra);
        Args::try_parse_from(argv).expect("arguments should parse")
    }

    /// The failure this guards is silent, not loud: `scale_at` clamps a negative
    /// ramp to zero progress and publishes a nominal engine, so the run looks
    /// healthy while the command line says otherwise.
    #[test]
    fn a_ramp_that_would_disable_the_fault_is_rejected() {
        assert!(validate_fault(&args(&["--fault-cylinder", "3", "--fault-ramp=-1"])).is_err());
    }

    #[test]
    fn a_scale_that_would_add_fuel_is_rejected() {
        assert!(validate_fault(&args(&["--fault-cylinder", "3", "--fault-scale", "1.2"])).is_err());
    }

    #[test]
    fn non_finite_is_rejected_before_it_reaches_the_integrator() {
        assert!(validate_fault(&args(&["--fault-cylinder", "3", "--fault-scale", "nan"])).is_err());
    }

    /// The extreme of the same fault, not a different one. `--fault-scale 0` is a
    /// totally blocked injector, and the model already answers for it: `lambda`
    /// returns infinity on zero fuel and `indicated_efficiency` reads a non-finite
    /// lambda as zero equivalence ratio.
    #[test]
    fn a_totally_blocked_injector_is_allowed() {
        assert!(validate_fault(&args(&["--fault-cylinder", "3", "--fault-scale", "0"])).is_ok());
    }

    #[test]
    fn defaults_describe_the_demonstration_fault() {
        assert!(validate_fault(&args(&["--fault-cylinder", "3"])).is_ok());
    }

    /// Without a cylinder there is no fault, so the other three arguments are
    /// inert and rejecting them would fail a healthy run for no reason.
    #[test]
    fn fault_arguments_are_inert_on_a_healthy_run() {
        assert!(validate_fault(&args(&["--fault-scale", "9"])).is_ok());
    }
}
