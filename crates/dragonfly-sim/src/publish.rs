//! Turning an operating point into DroneCAN transfers.
//!
//! Three nodes are simulated, not one, because that is how the data is actually
//! sourced on an airframe: the engine controller knows nothing about outside air
//! temperature and the air data computer knows nothing about oil pressure.
//! Publishing everything from one node ID would make the CAN seam a formality.
//!
//! | Node | Publishes | Rate |
//! | --- | --- | --- |
//! | 42, engine controller | `reciprocating.Status`, `AuxiliaryStatus`, `FuelTankStatus` | 20, 20 and 1 Hz |
//! | 43, air data computer | static pressure, static temperature, indicated airspeed | 5 Hz |
//! | 44, power module | `CircuitStatus` | 5 Hz |
//!
//! The rates are what the messages are worth rather than what the bus can carry.
//! A `Status` with four cylinders is eleven frames, so 20 Hz of it is 220 frames
//! per second before anything else is on the wire; publishing air data at the
//! same rate would triple the traffic to repeat numbers that change over minutes.

use dronecan_ice::{
    AuxiliaryStatus, CircuitStatus, EngineState, Frame, FuelTankStatus, IndicatedAirspeed, Message,
    MessageId, ReciprocatingStatus, StaticPressure, StaticTemperature, TransferIdMap, frames_for,
    messages::{CylinderStatus, flags},
};
use engine_model::{CYLINDERS, EngineParams, Outputs};

use crate::mission::Condition;
use crate::plant::Plant;
use crate::sensors::Reading;

/// Node ID of the simulated engine control unit.
pub const NODE_ENGINE: u8 = 42;
/// Node ID of the simulated air data computer.
pub const NODE_AIR_DATA: u8 = 43;
/// Node ID of the simulated power module.
pub const NODE_POWER: u8 = 44;

/// Priority for engine telemetry. 16 is the conventional nominal value.
const PRIORITY_ENGINE: u8 = 16;
/// Priority for air data and electrical, which nothing controls a loop on.
const PRIORITY_AMBIENT: u8 = 20;
/// Priority for fuel quantity, which changes over minutes.
const PRIORITY_SLOW: u8 = 24;

/// Usable fuel, m3. **estimated** for the airframe class.
const TANK_CAPACITY_M3: f64 = 0.350;

/// Head temperature above which the overheating flag is raised, K.
/// **estimated**: aluminium head, liquid cooled, above the coolant boiling point
/// at system pressure.
const CHT_OVERHEAT_K: f64 = 520.0;
/// Head temperature above which the caution flag is raised, K. **estimated**.
const CHT_CAUTION_K: f64 = 480.0;
/// Exhaust temperature above which the exhaust caution flag is raised, K.
/// **estimated** from the 700 to 850 C band a turbocharged diesel runs at.
const EGT_CAUTION_K: f64 = 1120.0;
/// Oil gallery pressure below which the low-pressure flag is raised, Pa gauge.
/// **estimated**.
const OIL_PRESSURE_LOW_PA: f64 = 1.5e5;

/// Builds the transfers for one publish tick.
#[derive(Debug)]
pub struct Publisher {
    transfer_ids: TransferIdMap,
    auxiliary_data_type_id: u16,
    tick: u64,
}

impl Publisher {
    /// A publisher whose vendor message uses `auxiliary_data_type_id`.
    #[must_use]
    pub fn new(auxiliary_data_type_id: u16) -> Self {
        Self {
            transfer_ids: TransferIdMap::new(),
            auxiliary_data_type_id,
            tick: 0,
        }
    }

    /// Every frame this tick should put on the bus.
    ///
    /// `tick_hz` is the rate [`Publisher::frames`] is called at; the slower
    /// messages are decimated from it.
    pub fn frames(
        &mut self,
        plant: &Plant,
        condition: &Condition,
        outputs: &Outputs,
        reading: &Reading,
        tick_hz: u64,
    ) -> Vec<Frame> {
        let mut out = Vec::new();
        let tick = self.tick;
        self.tick += 1;

        out.extend(self.engine_frames(plant, condition, outputs, reading));
        out.extend(self.auxiliary_frames(reading, plant.wastegate()));

        if tick.is_multiple_of((tick_hz / 5).max(1)) {
            out.extend(self.air_data_frames(condition));
            out.extend(self.power_frames(reading));
        }
        if tick.is_multiple_of(tick_hz.max(1)) {
            out.extend(self.fuel_tank_frames(plant, outputs, condition));
        }
        out
    }

