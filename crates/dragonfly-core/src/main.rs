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

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::Parser;
use dronecan_ice::{AuxiliaryStatus, Message};
use engine_model::EngineParams;
use socketcan::{EmbeddedFrame, Id, tokio::CanSocket};
use tokio::sync::{broadcast, mpsc};
use twin_core::Twin;

use dragonfly_core::ingest::{Decoded, Ingest};
use dragonfly_core::project::Seed;
use dragonfly_core::record::Recorder;
use dragonfly_core::server::{self, AppState, LinkStatus};
use dragonfly_core::telemetry::{self, Fusion, SLOW_STALE_AFTER, STALE_AFTER};

/// Frames buffered per WebSocket client before it is declared lagged.
///
/// Two seconds at the engine rate. Long enough to ride out a browser stutter,
/// short enough that a client which stops reading is cut off rather than
/// accumulating telemetry nobody will look at.
const CHANNEL_DEPTH: usize = 40;

/// How often the link is re-checked while the bus is silent.
const IDLE_TICK: Duration = Duration::from_millis(100);

/// Frames queued for the recording thread before the forwarder starts dropping.
///
/// Ten seconds at the engine rate, which is far more than a flush needs and
/// still bounded: a writer that has genuinely stopped must not grow a queue
/// until the process runs out of memory.
const RECORD_QUEUE: usize = 200;

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

    /// Data type ID fault commands are published on. Must match the simulator.
    #[arg(long, default_value_t = dronecan_ice::FaultCommand::DEFAULT_DATA_TYPE_ID)]
    command_dtid: u16,

    /// Directory mission recordings are written to.
    #[arg(long, default_value = "data/missions")]
    record_dir: PathBuf,

    /// Do not record this run.
    #[arg(long)]
    no_record: bool,
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
    // Depth eight, because commands come from a person pressing a control. A
    // deeper queue would only mean holding faults that land long after the press.
    let (commands, command_rx) = mpsc::channel(8);
    let link = Arc::new(LinkStatus::default());
    let params = engine_model::engines::ae330();

    let params = Arc::new(params);
    // Written by the twin loop, read by a projection request.
    let seed = Arc::new(Mutex::new(None));

    let state = AppState {
        frames: frames.clone(),
        iface: args.iface.clone(),
        link: Arc::clone(&link),
        // Generated here rather than inside the twin so the server can answer
        // before the first CAN frame arrives; a screen must be able to draw its
        // axes on a bus that has not started yet.
        signatures: Arc::new(twin_core::Signatures::generate(&params)),
        commands,
        record_dir: args.record_dir.clone(),
        params: Arc::clone(&params),
        seed: Arc::clone(&seed),
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

    // Attached before the ingest task starts, so the first frame of the mission
    // is in the file. A recorder that misses the opening seconds cannot answer
    // the question a replay is opened to answer, which is what the engine was
    // doing before anyone noticed.
    let recording = if args.no_record {
        None
    } else {
        Some(Recording::start(
            &args.record_dir,
            params.limits.rated_power_w,
            frames.subscribe(),
        )?)
    };

    let ingest = tokio::spawn(ingest_loop(
        args.iface.clone(),
        args.aux_dtid,
        args.command_dtid,
        frames,
        link,
        params,
        seed,
        command_rx,
    ));

    tokio::select! {
        result = axum::serve(listener, app) => result.context("serving HTTP")?,
        result = ingest => result.context("ingest task")??,
        _ = tokio::signal::ctrl_c() => tracing::info!("stopping"),
    }
    // A Parquet file with no footer cannot be opened at all, so this is not
    // housekeeping: without it every recording ends as an unreadable file and
    // the replay screen has nothing to show for the mission that just flew.
    if let Some(recording) = recording {
        recording.finish();
    }
    Ok(())
}

/// A mission recording running alongside the ingest loop.
///
/// Two hops rather than one. The Parquet writer is synchronous and flushes a
/// megabyte every few thousand rows, so it lives on its own thread and cannot
/// stall the runtime; a forwarding task moves frames from the broadcast channel
/// onto it. Pushing directly from an async subscriber would block the executor
/// for the length of a flush, and being slow on a broadcast channel is how a
/// subscriber gets lagged, so the frames a replay needs would be the first
/// thing dropped.
struct Recording {
    path: PathBuf,
    forwarder: tokio::task::JoinHandle<()>,
    writer: std::thread::JoinHandle<()>,
}

