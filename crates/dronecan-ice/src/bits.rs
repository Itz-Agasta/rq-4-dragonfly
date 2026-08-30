//! DSDL bit-level serialisation.
//!
//! The wire format is unusual enough to be worth stating precisely, because
//! nothing about it is guessable and getting it wrong produces frames that look
//! plausible and decode to nonsense.
//!
//! Fields are packed with no alignment. Within the stream, bits fill a byte from
//! its most significant end. A field wider than one bit is first decomposed into
//! bytes **least significant byte first**; each of those bytes is then written
//! most significant bit first. When the field width is not a multiple of eight,
//! the leading chunk is the remainder and is narrower than a byte.
//!
//! So a `uint30` whose bit 0 is set puts that bit at stream position 7, not 29:
//! the low byte goes out first and within it the bit is the least significant of
//! eight. Reading the specification as plain big-endian bit packing gives the
//! wrong answer for every field wider than eight bits.
//!
//! # Cast mode
//!
//! Every field in the message set is `saturated`, which is the DSDL default and
//! means a value outside the field's range is **clamped**, not truncated. Writing
//! 200 into a `uint7` gives 127, not 72, and writing 180000 into a `float16`
//! gives the largest finite half rather than infinity. Masking instead, which is
//! the obvious implementation and is what `truncated` means, produces a payload
//! that decodes to a plausible wrong number rather than to an error.
//!
//! <https://dronecan.github.io/Specification/3._Data_structure_description_language/>

/// Serialises DSDL fields into a bit stream.
#[derive(Debug, Default)]
pub struct BitWriter {
    bytes: Vec<u8>,
    bits: usize,
}

impl BitWriter {
    /// A writer with an empty stream.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn push_bit(&mut self, set: bool) {
        if self.bits.is_multiple_of(8) {
            self.bytes.push(0);
        }
        if set {
            let byte = self.bytes.len() - 1;
            self.bytes[byte] |= 0x80 >> (self.bits % 8);
        }
        self.bits += 1;
    }

    /// Write `value` into a `width`-bit unsigned field, saturating.
    ///
    /// Widths above 64 are not representable and are clamped; no DSDL primitive
    /// is wider than 64 bits, so this cannot be reached from a generated message.
    pub fn write_uint(&mut self, value: u64, width: u32) {
        let width = width.min(64);
        self.write_raw_bits(value.min(mask(width)), width);
    }

    /// Write a `void` field of `width` bits. The specification requires zeros.
    pub fn write_void(&mut self, width: u32) {
        self.write_uint(0, width);
    }

    /// Write an IEEE754 binary16, saturating at the largest finite half.
    pub fn write_f16(&mut self, value: f32) {
        self.write_raw_bits(u64::from(f16_from_f32(saturate(value, F16_MAX))), 16);
    }

    /// Write an IEEE754 binary32, saturating at the largest finite single.
    pub fn write_f32(&mut self, value: f32) {
        self.write_raw_bits(u64::from(saturate(value, f32::MAX).to_bits()), 32);
    }

    /// Write bits that are already in range, bypassing the saturating cast.
    fn write_raw_bits(&mut self, value: u64, width: u32) {
        let mut offset = 0;
        while offset < width {
            let chunk_bits = (width - offset).min(8);
            let chunk = (value >> offset) & mask(chunk_bits);
            for k in (0..chunk_bits).rev() {
                self.push_bit((chunk >> k) & 1 == 1);
            }
            offset += 8;
        }
    }

    /// The finished payload, zero-padded to the next byte boundary.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// Reads DSDL fields back out of a bit stream.
#[derive(Debug)]
pub struct BitReader<'a> {
    bytes: &'a [u8],
    bits: usize,
}