    fn engine_frames(
        &mut self,
        plant: &Plant,
        condition: &Condition,
        outputs: &Outputs,
        reading: &Reading,
    ) -> Vec<Frame> {
        let cylinders = (0..CYLINDERS)
            .map(|i| CylinderStatus {
                // No spark on a compression-ignition engine, and the DSDL way to
                // say a quantity does not exist is NaN rather than zero.
                ignition_timing_deg: f32::NAN,
                injection_time_ms: reading.injection_ms[i] as f32,
                cylinder_head_temperature: reading.cht_k[i] as f32,
                exhaust_gas_temperature: reading.egt_k[i] as f32,
                lambda_coefficient: reading.lambda[i] as f32,
            })
            .collect();

        let status = ReciprocatingStatus {
            state: if reading.rpm > 500.0 {
                EngineState::Running
            } else {
                EngineState::Stopped
            },
            flags: condition_flags(reading),
            engine_load_percent: ((outputs.power_brake_w / plant.params.limits.rated_power_w
                * 100.0)
                .round() as i64)
                .clamp(0, 127) as u8,
            engine_speed_rpm: reading.rpm.max(0.0).round() as u32,
            spark_dwell_time_ms: f32::NAN,
            atmospheric_pressure_kpa: (condition.p_amb / 1000.0) as f32,
            intake_manifold_pressure_kpa: (reading.map_pa / 1000.0) as f32,
            intake_manifold_temperature: reading.mat_k as f32,
            coolant_temperature: reading.coolant_t_k as f32,
            oil_pressure: (reading.oil_p_pa / 1000.0) as f32,
            oil_temperature: reading.oil_t_k as f32,
            // The common rail is not modelled, and NaN is the specified way to
            // say a controller does not report a field. Inventing a plausible
            // rail pressure would put a number on a screen that measures nothing.
            fuel_pressure: f32::NAN,
            fuel_consumption_rate_cm3pm: fuel_rate_cm3pm(&plant.params, outputs),
            estimated_consumed_fuel_volume_cm3: (plant.fuel_burnt_m3 * 1e6) as f32,
            throttle_position_percent: ((condition.fuel_cmd * 100.0).round() as i64).clamp(0, 127)
                as u8,
            ecu_index: 0,
            spark_plug_usage: 0,
            cylinder_status: cylinders,
        };

        self.emit(
            NODE_ENGINE,
            PRIORITY_ENGINE,
            ReciprocatingStatus::DEFAULT_DATA_TYPE_ID,
            ReciprocatingStatus::SIGNATURE,
            &status.encode(),
        )
    }

    fn auxiliary_frames(&mut self, reading: &Reading, wastegate: f64) -> Vec<Frame> {
        let aux = AuxiliaryStatus {
            turbocharger_speed_rpm: reading.turbo_rpm as f32,
            mass_air_flow_kgps: reading.maf_kgps as f32,
            wastegate_position: wastegate as f32,
            vibration_rms_g: reading.vib_rms_g as f32,
            vibration_kurtosis: reading.vib_kurtosis as f32,
        };
        let dtid = self.auxiliary_data_type_id;
        self.emit(
            NODE_ENGINE,
            PRIORITY_ENGINE,
            dtid,
            AuxiliaryStatus::SIGNATURE,
            &aux.encode(),
        )
    }

    fn fuel_tank_frames(
        &mut self,
        plant: &Plant,
        outputs: &Outputs,
        condition: &Condition,
    ) -> Vec<Frame> {
        let remaining_m3 = (TANK_CAPACITY_M3 - plant.fuel_burnt_m3).max(0.0);
        let tank = FuelTankStatus {
            available_fuel_volume_percent: ((remaining_m3 / TANK_CAPACITY_M3 * 100.0).round()
                as i64)
                .clamp(0, 127) as u8,
            available_fuel_volume_cm3: (remaining_m3 * 1e6) as f32,
            fuel_consumption_rate_cm3pm: fuel_rate_cm3pm(&plant.params, outputs),
            // Wing tanks sit at outside air temperature after a few hours aloft.
            fuel_temperature: condition.oat_k as f32,
            fuel_tank_id: 0,
        };
        self.emit(
            NODE_ENGINE,
            PRIORITY_SLOW,
            FuelTankStatus::DEFAULT_DATA_TYPE_ID,
            FuelTankStatus::SIGNATURE,
            &tank.encode(),
        )
    }

