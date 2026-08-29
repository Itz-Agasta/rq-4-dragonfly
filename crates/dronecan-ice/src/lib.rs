//! DroneCAN v0 codec for the reciprocating-engine message set.
//!
//! Written rather than vendored because no mature DroneCAN v0 codec exists in Rust.
//! `canadensis` implements Cyphal (UAVCAN v1), which is a different wire format from
//! the v0 that ArduPilot and PX4 EFI controllers actually speak.
//!
//! Scope is deliberately two messages, `uavcan.equipment.ice.reciprocating.Status`
//! and `uavcan.equipment.ice.FuelTankStatus`, decoded against the published DSDL.
//! Field semantics are cross-checked against PX4 `InternalCombustionEngineStatus`
//! and ArduPilot `EFI`/`EFI2`/`ECYL`, which are the real-world consumers.
//!
//! https://dronecan.github.io/Specification/7._List_of_standard_data_types/
#![forbid(unsafe_code)]
