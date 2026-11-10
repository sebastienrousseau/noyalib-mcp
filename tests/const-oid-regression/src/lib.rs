// SPDX-FileCopyrightText: 2026 Noyalib
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use const_oid::{Error, ObjectIdentifier, ObjectIdentifierRef};

    #[test]
    fn rejects_arc_larger_than_u32() {
        for leading in 16_u8..=127 {
            let bytes = [0x2a, 0x80 | leading, 0x80, 0x80, 0x80, 0x00];
            assert_eq!(
                ObjectIdentifierRef::from_bytes(&bytes),
                Err(Error::ArcTooBig)
            );
        }
    }

    #[test]
    fn rejects_six_byte_arc_without_truncating_it() {
        let bytes = [0x2a, 0x81, 0x80, 0x80, 0x80, 0x80, 0x00];
        assert_eq!(
            ObjectIdentifierRef::from_bytes(&bytes),
            Err(Error::ArcTooBig)
        );
    }

    #[test]
    fn accepts_representable_arc_boundaries() {
        for arc in [0, 1, 127, 128, 16383, 16384, 1 << 21, 1 << 28, u32::MAX] {
            let oid = ObjectIdentifier::from_arcs([1, 2, arc]).unwrap();
            let parsed = ObjectIdentifierRef::from_bytes(oid.as_bytes()).unwrap();
            assert_eq!(parsed.arcs().collect::<Vec<_>>(), vec![1, 2, arc]);
        }
    }
}