impl<'a> BitReader<'a> {
    /// A reader over a whole payload.
    #[must_use]
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, bits: 0 }
    }

    /// Bits not yet consumed.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.bytes.len() * 8 - self.bits
    }

    fn take_bit(&mut self) -> Option<bool> {
        let byte = self.bytes.get(self.bits / 8)?;
        let set = (byte >> (7 - self.bits % 8)) & 1 == 1;
        self.bits += 1;
        Some(set)
    }

    /// Read a `width`-bit unsigned field, or `None` if the stream is short.
    pub fn read_uint(&mut self, width: u32) -> Option<u64> {
        let width = width.min(64);
        let mut value = 0u64;
        let mut offset = 0;
        while offset < width {
            let chunk_bits = (width - offset).min(8);
            let mut chunk = 0u64;
            for _ in 0..chunk_bits {
                chunk = (chunk << 1) | u64::from(self.take_bit()?);
            }
            value |= chunk << offset;
            offset += 8;
        }
        Some(value)
    }

    /// Consume a `void` field. Its contents are ignored per the specification.
    pub fn skip_void(&mut self, width: u32) -> Option<()> {
        self.read_uint(width).map(|_| ())
    }

    /// Read an IEEE754 binary16.
    pub fn read_f16(&mut self) -> Option<f32> {
        self.read_uint(16).map(|v| f32_from_f16(v as u16))
    }

    /// Read an IEEE754 binary32.
    pub fn read_f32(&mut self) -> Option<f32> {
        self.read_uint(32).map(|v| f32::from_bits(v as u32))
    }
}

/// Largest finite IEEE754 binary16 value.
pub const F16_MAX: f32 = 65504.0;

/// The `saturated` cast: clamp into range, leaving NaN alone.
///
/// NaN compares false against everything, so it passes through unchanged, which
/// is what the specification wants: NaN is how an optional field says "not
/// measured" and clamping it to a finite value would invent a measurement.
fn saturate(value: f32, limit: f32) -> f32 {
    if value > limit {
        limit
    } else if value < -limit {
        -limit
    } else {
        value
    }
}

