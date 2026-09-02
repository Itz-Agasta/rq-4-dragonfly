//! DroneCAN v0 codec for the reciprocating-engine message set.
//!
//! Written rather than vendored because no mature DroneCAN v0 codec exists in
//! Rust. `canadensis` implements Cyphal, formerly UAVCAN v1, which is a different
//! wire format from the v0 that ArduPilot and PX4 engine control units speak.
//!
//! # Scope
//!
//! Enough of the protocol to carry a piston engine's telemetry and no more:
//! message transfers only, no services, no anonymous transfers, no dynamic node
//! ID allocation, one interface rather than a redundant pair.
//!
//! | Message | Data type ID |
//! | --- | --- |
//! | [`ReciprocatingStatus`] | 1120 |
//! | [`FuelTankStatus`] | 1129 |
//! | [`StaticPressure`] | 1028 |
//! | [`StaticTemperature`] | 1029 |
//! | [`IndicatedAirspeed`] | 1021 |
//! | [`CircuitStatus`] | 1091 |
//! | [`AuxiliaryStatus`] | 20950, vendor range, reconfigurable |
//!
//! # No I/O and no clock
//!
//! Frames go in and out as plain values, and reassembly takes the current time as
//! an argument. Nothing here opens a socket, which is what lets the same code
//! serve a live bus, a replayed log and a unit test.
//!
//! # Example
//!
//! ```
//! use dronecan_ice::{Message, MessageId, ReciprocatingStatus, Reassembler, frames_for};
//! use std::time::Duration;
//!
//! let status = ReciprocatingStatus {
//!     engine_speed_rpm: 3720,
//!     coolant_temperature: 361.0,
//!     ..ReciprocatingStatus::default()
//! };
//! let id = MessageId {
//!     priority: 16,
//!     data_type_id: ReciprocatingStatus::DEFAULT_DATA_TYPE_ID,
//!     source_node_id: 42,
//! };
//!
//! let frames = frames_for(id, ReciprocatingStatus::SIGNATURE, &status.encode(), 0);
//!
//! let mut rx = Reassembler::new();
//! let transfer = frames
//!     .iter()
//!     .filter_map(|f| rx.push(f, Duration::ZERO))
//!     .next()
//!     .expect("the last frame completes the transfer");
//! assert_eq!(ReciprocatingStatus::from_transfer(&transfer)?.engine_speed_rpm, 3720);
//! # Ok::<(), dronecan_ice::DecodeError>(())
//! ```
//!
//! <https://dronecan.github.io/Specification/>
#![forbid(unsafe_code)]

pub mod bits;
pub mod crc;
pub mod messages;
pub mod signature;
pub mod transfer;

pub use messages::{
    AuxiliaryStatus, CircuitStatus, CylinderStatus, DecodeError, EngineState, FaultCommand,
    FaultKind, FuelTankStatus, IndicatedAirspeed, Message, NODE_GROUND_STATION,
    ReciprocatingStatus, StaticPressure, StaticTemperature,
};
pub use transfer::{Frame, MessageId, Reassembler, Transfer, TransferIdMap, frames_for};
