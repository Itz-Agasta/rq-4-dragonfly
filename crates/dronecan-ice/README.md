# dronecan-ice

DroneCAN v0 codec for the reciprocating-engine message set, written rather than vendored because no mature v0 codec exists in Rust. `canadensis` implements Cyphal (UAVCAN v1), a different wire format from what ArduPilot and PX4 EFI controllers actually speak.

Scope is `uavcan.equipment.ice.reciprocating.Status` and `uavcan.equipment.ice.FuelTankStatus`. Intended to be published separately.
