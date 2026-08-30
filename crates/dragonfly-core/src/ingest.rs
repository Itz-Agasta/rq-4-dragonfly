//! CAN frames in, decoded messages out.
//!
//! One reassembler serves every node on the interface, because the transfer
//! state is keyed on the CAN identifier and that already carries the source node
//! ID. Decoding is dispatched on the data type ID, with the vendor message's ID
//! configurable since the vendor range has no registry.
//!
//! A decode failure is counted and dropped, never propagated. A malformed frame
//! from one node must not take the ingest loop down: the correct response to a
//! misbehaving device on a shared bus is to keep listening to the others.

use std::time::Instant;

use dronecan_ice::{
    AuxiliaryStatus, CircuitStatus, FuelTankStatus, IndicatedAirspeed, Message, Reassembler,
    ReciprocatingStatus, StaticPressure, StaticTemperature, Transfer,
};

/// A decoded message, tagged with what it is.
#[derive(Clone, Debug)]
pub enum Decoded {
    /// Engine controller status.
    Engine(Box<ReciprocatingStatus>),
    /// Vendor auxiliary channels.
    Auxiliary(AuxiliaryStatus),
    /// Fuel tank quantity.
    Fuel(FuelTankStatus),
    /// Ambient static pressure.
    Pressure(StaticPressure),
    /// Outside air temperature.
    Temperature(StaticTemperature),
    /// Indicated airspeed.
    Airspeed(IndicatedAirspeed),
    /// Electrical bus.
    Bus(CircuitStatus),
}

/// Counters worth reporting when something looks wrong on the bus.
#[derive(Clone, Copy, Debug, Default)]
pub struct Counters {
    /// Frames accepted from the interface.
    pub frames: u64,
    /// Transfers that reassembled successfully.
    pub transfers: u64,
    /// Transfers whose data type ID is not one this build handles.
    pub unknown: u64,
    /// Transfers that failed to decode, most often a signature mismatch.
    pub rejected: u64,
}

/// Reassembles transfers and decodes them.
#[derive(Debug)]
pub struct Ingest {
    reassembler: Reassembler,
    started: Instant,
    auxiliary_data_type_id: u16,
    /// Running counters.
    pub counters: Counters,
}

impl Ingest {
    /// An ingest whose vendor message uses `auxiliary_data_type_id`.
    #[must_use]
    pub fn new(auxiliary_data_type_id: u16) -> Self {
        Self {
            reassembler: Reassembler::new(),
            started: Instant::now(),
            auxiliary_data_type_id,
            counters: Counters::default(),
        }
    }

    /// Feed one CAN frame, yielding a message when a transfer completes.
    pub fn accept(&mut self, id: u32, data: &[u8]) -> Option<Decoded> {
        self.counters.frames += 1;
        let frame = dronecan_ice::Frame::new(id, data)?;
        let elapsed = Instant::now().saturating_duration_since(self.started);
        let transfer = self.reassembler.push(&frame, elapsed)?;
        self.counters.transfers += 1;

        let data_type_id = transfer.id.data_type_id;
        if data_type_id == self.auxiliary_data_type_id {
            return match AuxiliaryStatus::from_transfer_as(&transfer, data_type_id) {
                Ok(m) => Some(Decoded::Auxiliary(m)),
                Err(error) => self.reject(data_type_id, &error),
            };
        }

        match data_type_id {
            ReciprocatingStatus::DEFAULT_DATA_TYPE_ID => {
                self.decode(&transfer, |m| Decoded::Engine(Box::new(m)))
            }
            FuelTankStatus::DEFAULT_DATA_TYPE_ID => self.decode(&transfer, Decoded::Fuel),
            StaticPressure::DEFAULT_DATA_TYPE_ID => self.decode(&transfer, Decoded::Pressure),
            StaticTemperature::DEFAULT_DATA_TYPE_ID => self.decode(&transfer, Decoded::Temperature),
            IndicatedAirspeed::DEFAULT_DATA_TYPE_ID => self.decode(&transfer, Decoded::Airspeed),
            CircuitStatus::DEFAULT_DATA_TYPE_ID => self.decode(&transfer, Decoded::Bus),
            _ => {
                self.counters.unknown += 1;
                None
            }
        }
    }

