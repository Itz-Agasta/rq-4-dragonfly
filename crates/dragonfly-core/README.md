# dragonfly-core

The daemon. SocketCAN ingest, the twin loop, Parquet mission recording, ONNX inference, and an axum HTTP/WebSocket API on :8787 that also serves the built UI.

One binary owns the entire runtime path, so the demo is a single process plus a browser.
