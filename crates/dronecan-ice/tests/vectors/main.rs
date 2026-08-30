//! Byte-for-byte comparison against frames the reference implementation produced.
//!
//! Round-trip tests cannot catch a bit offset that the encoder and the decoder
//! get wrong the same way, and that is the failure mode this codec is most
//! exposed to: it would look correct in every test here and produce garbage on a
//! real bus. These vectors come from pydronecan, so they fail on exactly that.
//!
//! Regenerate with `just golden` after changing the message set.

mod golden;

use dronecan_ice::messages::{CircuitStatus, IndicatedAirspeed, StaticPressure, StaticTemperature};
use dronecan_ice::{
    FuelTankStatus, Message, MessageId, Reassembler, ReciprocatingStatus, frames_for,
};
use std::time::Duration;

/// Encode `message`, split it, and check every frame against the reference.
fn assert_frames<M: Message>(message: &M, transfer_id: u8, expected: &[(u32, &[u8])]) {
    let id = MessageId {
        priority: golden::PRIORITY,
        data_type_id: M::DEFAULT_DATA_TYPE_ID,
        source_node_id: golden::NODE_ID,
    };
    let frames = frames_for(id, M::SIGNATURE, &message.encode(), transfer_id);

    assert_eq!(frames.len(), expected.len(), "{}: frame count", M::NAME);
    for (i, (frame, (want_id, want_data))) in frames.iter().zip(expected).enumerate() {
        assert_eq!(frame.id(), *want_id, "{}: frame {i} identifier", M::NAME);
        assert_eq!(frame.data(), *want_data, "{}: frame {i} data", M::NAME);
    }
}

/// Feed the reference frames through the reassembler and decode the result.
fn decode_reference<M: Message>(frames: &[(u32, &[u8])]) -> M {
    let mut rx = Reassembler::new();
    let mut transfer = None;
    for (id, data) in frames {
        let frame = dronecan_ice::Frame::new(*id, data).expect("reference frame fits");
        transfer = rx.push(&frame, Duration::ZERO).or(transfer);
    }
    let transfer = transfer.expect("the reference frames form one transfer");
    M::from_transfer(&transfer).expect("reference transfer decodes")
}

fn reciprocating_status() -> ReciprocatingStatus {
    use dronecan_ice::messages::flags;
    let cylinders = [
        (412.0, 843.0, 1.68),
        (408.0, 839.0, 1.71),
        (419.0, 887.0, 1.89),
        (406.0, 841.0, 1.70),
    ];
    ReciprocatingStatus {
        state: dronecan_ice::EngineState::Running,
        flags: flags::TEMPERATURE_SUPPORTED | flags::OIL_PRESSURE_SUPPORTED,
        engine_load_percent: 34,
        engine_speed_rpm: 3720,
        spark_dwell_time_ms: f32::NAN,
        atmospheric_pressure_kpa: 42.07,
        intake_manifold_pressure_kpa: 118.0,
        intake_manifold_temperature: 318.5,
        coolant_temperature: 361.0,
        oil_pressure: 420.0,
        oil_temperature: 358.0,
        fuel_pressure: 180_000.0,
        fuel_consumption_rate_cm3pm: 255.0,
        estimated_consumed_fuel_volume_cm3: 65_500.0,
        throttle_position_percent: 34,
        ecu_index: 0,
        spark_plug_usage: 0,
        cylinder_status: cylinders
            .iter()
            .enumerate()
            .map(|(i, &(cht, egt, lambda))| dronecan_ice::CylinderStatus {
                ignition_timing_deg: f32::NAN,
                injection_time_ms: 0.125f32.mul_add(i as f32, 1.5),
                cylinder_head_temperature: cht,
                exhaust_gas_temperature: egt,
                lambda_coefficient: lambda,
            })
            .collect(),
    }
}

/// Eleven frames, a transfer checksum, and a tail array with no length prefix.
/// If any one of those is wrong this assertion is the thing that says so.
#[test]
fn reciprocating_status_matches_the_reference() {
    assert_frames(
        &reciprocating_status(),
        golden::RECIPROCATING_STATUS_TRANSFER_ID,
        golden::RECIPROCATING_STATUS,
    );

    let back: ReciprocatingStatus = decode_reference(golden::RECIPROCATING_STATUS);
    assert_eq!(back.engine_speed_rpm, 3720);
    assert_eq!(back.cylinder_status.len(), 4);
    assert!(back.spark_dwell_time_ms.is_nan());
    // Above the binary16 range, and every field in the set is `saturated`, so
    // the reference clamps it to the largest finite half rather than to infinity.
    assert_eq!(back.fuel_pressure, dronecan_ice::bits::F16_MAX);
    assert!((back.cylinder_status[2].exhaust_gas_temperature - 887.0).abs() < 1.0);
}

#[test]
fn fuel_tank_status_matches_the_reference() {
    let message = FuelTankStatus {
        available_fuel_volume_percent: 63,
        available_fuel_volume_cm3: 189_000.0,
        fuel_consumption_rate_cm3pm: 255.0,
        fuel_temperature: 291.0,
        fuel_tank_id: 0,
    };
    assert_frames(
        &message,
        golden::FUEL_TANK_STATUS_TRANSFER_ID,
        golden::FUEL_TANK_STATUS,
    );
    let back: FuelTankStatus = decode_reference(golden::FUEL_TANK_STATUS);
    assert_eq!(back.available_fuel_volume_percent, 63);
    assert_eq!(back.available_fuel_volume_cm3, 189_000.0);
}

#[test]
fn single_frame_messages_match_the_reference() {
    assert_frames(
        &StaticPressure {
            static_pressure: 42_070.0,
            static_pressure_variance: 4.0,
        },
        golden::STATIC_PRESSURE_TRANSFER_ID,
        golden::STATIC_PRESSURE,
    );
    assert_frames(
        &StaticTemperature {
            static_temperature: 242.15,
            static_temperature_variance: 0.25,
        },
        golden::STATIC_TEMPERATURE_TRANSFER_ID,
        golden::STATIC_TEMPERATURE,
    );
    assert_frames(
        &IndicatedAirspeed {
            indicated_airspeed: 40.125,
            indicated_airspeed_variance: 0.5,
        },
        golden::INDICATED_AIRSPEED_TRANSFER_ID,
        golden::INDICATED_AIRSPEED,
    );
    assert_frames(
        &CircuitStatus {
            circuit_id: 1,
            voltage: 27.8,
            current: 42.0,
            error_flags: 0,
        },
        golden::CIRCUIT_STATUS_TRANSFER_ID,
        golden::CIRCUIT_STATUS,
    );

    let bus: CircuitStatus = decode_reference(golden::CIRCUIT_STATUS);
    assert!((bus.voltage - 27.8).abs() < 0.02, "{}", bus.voltage);
}