    fn decode<M: Message>(
        &mut self,
        transfer: &Transfer,
        wrap: impl FnOnce(M) -> Decoded,
    ) -> Option<Decoded> {
        match M::from_transfer(transfer) {
            Ok(message) => Some(wrap(message)),
            Err(error) => self.reject(transfer.id.data_type_id, &error),
        }
    }

    fn reject(&mut self, data_type_id: u16, error: &dronecan_ice::DecodeError) -> Option<Decoded> {
        self.counters.rejected += 1;
        tracing::debug!(data_type_id, %error, "dropping a transfer");
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dronecan_ice::{MessageId, frames_for};

    fn publish(ingest: &mut Ingest, id: MessageId, signature: u64, payload: &[u8]) -> Vec<Decoded> {
        frames_for(id, signature, payload, 0)
            .iter()
            .filter_map(|f| ingest.accept(f.id(), f.data()))
            .collect()
    }

    fn engine_id(data_type_id: u16) -> MessageId {
        MessageId {
            priority: 16,
            data_type_id,
            source_node_id: 42,
        }
    }

    #[test]
    fn a_multi_frame_status_decodes_end_to_end() {
        let status = ReciprocatingStatus {
            engine_speed_rpm: 3720,
            cylinder_status: vec![dronecan_ice::CylinderStatus::default(); 4],
            ..ReciprocatingStatus::default()
        };
        let mut ingest = Ingest::new(AuxiliaryStatus::DEFAULT_DATA_TYPE_ID);
        let got = publish(
            &mut ingest,
            engine_id(ReciprocatingStatus::DEFAULT_DATA_TYPE_ID),
            ReciprocatingStatus::SIGNATURE,
            &status.encode(),
        );
        assert_eq!(got.len(), 1);
        assert!(matches!(&got[0], Decoded::Engine(s) if s.engine_speed_rpm == 3720));
        assert_eq!(ingest.counters.transfers, 1);
        assert_eq!(ingest.counters.rejected, 0);
    }

    #[test]
    fn the_vendor_message_follows_its_configured_id() {
        let aux = AuxiliaryStatus {
            turbocharger_speed_rpm: 118_400.0,
            ..AuxiliaryStatus::default()
        };
        let mut ingest = Ingest::new(20_951);
        let got = publish(
            &mut ingest,
            engine_id(20_951),
            AuxiliaryStatus::SIGNATURE,
            &aux.encode(),
        );
        assert!(matches!(&got[0], Decoded::Auxiliary(a) if a.turbocharger_speed_rpm == 118_400.0));

        // The default ID is no longer special once it has been reassigned.
        let mut ingest = Ingest::new(20_951);
        publish(
            &mut ingest,
            engine_id(AuxiliaryStatus::DEFAULT_DATA_TYPE_ID),
            AuxiliaryStatus::SIGNATURE,
            &aux.encode(),
        );
        assert_eq!(ingest.counters.unknown, 1);
    }

    /// A sender using a different definition of the same data type produces a
    /// checksum mismatch. Counting it is the difference between "the bus is
    /// quiet" and "the bus is talking and we cannot understand it".
    #[test]
    fn a_signature_mismatch_is_counted_not_decoded() {
        let status = ReciprocatingStatus {
            cylinder_status: vec![dronecan_ice::CylinderStatus::default(); 4],
            ..ReciprocatingStatus::default()
        };
        let mut ingest = Ingest::new(AuxiliaryStatus::DEFAULT_DATA_TYPE_ID);
        let got = publish(
            &mut ingest,
            engine_id(ReciprocatingStatus::DEFAULT_DATA_TYPE_ID),
            ReciprocatingStatus::SIGNATURE ^ 1,
            &status.encode(),
        );
        assert!(got.is_empty());
        assert_eq!(ingest.counters.rejected, 1);
    }

    #[test]
    fn traffic_for_another_data_type_is_ignored_quietly() {
        let mut ingest = Ingest::new(AuxiliaryStatus::DEFAULT_DATA_TYPE_ID);
        let got = publish(&mut ingest, engine_id(341), 0, &[1, 2, 3]);
        assert!(got.is_empty());
        assert_eq!(ingest.counters.unknown, 1);
    }
}
