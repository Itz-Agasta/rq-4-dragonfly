//! The daemon: CAN ingest, twin loop, storage, and the HTTP/WebSocket API.
//!
//! One binary owns the whole runtime path. Telemetry is decoded from SocketCAN,
//! fused into one frame per instant, and pushed to the UI as MessagePack over a
//! WebSocket at the rate the engine controller publishes. The frontend bundle is
//! served from here too, so the demo is a single process plus a browser.
//!
//! The twin runs inside the ingest loop rather than as a subscriber to the
//! broadcast channel, because its output belongs in the frame: a consumer that
//! received a prediction separately could pair it with a measurement from a
//! different instant, and a residual display built on that would be wrong in a
//! way nothing on screen would reveal. The Parquet recorder does attach as a
//! subscriber, because it only reads.

mod ingest;
mod server;
mod telemetry;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use dronecan_ice::{AuxiliaryStatus, Message};
use engine_model::EngineParams;
use socketcan::{EmbeddedFrame, Id, tokio::CanSocket};
use tokio::sync::broadcast;
use twin_core::Twin;

use ingest::{Decoded, Ingest};
use server::{AppState, LinkStatus};
use telemetry::{Fusion, STALE_AFTER};

/// Frames buffered per WebSocket client before it is declared lagged.
///
/// Two seconds at the engine rate. Long enough to ride out a browser stutter,
/// short enough that a client which stops reading is cut off rather than
/// accumulating telemetry nobody will look at.
const CHANNEL_DEPTH: usize = 40;

/// How often the link is re-checked while the bus is silent.
const IDLE_TICK: Duration = Duration::from_millis(100);

#[derive(Parser, Debug)]
#[command(about = "Digital twin core: DroneCAN ingest, telemetry fusion, and the operator API")]
struct Args {
    /// CAN interface to ingest. `vcan0` for the virtual bus, `can0` for real hardware.
    #[arg(long, default_value = "vcan0")]
    iface: String,

    /// Address to serve the API and the frontend on.
    #[arg(long, default_value = "127.0.0.1:8787")]
    bind: String,

    /// Directory holding the built frontend.
    #[arg(long, default_value = "ui/dist")]
    ui_dir: PathBuf,

    /// Data type ID the vendor message uses. Must match the publisher.
    #[arg(long, default_value_t = AuxiliaryStatus::DEFAULT_DATA_TYPE_ID)]
    aux_dtid: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dragonfly_core=info".into()),
        )
        .init();

    let args = Args::parse();
    let (frames, _) = broadcast::channel(CHANNEL_DEPTH);
    let link = Arc::new(LinkStatus::default());

    let state = AppState {
        frames: frames.clone(),
        iface: args.iface.clone(),
        link: Arc::clone(&link),
    };
    let app = server::router(state, args.ui_dir.clone());

    let listener = tokio::net::TcpListener::bind(&args.bind)
        .await
        .with_context(|| format!("binding {}", args.bind))?;
    tracing::info!(
        bind = %args.bind,
        iface = %args.iface,
        ui_dir = %args.ui_dir.display(),
        "serving"
    );

    let ingest = tokio::spawn(ingest_loop(args.iface.clone(), args.aux_dtid, frames, link));

    tokio::select! {
        result = axum::serve(listener, app) => result.context("serving HTTP")?,
        result = ingest => result.context("ingest task")??,
        _ = tokio::signal::ctrl_c() => tracing::info!("stopping"),
    }
    Ok(())
}

