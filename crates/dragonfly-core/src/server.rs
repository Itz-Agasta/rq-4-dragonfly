//! The HTTP and WebSocket surface.
//!
//! Four things: a WebSocket carrying telemetry, a health endpoint, the fault
//! signature matrix, and the frontend bundle. Serving the bundle from the same
//! process is what makes the demo one binary and a browser rather than a stack of
//! services, and it is what an air-gapped ground station needs anyway.
//!
//! Telemetry goes out as MessagePack rather than JSON. At 20 Hz with fifty
//! fields, JSON spends most of its bytes on field names that never change, and
//! the browser spends real time parsing them.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use axum::extract::State;
use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use dronecan_ice::FaultCommand;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use tokio::sync::{broadcast, mpsc};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

use crate::telemetry::Frame;
use twin_core::channels::TABLE;
use twin_core::health::{DESCRIPTORS, PARAMS};
use twin_core::signature::{HYPOTHESES, Signatures, catalogue};

/// What the ingest loop publishes about itself for the health endpoint.
///
/// Two atomics rather than a peek at the broadcast channel: a receiver created
/// inside a request starts at the channel's tail and sees nothing, so reading
/// the link that way reports a dead bus on a live one.
#[derive(Debug, Default)]
pub struct LinkStatus {
    /// Sequence number of the most recent frame. Zero before the first one.
    pub last_seq: AtomicU64,
    /// Whether the engine has been heard from recently.
    pub link_ok: AtomicBool,
    /// Whether the twin's residual has been small enough for long enough.
    pub twin_locked: AtomicBool,
}

impl LinkStatus {
    /// Record a frame that has just been published.
    pub fn record(&self, seq: u64, link_ok: bool, twin_locked: bool) {
        self.last_seq.store(seq, Ordering::Relaxed);
        self.link_ok.store(link_ok, Ordering::Relaxed);
        self.twin_locked.store(twin_locked, Ordering::Relaxed);
    }

    /// Record that there is no socket on the bus.
    ///
    /// **Every path that leaves the pump calls this**, including one about to
    /// reopen at once: `POST /api/fault` reads `link_ok` to decide whether a
    /// command has anywhere to go, and a link latched true through a reconnect
    /// accepts a fault the next pump discards. The sequence number is left alone
    /// because it is history, and a dropped link does not un-see its frames.
    pub fn down(&self) {
        self.link_ok.store(false, Ordering::Relaxed);
        self.twin_locked.store(false, Ordering::Relaxed);
    }
}

/// Shared state every handler sees.
#[derive(Clone)]
pub struct AppState {
    /// Telemetry fan-out. Subscribers that fall behind are lagged, not blocked.
    pub frames: broadcast::Sender<Arc<Frame>>,
    /// Interface being ingested, reported by the health endpoint.
    pub iface: String,
    /// Last thing the ingest loop said about the bus.
    pub link: Arc<LinkStatus>,
    /// The fault signature matrix, generated once at startup from the engine
    /// parameters. Served rather than streamed: it is constant for a given
    /// engine, and 198 floats twenty times a second would be the largest thing
    /// on the wire and never change.
    pub signatures: Arc<Signatures>,
    /// Fault commands on their way to the bus.
    ///
    /// A channel rather than a socket handle, because the CAN socket is owned by
    /// the ingest loop and is reopened whenever the bus drops. An HTTP handler
    /// that held the socket would have to be told about every reconnection; this
    /// way the command is queued and whichever socket is current sends it.
    ///
    /// Bounded and small: commands are pressed by a person, so nothing legitimate
    /// fills it. **Space in the queue is not a route to the bus**, since the
    /// ingest loop drains it only while its socket is open. See [`LinkStatus::down`].
    pub commands: mpsc::Sender<FaultCommand>,
}

/// What `POST /api/fault` accepts.
///
/// Deliberately not a typed enum on the wire: the kind is the same `uint8` the
/// DroneCAN message carries, so the browser, the HTTP body and the CAN payload
/// all name a fault the same way and there is no third vocabulary to keep in
/// step.
#[derive(Debug, serde::Deserialize)]
pub struct FaultRequest {
    /// `FaultKind` discriminant. 0 clears every injected fault.
    pub kind: u8,
    /// Cylinder, 1 to 4. Ignored by faults that are not per-cylinder.
    #[serde(default)]
    pub cylinder: u8,
    /// Fault-specific magnitude.
    #[serde(default)]
    pub severity: f32,
    /// Seconds the fault takes to reach that magnitude.
    #[serde(default)]
    pub ramp_s: f32,
}