    fn air_data_frames(&mut self, condition: &Condition) -> Vec<Frame> {
        let mut out = Vec::new();
        // Variances are the sensor noise this sim actually applies, not a guess:
        // a consumer weighting these against a model needs them to be true.
        out.extend(
            self.emit(
                NODE_AIR_DATA,
                PRIORITY_AMBIENT,
                StaticPressure::DEFAULT_DATA_TYPE_ID,
                StaticPressure::SIGNATURE,
                &StaticPressure {
                    static_pressure: condition.p_amb as f32,
                    static_pressure_variance: 25.0,
                }
                .encode(),
            ),
        );
        out.extend(
            self.emit(
                NODE_AIR_DATA,
                PRIORITY_AMBIENT,
                StaticTemperature::DEFAULT_DATA_TYPE_ID,
                StaticTemperature::SIGNATURE,
                &StaticTemperature {
                    static_temperature: condition.oat_k as f32,
                    static_temperature_variance: 0.25,
                }
                .encode(),
            ),
        );
        out.extend(
            self.emit(
                NODE_AIR_DATA,
                PRIORITY_AMBIENT,
                IndicatedAirspeed::DEFAULT_DATA_TYPE_ID,
                IndicatedAirspeed::SIGNATURE,
                &IndicatedAirspeed {
                    indicated_airspeed: condition.ias_m_s as f32,
                    indicated_airspeed_variance: 0.5,
                }
                .encode(),
            ),
        );
        out
    }

    fn power_frames(&mut self, reading: &Reading) -> Vec<Frame> {
        let bus = CircuitStatus {
            circuit_id: 1,
            voltage: reading.bus_v as f32,
            current: 30.0,
            error_flags: 0,
        };
        self.emit(
            NODE_POWER,
            PRIORITY_AMBIENT,
            CircuitStatus::DEFAULT_DATA_TYPE_ID,
            CircuitStatus::SIGNATURE,
            &bus.encode(),
        )
    }

    fn emit(
        &mut self,
        node: u8,
        priority: u8,
        data_type_id: u16,
        signature: u64,
        payload: &[u8],
    ) -> Vec<Frame> {
        let id = MessageId {
            priority,
            data_type_id,
            source_node_id: node,
        };
        let transfer_id = self.transfer_ids.next(node, data_type_id);
        frames_for(id, signature, payload, transfer_id)
    }
}

/// Fuel flow in the units the DSDL field uses, cm3 per minute.
///
/// Routed through the model's own litres-per-hour helper rather than converted
/// here. The model works in **mass** flow, and the message wants **volume**; an
/// earlier version of this function multiplied kilograms per second straight
/// into cubic centimetres and published a fuel burn a thousand times too high,
/// which every downstream unit conversion then faithfully preserved.
fn fuel_rate_cm3pm(params: &EngineParams, outputs: &Outputs) -> f32 {
    (outputs.fuel_litres_per_hour(params) * 1000.0 / 60.0) as f32
}