/// Read the bus forever, fusing what arrives into frames.
///
/// Reconnects with a backoff rather than exiting: on a real airframe the CAN
/// interface can go down and come back, and a health monitor that dies with it
/// is the one thing that must not happen.
async fn ingest_loop(
    iface: String,
    auxiliary_data_type_id: u16,
    frames: broadcast::Sender<Arc<telemetry::Frame>>,
    link: Arc<LinkStatus>,
) -> Result<()> {
    // The twin is built per connection rather than per process. A bus that has
    // been down for seconds has left the estimate describing an engine that has
    // since moved, and re-seeding from the first frame back costs one millisecond.
    let params = engine_model::engines::ae330();
    let mut backoff = Duration::from_millis(250);
    loop {
        match CanSocket::open(&iface) {
            Ok(socket) => {
                backoff = Duration::from_millis(250);
                if let Err(error) =
                    pump(&socket, auxiliary_data_type_id, &frames, &link, &params).await
                {
                    tracing::warn!(%error, "CAN read failed, reopening");
                }
            }
            Err(error) => {
                link.record(0, false, false);
                tracing::warn!(%error, iface = %iface, "cannot open interface, is `just can` done?");
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(5));
    }
}

async fn pump(
    socket: &CanSocket,
    auxiliary_data_type_id: u16,
    frames: &broadcast::Sender<Arc<telemetry::Frame>>,
    link: &LinkStatus,
    params: &EngineParams,
) -> Result<()> {
    let mut ingest = Ingest::new(auxiliary_data_type_id);
    let mut fusion = Fusion::new();
    let mut twin = Twin::new(params.clone());
    let mut logged = Instant::now();

    loop {
        // A timeout rather than a plain read, so a silent bus still produces
        // frames. Without this the last frame a client saw stays on screen
        // looking live, which is exactly the failure the stale flag exists for.
        let read = tokio::time::timeout(IDLE_TICK, socket.read_frame()).await;
        let now = Instant::now();

        let mut publish = false;
        if let Ok(frame) = read {
            let frame = frame.context("reading a CAN frame")?;
            let Id::Extended(id) = frame.id() else {
                continue;
            };
            if let Some(decoded) = ingest.accept(id.as_raw(), frame.data()) {
                match decoded {
                    Decoded::Engine(status) => {
                        fusion.engine(now, *status);
                        publish = true;
                    }
                    Decoded::Auxiliary(aux) => fusion.auxiliary(now, aux),
                    Decoded::Fuel(fuel) => fusion.fuel(now, fuel),
                    Decoded::Pressure(p) => fusion.pressure(now, p),
                    Decoded::Temperature(t) => fusion.temperature(now, t),
                    Decoded::Airspeed(a) => fusion.airspeed(now, a),
                    Decoded::Bus(b) => fusion.bus(now, b),
                }
            }
        } else {
            publish = true;
        }

        if publish && let Some(mut frame) = fusion.frame(now) {
            let stale = !frame.link_ok;
            let stale_ms = STALE_AFTER.as_millis() as u64;
            // `frame.link_ok` covers only the engine controller. The twin's
            // measurement also draws on the auxiliary, air-data and power
            // sources, so a node other than the engine going silent would
            // otherwise feed the estimator the same frozen reading forever,
            // which it would read as an engine that has stopped moving.
            let measurement_fresh = frame.link_ok
                && frame.ages.auxiliary_ms < stale_ms
                && frame.ages.air_data_ms < stale_ms
                && frame.ages.power_ms < stale_ms;

            if measurement_fresh {
                // Only stepped on live data. Feeding a frozen measurement to an
                // estimator makes it more confident every frame that the engine
                // is exactly where it stopped reporting from, which is the
                // opposite of what a silent bus means.
                if let Err(error) = twin.update(&frame.measurement(params.limits.rated_power_w)) {
                    tracing::warn!(%error, "twin lost its estimate, re-seeding");
                }
                // Attached only on the tick it was current for. Carrying the
                // last estimate forward on a stale frame would present an old
                // diagnosis as synchronised with a measurement it never saw.
                if twin.is_seeded() {
                    frame.twin = Some(twin.output().clone());
                }
            }
            link.record(frame.seq, frame.link_ok, twin.output().locked);
            // A send with no subscribers is not an error; the core runs whether
            // or not anyone is watching, and the recorder will attach here too.
            let _ = frames.send(Arc::new(frame));
            if stale && now.duration_since(logged) > Duration::from_secs(2) {
                tracing::warn!(
                    stale_for_ms = STALE_AFTER.as_millis() as u64,
                    counters = ?ingest.counters,
                    "no engine telemetry"
                );
                logged = now;
            }
        }
    }
}
