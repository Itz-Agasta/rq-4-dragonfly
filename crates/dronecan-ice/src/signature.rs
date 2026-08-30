//! Data type signatures, derived rather than transcribed.
//!
//! A signature is CRC-64-WE over the *normalised definition* of a data type: the
//! full type name, then one line per field, comments and constants stripped,
//! cast mode made explicit. Nested compound types contribute their own signature
//! rather than their text.
//!
//! Every signature this crate uses is computed here at compile time from the
//! definition string, and the tests assert the results equal the values the
//! reference implementation publishes. That is what makes the one signature with
//! no published value, the vendor-specific message, trustworthy: it is produced
//! by machinery proven against seven types that do have one.
//!
//! <https://dronecan.github.io/Specification/3._Data_structure_description_language/>

use crate::crc::crc64_we;

/// Signature of a data type with no nested compound fields.
///
/// For such a type the data type signature and the DSDL signature are equal, so
/// this is the whole computation.
#[must_use]
pub const fn dsdl_signature(normalised_definition: &str) -> u64 {
    crc64_we(normalised_definition.as_bytes())
}

/// Extend a signature with a nested compound type's signature.
///
/// The hash is re-run over the nested signature followed by the current value,
/// both least significant byte first. Applied once per nested structure, in the
/// order the fields appear.
#[must_use]
pub const fn extend(signature: u64, nested: u64) -> u64 {
    let mut bytes = [0u8; 16];
    let n = nested.to_le_bytes();
    let s = signature.to_le_bytes();
    let mut i = 0;
    while i < 8 {
        bytes[i] = n[i];
        bytes[i + 8] = s[i];
        i += 1;
    }
    crc64_we_from(signature, &bytes)
}

/// CRC-64-WE resumed from an existing signature value.
///
/// The published value has already had the output xor applied, so it is undone
/// before the register is reloaded.
const fn crc64_we_from(signature: u64, bytes: &[u8]) -> u64 {
    const POLY: u64 = 0x42F0_E1EB_A9EA_3693;
    let mut crc = signature ^ u64::MAX;
    let mut i = 0;
    while i < bytes.len() {
        crc ^= (bytes[i] as u64) << 56;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & (1 << 63) != 0 {
                (crc << 1) ^ POLY
            } else {
                crc << 1
            };
            bit += 1;
        }
        i += 1;
    }
    crc ^ u64::MAX
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{
        AuxiliaryStatus, CircuitStatus, CylinderStatus, FuelTankStatus, IndicatedAirspeed, Message,
        ReciprocatingStatus, StaticPressure, StaticTemperature,
    };

    /// Against the signature table the reference implementation publishes.
    ///
    /// Seven independent values agreeing means the hash, the normalisation rules
    /// and the nesting extension are all correct, which is the licence to trust
    /// the eighth.
    ///
    /// <https://forum.opencyphal.org/t/data-type-signature/241>
    #[test]
    fn signatures_match_the_published_table() {
        assert_eq!(StaticPressure::SIGNATURE, 0xCDC7_C434_12BD_C89A);
        assert_eq!(StaticTemperature::SIGNATURE, 0x4927_2A64_77D9_6271);
        assert_eq!(IndicatedAirspeed::SIGNATURE, 0x0A18_92D7_2AB8_945F);
        assert_eq!(CircuitStatus::SIGNATURE, 0x8313_D33D_0DDD_A115);
        assert_eq!(FuelTankStatus::SIGNATURE, 0x286B_4A38_7BA8_4BC4);
        assert_eq!(CylinderStatus::SIGNATURE, 0xD68A_C83A_89D5_B36B);
        assert_eq!(ReciprocatingStatus::SIGNATURE, 0xD38A_A3EE_7553_7EC6);
    }

    /// The nesting step in isolation: `Status` carries `CylinderStatus`, so its
    /// data type signature is its own DSDL signature extended once.
    #[test]
    fn extension_produces_the_published_status_signature() {
        let dsdl = 0x5465_C0CF_3761_9F32;
        assert_eq!(
            extend(dsdl, CylinderStatus::SIGNATURE),
            0xD38A_A3EE_7553_7EC6
        );
    }

    /// No published value exists for the vendor type, so all that can be asserted
    /// is that it is derived and stable. A change to its field list changes this.
    #[test]
    fn the_vendor_signature_is_derived_from_its_definition() {
        assert_eq!(
            AuxiliaryStatus::SIGNATURE,
            dsdl_signature(AuxiliaryStatus::NORMALISED_DEFINITION)
        );
    }
}