/// The optional flag groups this controller populates, plus the conditions.
fn condition_flags(reading: &Reading) -> u32 {
    let mut set = flags::TEMPERATURE_SUPPORTED | flags::OIL_PRESSURE_SUPPORTED;
    let hottest_head = reading.cht_k.iter().copied().fold(f64::MIN, f64::max);
    let hottest_exhaust = reading.egt_k.iter().copied().fold(f64::MIN, f64::max);

    if hottest_head > CHT_OVERHEAT_K {
        set |= flags::TEMPERATURE_OVERHEATING;
    } else if hottest_head > CHT_CAUTION_K {
        set |= flags::TEMPERATURE_ABOVE_NOMINAL;
    }
    if hottest_exhaust > EGT_CAUTION_K {
        set |= flags::TEMPERATURE_EGT_ABOVE_NOMINAL;
    }
    if reading.oil_p_pa < OIL_PRESSURE_LOW_PA {
        set |= flags::OIL_PRESSURE_BELOW_NOMINAL;
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::Profile;
    use crate::plant::Plant;
    use crate::sensors::Sensors;

    fn one_tick() -> (Vec<Frame>, Reading) {
        let condition = Profile::Cruise.condition_at(0.0);
        let mut plant = Plant::new(engine_model::engines::ae330(), &condition);
        let mut sensors = Sensors::new(1);
        let outputs = plant.advance(&condition, 0.05);
        let reading = sensors.sample(&plant.params, &plant.state, &outputs, 0.05);
        let mut publisher = Publisher::new(AuxiliaryStatus::DEFAULT_DATA_TYPE_ID);
        let frames = publisher.frames(&plant, &condition, &outputs, &reading, 20);
        (frames, reading)
    }

    /// Every message type reaches the bus on the first tick, each from the node
    /// that would source it on a real airframe.
    #[test]
    fn the_first_tick_publishes_every_message_type() {
        let (frames, _) = one_tick();
        let mut seen: Vec<(u16, u8)> = frames
            .iter()
            .filter_map(|f| MessageId::from_raw(f.id()))
            .map(|id| (id.data_type_id, id.source_node_id))
            .collect();
        seen.sort_unstable();
        seen.dedup();

        assert!(seen.contains(&(1120, NODE_ENGINE)));
        assert!(seen.contains(&(1129, NODE_ENGINE)));
        assert!(seen.contains(&(20950, NODE_ENGINE)));
        assert!(seen.contains(&(1028, NODE_AIR_DATA)));
        assert!(seen.contains(&(1029, NODE_AIR_DATA)));
        assert!(seen.contains(&(1021, NODE_AIR_DATA)));
        assert!(seen.contains(&(1091, NODE_POWER)));
        assert_eq!(seen.len(), 7);
    }

    /// A status message is eleven frames, so the steady-state frame budget is
    /// worth knowing before it is on a real 1 Mbit bus rather than after.
    #[test]
    fn the_steady_state_tick_is_thirteen_frames() {
        let condition = Profile::Cruise.condition_at(0.0);
        let mut plant = Plant::new(engine_model::engines::ae330(), &condition);
        let mut sensors = Sensors::new(1);
        let mut publisher = Publisher::new(AuxiliaryStatus::DEFAULT_DATA_TYPE_ID);
        let mut counts = Vec::new();
        for _ in 0..20 {
            let outputs = plant.advance(&condition, 0.05);
            let reading = sensors.sample(&plant.params, &plant.state, &outputs, 0.05);
            counts.push(
                publisher
                    .frames(&plant, &condition, &outputs, &reading, 20)
                    .len(),
            );
        }
        // Status is 11 frames and the vendor message 2; the rest are decimated.
        assert_eq!(counts[1], 13);
        assert!(counts.iter().sum::<usize>() < 20 * 20);
    }

    /// The check that was missing when a mass flow went out as a volumetric one.
    ///
    /// Asserts against the model's own conversion rather than against a literal,
    /// so it stays true if the fuel density is retuned, and separately against a
    /// plausible band, so a factor-of-a-thousand error cannot pass by agreeing
    /// with an equally wrong helper.
    #[test]
    fn the_published_fuel_rate_is_volumetric() {
        let condition = Profile::Cruise.condition_at(0.0);
        let mut plant = Plant::new(engine_model::engines::ae330(), &condition);
        let outputs = plant.advance(&condition, 0.05);

        let published = f64::from(fuel_rate_cm3pm(&plant.params, &outputs));
        let litres_per_hour = published * 60.0 / 1000.0;
        assert!(
            (litres_per_hour - outputs.fuel_litres_per_hour(&plant.params)).abs() < 0.1,
            "{litres_per_hour} L/h disagrees with the model"
        );
        // A 2 litre engine at part power burns tens of litres an hour, not tens
        // of thousands. The rating point is 39 L/h at full power at sea level.
        assert!(
            (1.0..60.0).contains(&litres_per_hour),
            "{litres_per_hour} L/h is not a physical fuel burn"
        );
    }

    #[test]
    fn a_healthy_engine_raises_only_the_supported_flags() {
        let (_, reading) = one_tick();
        let set = condition_flags(&reading);
        assert_eq!(
            set,
            flags::TEMPERATURE_SUPPORTED | flags::OIL_PRESSURE_SUPPORTED,
            "unexpected condition flags at the canonical cruise point"
        );
    }
}