/// What `/api/signatures` answers.
///
/// The matrix a diagnosis screen draws its rows from, with the axes named so the
/// client does not have to keep its own copy of either and drift from it.
#[derive(Debug, Serialize)]
struct Matrix {
    /// Hypothesis names, in row order. Index 0 is the null hypothesis.
    hypotheses: [&'static str; HYPOTHESES],
    /// Subsystem each hypothesis belongs to, indexing the health rail.
    subsystem: [usize; HYPOTHESES],
    /// Whether each hypothesis is a fault in the instrument rather than the
    /// engine, which is the one case where the right action is to distrust the
    /// measurement rather than open the machine.
    instrument: [bool; HYPOTHESES],
    /// Channel names, in column order.
    channels: Vec<&'static str>,
    /// One unit-length row per hypothesis: the direction that fault pushes the
    /// residual, in each channel's own standard deviations.
    rows: Vec<Vec<f64>>,
    /// The health parameters, in `theta` order, with the two values a
    /// degradation trajectory needs an axis between.
    parameters: [Parameter; PARAMS],
}

/// One health parameter's identity and the span a trajectory is drawn across.
///
/// Sent rather than mirrored in the client because `prognostics::rul` projects to
/// exactly this `failure` value: a chart that drew its own threshold would be free
/// to disagree with the number beside it about when the engine stops flying.
#[derive(Debug, Serialize)]
struct Parameter {
    /// Short name, matching `Rul::driver`.
    name: &'static str,
    /// Value on a healthy engine.
    nominal: f64,
    /// Value at which the subsystem no longer meets its duty.
    failure: f64,
}

/// What `/api/health` answers.
#[derive(Debug, Serialize)]
struct Health {
    /// Build version.
    version: &'static str,
    /// CAN interface being ingested.
    iface: String,
    /// WebSocket clients currently attached.
    clients: usize,
    /// Sequence number of the most recent frame, zero before the first one.
    last_seq: u64,
    /// Whether the engine has been heard from recently.
    link_ok: bool,
    /// Whether the twin is synchronised.
    twin_locked: bool,
}

/// Build the router.
///
/// `ui_dir` is served as a fallback so client-side routes resolve to the app
/// shell rather than to a 404.
pub fn router(state: AppState, ui_dir: PathBuf) -> Router {
    let index = ui_dir.join("index.html");
    let files = ServeDir::new(&ui_dir).fallback(ServeFile::new(index));

    Router::new()
        .route("/ws", get(websocket))
        .route("/api/health", get(health))
        .route("/api/signatures", get(signatures))
        .route("/api/fault", post(fault))
        .fallback_service(files)
        // The bundle is same-origin in the kiosk, but the Vite dev server is not,
        // and D7 onward is developed against it.
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    Json(Health {
        version: env!("CARGO_PKG_VERSION"),
        iface: state.iface.clone(),
        clients: state.frames.receiver_count(),
        last_seq: state.link.last_seq.load(Ordering::Relaxed),
        link_ok: state.link.link_ok.load(Ordering::Relaxed),
        twin_locked: state.link.twin_locked.load(Ordering::Relaxed),
    })
}

async fn signatures(State(state): State<AppState>) -> impl IntoResponse {
    let rows = catalogue();
    Json(Matrix {
        hypotheses: std::array::from_fn(|h| rows[h].name),
        subsystem: std::array::from_fn(|h| rows[h].subsystem),
        instrument: std::array::from_fn(|h| rows[h].instrument),
        channels: TABLE.iter().map(|c| c.name).collect(),
        rows: (0..HYPOTHESES)
            .map(|h| state.signatures.row(h).to_vec())
            .collect(),
        parameters: std::array::from_fn(|i| Parameter {
            name: DESCRIPTORS[i].name,
            nominal: DESCRIPTORS[i].nominal,
            failure: DESCRIPTORS[i].failure,
        }),
    })
}

/// Ask the simulator to inject a fault.
///
/// **This commands a simulator and must never be wired to flight software.** It
/// exists so a demonstration can break the engine while somebody is watching, and
/// it travels over the same CAN link as the telemetry rather than over a side
/// channel, so the bus is visibly bidirectional and there is one transport to
/// explain.
///
/// The sequence number is assigned here rather than by the caller, so a browser
/// that retries an HTTP request cannot make the simulator treat one press as two
/// different commands.
async fn fault(
    State(state): State<AppState>,
    Json(request): Json<FaultRequest>,
) -> impl IntoResponse {
    // `try_send` succeeding says the queue has space, never that anything is
    // listening, so with the bus down eight commands queue and all land when it
    // returns. Checked before a sequence is spent: the simulator de-duplicates on it.
    if !state.link.link_ok.load(Ordering::Relaxed) {
        return (StatusCode::SERVICE_UNAVAILABLE, "no route to the bus").into_response();
    }
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed) as u8;
    let command = FaultCommand {
        sequence,
        kind: request.kind,
        cylinder: request.cylinder,
        severity: request.severity,
        ramp_s: request.ramp_s,
    };
    match state.commands.try_send(command) {
        Ok(()) => (StatusCode::ACCEPTED, Json(sequence)).into_response(),
        // Full with the link up means commands are arriving faster than the bus
        // takes them, which a person pressing a control cannot do.
        Err(error) => {
            tracing::warn!(%error, "fault command not queued");
            (StatusCode::SERVICE_UNAVAILABLE, "no route to the bus").into_response()
        }
    }
}

async fn websocket(upgrade: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| stream_frames(socket, state))
}

async fn stream_frames(socket: WebSocket, state: AppState) {
    let mut rx = state.frames.subscribe();
    // Split so the socket can be read and written at once. A connection that is
    // only ever written to never processes an incoming ping, so it never answers
    // one, and any client that pings on a timer disconnects when its timeout
    // expires. Browsers do not ping, which is why this only shows up against a
    // recorder or a test harness. The reader below discards what it receives;
    // its purpose is to let the protocol layer see the ping and queue the pong,
    // which the next telemetry frame flushes.
    let (mut sink, mut stream) = socket.split();
    let drain = tokio::spawn(async move { while stream.next().await.is_some() {} });
    tracing::info!("websocket client attached");

    loop {
        match rx.recv().await {
            Ok(frame) => {
                let Ok(payload) = rmp_serde::to_vec_named(&*frame) else {
                    // Serialising a frame cannot fail for these types; if it
                    // somehow does, dropping the client is better than looping.
                    break;
                };
                if sink.send(WsMessage::Binary(payload.into())).await.is_err() {
                    break;
                }
            }
            // The client fell behind. Frames are dropped rather than queued: at
            // 20 Hz a backlog is stale by the time it arrives, and the sequence
            // numbers already tell the client that it missed some.
            Err(broadcast::error::RecvError::Lagged(missed)) => {
                tracing::warn!(missed, "websocket client fell behind");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
    drain.abort();
    tracing::info!("websocket client detached");
}
