//! Command line front end for [`mission_gen::generate`].

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use dragonfly_sim::fault::FaultArgs;
use dragonfly_sim::mission::Profile;
use dronecan_ice::{AuxiliaryStatus, Message};
use mission_gen::{Config, generate};
use twin_core::health::DESCRIPTORS;

#[derive(Parser, Debug)]
#[command(about = "Generate a recorded mission offline, faster than real time")]
struct Args {
    /// Where to write the Parquet recording.
    #[arg(long, default_value = "data/missions/mission.parquet")]
    out: PathBuf,

    /// Mission profile to fly.
    #[arg(long, value_enum, default_value_t = Profile::Cruise)]
    profile: Profile,

    /// Mission length, hours.
    #[arg(long, default_value_t = 1.0)]
    hours: f64,

    /// Seed for the instrument noise, so a run is reproducible.
    #[arg(long, default_value_t = 0x5EED)]
    seed: u64,

    /// Data type ID the vendor message uses.
    #[arg(long, default_value_t = AuxiliaryStatus::DEFAULT_DATA_TYPE_ID)]
    aux_dtid: u16,

    #[command(flatten)]
    faults: FaultArgs,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mission_gen=info".into()),
        )
        .init();
    let args = Args::parse();

    let config = Config {
        out: Some(args.out.clone()),
        profile: args.profile,
        hours: args.hours,
        seed: args.seed,
        aux_dtid: args.aux_dtid,
        faults: args.faults.build()?,
    };
    tracing::info!(
        profile = ?config.profile,
        hours = config.hours,
        fault = %args.faults.summary(),
        out = %args.out.display(),
        "generating"
    );

    let summary = generate(&config)?;
    tracing::info!(
        rows = summary.rows,
        wall_s = format!("{:.1}", summary.wall_s),
        speedup = format!("{:.0}x", summary.t_s / summary.wall_s.max(1e-9)),
        locked = summary.locked,
        out = %args.out.display(),
        "mission recorded"
    );
    for (i, d) in DESCRIPTORS.iter().enumerate() {
        if let Some(hours) = summary.rul_hours[i] {
            tracing::info!(
                parameter = d.name,
                value = format!("{:.4}", summary.theta[i]),
                rul_h = format!("{hours:.2}"),
                p10_h = summary.rul_p10[i].map(|v| format!("{v:.2}")),
                p90_h = summary.rul_p90[i].map(|v| format!("{v:.2}")),
                fit_span_s = format!("{:.0}", summary.fit_span_s[i]),
                "remaining life"
            );
        }
    }
    Ok(())
}