fn mask(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

/// Convert `f32` to IEEE754 binary16.
///
/// This is libuavcan's conversion, reproduced bit for bit rather than replaced
/// with a correctly rounding one. It truncates the mantissa to the binary16
/// width and then adds one unit in the last place, which is not round-to-nearest
/// and differs from a naive implementation on about half of all inputs. Every
/// DroneCAN node on the bus rounds this way, so matching it is interoperability,
/// not pedantry.
#[must_use]
pub fn f16_from_f32(value: f32) -> u16 {
    const F32_INFTY: u32 = 255 << 23;
    const F16_INFTY: u32 = 31 << 23;
    const MAGIC: u32 = 15 << 23;
    const ROUND_MASK: u32 = !0xFFF;

    let bits = value.to_bits();
    let sign = bits & 0x8000_0000;
    let mut u = bits ^ sign;

    let out = if u >= F32_INFTY {
        if u > F32_INFTY { 0x7FFF } else { 0x7C00 }
    } else {
        u &= ROUND_MASK;
        u = (f32::from_bits(u) * f32::from_bits(MAGIC)).to_bits();
        u = u.wrapping_sub(ROUND_MASK);
        if u > F16_INFTY {
            u = F16_INFTY;
        }
        ((u >> 13) & 0xFFFF) as u16
    };
    out | ((sign >> 16) & 0xFFFF) as u16
}

/// Convert IEEE754 binary16 to `f32`. The inverse of [`f16_from_f32`].
#[must_use]
pub fn f32_from_f16(value: u16) -> f32 {
    const MAGIC: u32 = (254 - 15) << 23;
    const WAS_INF_NAN: u32 = (127 + 16) << 23;

    let mut out = f32::from_bits(u32::from(value & 0x7FFF) << 13) * f32::from_bits(MAGIC);
    if out.to_bits() >= WAS_INF_NAN {
        out = f32::from_bits(out.to_bits() | (255 << 23));
    }
    f32::from_bits(out.to_bits() | (u32::from(value & 0x8000) << 16))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The layout the whole codec rests on, taken from the first four bytes of a
    /// reference-encoded `reciprocating.Status`: a `uint2` then a `uint30`.
    #[test]
    fn a_thirty_bit_field_is_packed_low_byte_first() {
        let mut w = BitWriter::new();
        w.write_uint(0, 2);
        w.write_uint(1, 30);
        assert_eq!(w.finish(), vec![0x00, 0x40, 0x00, 0x00]);

        let mut w = BitWriter::new();
        w.write_uint(0, 2);
        w.write_uint(32768, 30);
        assert_eq!(w.finish(), vec![0x00, 0x20, 0x00, 0x00]);

        let mut w = BitWriter::new();
        w.write_uint(2, 2);
        w.write_uint(0, 30);
        assert_eq!(w.finish(), vec![0x80, 0x00, 0x00, 0x00]);
    }

    /// `saturated` is the default cast mode for every field in the message set,
    /// so an out-of-range value clamps. Masking would decode to a wrong number
    /// rather than to a wrong-looking one.
    #[test]
    fn out_of_range_values_saturate_rather_than_wrap() {
        let mut w = BitWriter::new();
        w.write_uint(200, 7);
        assert_eq!(BitReader::new(&w.finish()).read_uint(7), Some(127));

        let mut w = BitWriter::new();
        w.write_f16(180_000.0);
        assert_eq!(BitReader::new(&w.finish()).read_f16(), Some(F16_MAX));

        let mut w = BitWriter::new();
        w.write_f16(f32::NEG_INFINITY);
        assert_eq!(BitReader::new(&w.finish()).read_f16(), Some(-F16_MAX));

        // NaN is how an optional field says "not measured", so it must survive.
        let mut w = BitWriter::new();
        w.write_f16(f32::NAN);
        assert!(
            BitReader::new(&w.finish())
                .read_f16()
                .is_some_and(f32::is_nan)
        );
    }

    #[test]
    fn unaligned_fields_round_trip() {
        let widths = [1u32, 2, 3, 6, 7, 8, 9, 16, 17, 30, 32, 33, 64];
        let mut w = BitWriter::new();
        for (i, &width) in widths.iter().enumerate() {
            w.write_uint(mask(width) ^ (i as u64), width);
        }
        let bytes = w.finish();
        let mut r = BitReader::new(&bytes);
        for (i, &width) in widths.iter().enumerate() {
            assert_eq!(
                r.read_uint(width),
                Some(mask(width) ^ (i as u64)),
                "{width}"
            );
        }
    }

    #[test]
    fn reading_past_the_end_is_none_not_a_panic() {
        let mut r = BitReader::new(&[0xFF]);
        assert_eq!(r.read_uint(8), Some(0xFF));
        assert_eq!(r.read_uint(1), None);
    }

    #[test]
    fn half_floats_match_the_reference_special_cases() {
        assert_eq!(f16_from_f32(f32::NAN), 0x7FFF);
        assert_eq!(f16_from_f32(f32::INFINITY), 0x7C00);
        assert_eq!(f16_from_f32(f32::NEG_INFINITY), 0xFC00);
        assert_eq!(f16_from_f32(0.0), 0x0000);
        assert_eq!(f16_from_f32(1.0), 0x3C00);
        assert_eq!(f16_from_f32(-2.0), 0xC000);
        // Above binary16 range, so it saturates rather than wrapping.
        assert_eq!(f16_from_f32(180_000.0), 0x7C00);

        assert!(f32_from_f16(0x7FFF).is_nan());
        assert_eq!(f32_from_f16(0x7C00), f32::INFINITY);
        assert_eq!(f32_from_f16(0x3C00), 1.0);
        assert_eq!(f32_from_f16(0xC000), -2.0);
    }

    #[test]
    fn half_float_round_trip_is_within_binary16_resolution() {
        for v in [361.0f32, 843.0, 27.8, 1.68, 0.125, -40.0] {
            let back = f32_from_f16(f16_from_f32(v));
            assert!((back - v).abs() <= v.abs() * 1e-3 + 1e-3, "{v} -> {back}");
        }
    }
}