impl Recording {
    /// Begin recording every frame published on `frames`.
    fn start(
        dir: &Path,
        rated_power_w: f64,
        mut frames: broadcast::Receiver<Arc<telemetry::Frame>>,
    ) -> Result<Self> {
        // Seconds since the epoch rather than a formatted date, so the name
        // sorts chronologically as a string and no date library is pulled in to
        // produce it.
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before the epoch")?
            .as_secs();
        let path = dir.join(format!("mission-{stamp}.parquet"));
        let mut recorder = Recorder::create(&path, rated_power_w)?;
        tracing::info!(path = %path.display(), "recording");

        let (tx, mut queue) = mpsc::channel::<Arc<telemetry::Frame>>(RECORD_QUEUE);
        let forwarder = tokio::spawn(async move {
            loop {
                match frames.recv().await {
                    Ok(frame) => {
                        if tx.send(frame).await.is_err() {
                            break;
                        }
                    }
                    // The writer is behind by more than the queue. Reported
                    // rather than fatal: a recording with a gap in it is worth
                    // more than no recording, and the sequence numbers in the
                    // file say exactly where the gap is.
                    Err(broadcast::error::RecvError::Lagged(missed)) => {
                        tracing::warn!(missed, "recorder fell behind");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        let writer = std::thread::spawn(move || {
            while let Some(frame) = queue.blocking_recv() {
                if let Err(error) = recorder.push(&frame) {
                    tracing::error!(%error, "recording stopped");
                    return;
                }
            }
            match recorder.finish() {
                Ok(rows) => tracing::info!(rows, "recording closed"),
                Err(error) => tracing::error!(%error, "recording could not be closed"),
            }
        });
        Ok(Self {
            path,
            forwarder,
            writer,
        })
    }

    /// Stop recording and write the footer.
    ///
    /// Aborting the forwarder is what drops the last sender, which ends the
    /// writer's loop and lets it close the file. Joining is the point of the
    /// whole method, so it is not skipped on the way out.
    fn finish(self) {
        self.forwarder.abort();
        if self.writer.join().is_err() {
            tracing::error!(path = %self.path.display(), "the recording thread panicked");
        }
    }
}

/// Read the bus forever, fusing what arrives into frames.
///
/// Reconnects with a backoff rather than exiting: on a real airframe the CAN
/// interface can go down and come back, and a health monitor that dies with it
/// is the one thing that must not happen.
#[allow(clippy::too_many_arguments)]
async fn ingest_loop(
    iface: String,
    auxiliary_data_type_id: u16,
    command_data_type_id: u16,
    frames: broadcast::Sender<Arc<telemetry::Frame>>,
    link: Arc<LinkStatus>,
    params: Arc<EngineParams>,
    seed: Arc<Mutex<Option<Seed>>>,
    mut commands: mpsc::Receiver<server::Command>,
) -> Result<()> {
    // The twin is built per connection rather than per process. A bus that has
    // been down for seconds has left the estimate describing an engine that has
    // since moved, and re-seeding from the first frame back costs one millisecond.
    let mut backoff = Duration::from_millis(250);
    loop {
        // The twin is rebuilt per connection, so an estimate never survives a
        // dropout and the seed taken from it must not either. Cleared before
        // the socket opens, which covers both a pump that returned and an
        // interface that will not open at all.
        *seed.lock().unwrap_or_else(PoisonError::into_inner) = None;

        match CanSocket::open(&iface) {
            Ok(socket) => {
                backoff = Duration::from_millis(250);
                if let Err(error) = pump(
                    &socket,
                    auxiliary_data_type_id,
                    command_data_type_id,
                    &frames,
                    &link,
                    &params,
                    &seed,
                    &mut commands,
                )
                .await
                {
                    link.down();
                    tracing::warn!(%error, "CAN read failed, reopening");
                }
            }
            Err(error) => {
                link.down();
                tracing::warn!(%error, iface = %iface, "cannot open interface, is `just can` done?");
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(5));
    }
}

#[allow(clippy::too_many_arguments)]
async fn pump(
    socket: &CanSocket,
    auxiliary_data_type_id: u16,
    command_data_type_id: u16,
    frames: &broadcast::Sender<Arc<telemetry::Frame>>,
    link: &LinkStatus,
    params: &EngineParams,
    seed: &Mutex<Option<Seed>>,
    commands: &mut mpsc::Receiver<server::Command>,
) -> Result<()> {
    let mut ingest = Ingest::new(auxiliary_data_type_id);
    let mut fusion = Fusion::new();
    let mut twin = Twin::new(params.clone());
    // Degradation trends live here rather than inside the twin because
    // `prognostics` depends on `twin-core` and not the other way round: the filter
    // has no business knowing that anyone is extrapolating it.
    let mut trends = prognostics::Trends::new();
    let mut logged = Instant::now();
    let mut transfer_ids = dronecan_ice::TransferIdMap::new();

    loop {
        // A timeout rather than a plain read, so a silent bus still produces
        // frames. Without this the last frame a client saw stays on screen
        // looking live, which is exactly the failure the stale flag exists for.
        // Commands are drained before the read rather than raced against it in a
        // select: the read has a timeout that makes it return on a silent bus, so
        // racing them would mean a command waiting up to that timeout behind a
        // bus that is already quiet.
        while let Ok((command, ack)) = commands.try_recv() {
            // A caller that has stopped waiting has already been told this did not
            // land, so sending it would inject a fault at a moment nobody chose.
            // That is what happens to anything queued while the bus was away.
            if ack.is_closed() {
                tracing::warn!(
                    sequence = command.sequence,
                    "fault command dropped, nobody waiting"
                );
                continue;
            }
            send_command(socket, command_data_type_id, &mut transfer_ids, command).await?;
            let _ = ack.send(());
        }

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
            // `link_ok` covers only the engine controller, and a silent auxiliary
            // or air-data node would feed the estimator the same frozen reading
            // every frame, which reads as an engine that stopped where it stopped
            // talking. Power is left out on purpose: bus voltage is not a filter
            // channel, so a dead voltmeter costs the one health index that
            // `Frame::measurement` blanks, not the whole twin.
            let measurement_fresh = frame.link_ok
                && frame.ages.auxiliary_ms < STALE_AFTER.as_millis() as u64
                && frame.ages.air_data_ms < SLOW_STALE_AFTER.as_millis() as u64;

            if measurement_fresh {
                match twin.update(&frame.measurement(params.limits.rated_power_w)) {
                    // Attached only to the frame it was computed from. Carrying
                    // the last estimate forward would present an old diagnosis
                    // as synchronised with a measurement the twin never saw.
                    Ok(Some(output)) => {
                        trends.observe(frame.t_s, &output.theta);
                        // Attached to every frame even though it changes once a
                        // second, because a client that reconnects mid-mission
                        // would otherwise wait up to a second with nothing to
                        // draw in the remaining-life panel.
                        frame.prognosis = Some(prognostics::evaluate(&trends));
                        frame.twin = Some(output.clone());
                    }
                    Ok(None) => {}
                    Err(error) => tracing::warn!(%error, "twin lost its estimate, re-seeding"),
                }
            }

            // The seed is an estimate the twin holds now, taken from a
            // measurement that is currently fresh, or it is nothing.
            //
            // Outside the freshness gate above on purpose, because **a dropped
            // interface does not error the socket**: the read times out, the loop
            // keeps producing frames, and the twin keeps its last estimate
            // indefinitely. Gating only on the filter having discarded its own
            // therefore misses the ordinary way a bus goes quiet, measured as a
            // 200 on a live run. A projection seeded from an engine that stopped
            // being observed is a value rendered without consulting its age.
            //
            // An unusable measurement on a live link is not staleness: the filter
            // still holds its estimate and simply did not advance, so the seed
            // stands until the link itself goes.
            {
                let mut held = seed.lock().unwrap_or_else(PoisonError::into_inner);
                if !measurement_fresh || !twin.is_seeded() {
                    *held = None;
                } else if let (Some(output), Some(state)) = (frame.twin.as_ref(), twin.state()) {
                    *held = Some(Seed {
                        t_s: frame.t_s,
                        state,
                        theta: output.theta,
                    });
                }
            }
            // Read back out of the frame rather than off the twin, so the link
            // status cannot claim a lock for a frame that carries no estimate.
            let locked = frame.twin.as_ref().is_some_and(|t| t.locked);
            link.record(frame.seq, frame.link_ok, locked);
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

/// Publish one fault command onto the bus, several times.
///
/// **Repeated rather than acknowledged.** CAN gives no delivery guarantee and this
/// system implements no service protocol, so the same command goes out three times
/// carrying one sequence number; the simulator applies the first copy it sees and
/// ignores the rest. Losing two of three still lands the command, and a duplicate
/// costs nothing.
async fn send_command(
    socket: &CanSocket,
    data_type_id: u16,
    transfer_ids: &mut dronecan_ice::TransferIdMap,
    command: dronecan_ice::FaultCommand,
) -> Result<()> {
    use dronecan_ice::{Message, MessageId, NODE_GROUND_STATION, frames_for};

    /// Copies of each command. Three is two more than a lossless bus needs.
    const COPIES: usize = 3;

    let id = MessageId {
        data_type_id,
        source_node_id: NODE_GROUND_STATION,
        priority: 20,
    };
    let payload = command.encode();

    for _ in 0..COPIES {
        let transfer_id = transfer_ids.next(NODE_GROUND_STATION, data_type_id);
        for frame in frames_for(
            id,
            dronecan_ice::FaultCommand::SIGNATURE,
            &payload,
            transfer_id,
        ) {
            let raw = socketcan::ExtendedId::new(frame.id()).context("command id over 29 bits")?;
            let out = socketcan::CanFrame::new(raw, frame.data()).context("command payload")?;
            socket
                .write_frame(out)
                .await
                .context("writing a fault command")?;
        }
    }
    tracing::info!(
        sequence = command.sequence,
        kind = command.kind,
        cylinder = command.cylinder,
        "fault command published"
    );
    Ok(())
}
