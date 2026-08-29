//! The daemon: CAN ingest, twin loop, storage, and the HTTP/WebSocket API.
//!
//! One binary owns the whole runtime path. Telemetry is decoded from SocketCAN, fed
//! through the twin, recorded to Parquet, and pushed to the UI as MessagePack over a
//! WebSocket at 20 Hz. The frontend bundle is served from here too, so the demo is a
//! single process plus a browser in kiosk mode.

fn main() {
    todo!("D6: ingest vcan0, serve WS on :8787")
}
