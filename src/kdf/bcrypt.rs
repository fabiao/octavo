use byteorder::{ByteOrder, BigEndian};

use crypto::block::blowfish::Blowfish;

fn bcrypt_setup(cost: usize, salt: &[u8], key: &[u8]) -> Blowfish {
    let mut state = Blowfish::init().salted_expand_key(salt, key);

    for _ in 0..(1 << cost) {
        state = state.expand_key(key).expand_key(salt);
    }

    state
}

pub fn bcrypt<S: AsRef<[u8]>, I: AsRef<[u8]>, O: AsMut<[u8]>>(cost: usize,
                                                              salt: S,
                                                              input: I,
                                                              mut output: O) {
    assert_eq!(salt.as_ref().len(), 16);
    assert!(0 < input.as_ref().len() && input.as_ref().len() <= 72);
    assert_eq!(output.as_mut().len(), 24);

    let mut output = output.as_mut();

    let state = bcrypt_setup(cost, salt.as_ref(), input.as_ref());
    let mut ctext = [0x4f727068, 0x65616e42, 0x65686f6c, 0x64657253, 0x63727944, 0x6f756274];
    for (chunk, out) in ctext.chunks_mut(2).zip(output.chunks_mut(8)) {
        for _ in 0..64 {
            let (l, r) = state.encrypt_round((chunk[0], chunk[1]));
            chunk[0] = l;
            chunk[1] = r;
        }
        BigEndian::write_u32(&mut out[0..4], chunk[0]);
        BigEndian::write_u32(&mut out[4..8], chunk[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::bcrypt;

    struct Test<'a> {
        cost: usize,
        salt: &'a [u8],
        input: &'a [u8],
        output: [u8; 23],
    }

    const OPENWALL_TESTS: &'static [Test<'static>] = &[Test {
                                                           input: &[0x55, 0x2A, 0x55, 0x00],
                                                           cost: 5,
                                                           salt: &[0x10, 0x41, 0x04, 0x10, 0x41,
                                                                   0x04, 0x10, 0x41, 0x04, 0x10,
                                                                   0x41, 0x04, 0x10, 0x41, 0x04,
                                                                   0x10],
                                                           output: [0x1B, 0xB6, 0x91, 0x43, 0xF9,
                                                                    0xA8, 0xD3, 0x04, 0xC8, 0xD2,
                                                                    0x3D, 0x99, 0xAB, 0x04, 0x9A,
                                                                    0x77, 0xA6, 0x8E, 0x2C, 0xCC,
                                                                    0x74, 0x42, 0x06],
                                                       },
                                                       Test {
                                                           input: &[0x55, 0x2A, 0x55, 0x2A, 0x00],
                                                           cost: 5,
                                                           salt: &[0x10, 0x41, 0x04, 0x10, 0x41,
                                                                   0x04, 0x10, 0x41, 0x04, 0x10,
                                                                   0x41, 0x04, 0x10, 0x41, 0x04,
                                                                   0x10],
                                                           output: [0x5C, 0x84, 0x35, 0x0B, 0xDF,
                                                                    0xBA, 0xA9, 0x6A, 0xC1, 0x6F,
                                                                    0x61, 0x5A, 0xE7, 0x9F, 0x35,
                                                                    0xCF, 0xDA, 0xCD, 0x68, 0x2D,
                                                                    0x36, 0x9F, 0x23],
                                                       },
                                                       Test {
                                                           input: &[0x55, 0x2A, 0x55, 0x2A, 0x55,
                                                                    0x00],
                                                           cost: 5,
                                                           salt: &[0x65, 0x96, 0x59, 0x65, 0x96,
                                                                   0x59, 0x65, 0x96, 0x59, 0x65,
                                                                   0x96, 0x59, 0x65, 0x96, 0x59,
                                                                   0x65],
                                                           output: [0x09, 0xE6, 0x73, 0xA3, 0xF9,
                                                                    0xA5, 0x44, 0x81, 0x8E, 0xB8,
                                                                    0xDD, 0x69, 0xA8, 0xCB, 0x28,
                                                                    0xB3, 0x2F, 0x6F, 0x7B, 0xE6,
                                                                    0x04, 0xCF, 0xA7],
                                                       },
                                                       Test {
                                                           input: &[0x30, 0x31, 0x32, 0x33, 0x34,
                                                                    0x35, 0x36, 0x37, 0x38, 0x39,
                                                                    0x61, 0x62, 0x63, 0x64, 0x65,
                                                                    0x66, 0x67, 0x68, 0x69, 0x6A,
                                                                    0x6B, 0x6C, 0x6D, 0x6E, 0x6F,
                                                                    0x70, 0x71, 0x72, 0x73, 0x74,
                                                                    0x75, 0x76, 0x77, 0x78, 0x79,
                                                                    0x7A, 0x41, 0x42, 0x43, 0x44,
                                                                    0x45, 0x46, 0x47, 0x48, 0x49,
                                                                    0x4A, 0x4B, 0x4C, 0x4D, 0x4E,
                                                                    0x4F, 0x50, 0x51, 0x52, 0x53,
                                                                    0x54, 0x55, 0x56, 0x57, 0x58,
                                                                    0x59, 0x5A, 0x30, 0x31, 0x32,
                                                                    0x33, 0x34, 0x35, 0x36, 0x37,
                                                                    0x38, 0x39],
                                                           cost: 5,
                                                           salt: &[0x71, 0xD7, 0x9F, 0x82, 0x18,
                                                                   0xA3, 0x92, 0x59, 0xA7, 0xA2,
                                                                   0x9A, 0xAB, 0xB2, 0xDB, 0xAF,
                                                                   0xC3],
                                                           output: [0xEE, 0xEE, 0x31, 0xF8, 0x09,
                                                                    0x19, 0x92, 0x04, 0x25, 0x88,
                                                                    0x10, 0x02, 0xD1, 0x40, 0xD5,
                                                                    0x55, 0xB2, 0x8A, 0x5C, 0x72,
                                                                    0xE0, 0x0F, 0x09],
                                                       },
                                                       Test {
                                                           input: &[0xFF, 0xFF, 0xA3, 0x00],
                                                           cost: 5,
                                                           salt: &[0x05, 0x03, 0x00, 0x85, 0xD5,
                                                                   0xED, 0x4C, 0x17, 0x6B, 0x2A,
                                                                   0xC3, 0xCB, 0xEE, 0x47, 0x29,
                                                                   0x1C],
                                                           output: [0x10, 0x6E, 0xE0, 0x9C, 0x97,
                                                                    0x1C, 0x43, 0xA1, 0x9D, 0x8A,
                                                                    0x25, 0xC5, 0x95, 0xDF, 0x91,
                                                                    0xDF, 0xF4, 0xF0, 0x9B, 0x56,
                                                                    0x54, 0x3B, 0x98],
                                                       },
                                                       Test {
                                                           input: &[0xA3, 0x00],
                                                           cost: 5,
                                                           salt: &[0x05, 0x03, 0x00, 0x85, 0xD5,
                                                                   0xED, 0x4C, 0x17, 0x6B, 0x2A,
                                                                   0xC3, 0xCB, 0xEE, 0x47, 0x29,
                                                                   0x1C],
                                                           output: [0x51, 0xCF, 0x6E, 0x8D, 0xDA,
                                                                    0x3A, 0x01, 0x0D, 0x4C, 0xAF,
                                                                    0x11, 0xE9, 0x67, 0x7A, 0xD2,
                                                                    0x36, 0x84, 0x98, 0xFF, 0xCA,
                                                                    0x96, 0x9C, 0x4B],
                                                       },
                                                       Test {
                                                           input: &[0xFF, 0xA3, 0x33, 0x34, 0xFF,
                                                                    0xFF, 0xFF, 0xA3, 0x33, 0x34,
                                                                    0x35, 0x00],
                                                           cost: 5,
                                                           salt: &[0x05, 0x03, 0x00, 0x85, 0xD5,
                                                                   0xED, 0x4C, 0x17, 0x6B, 0x2A,
                                                                   0xC3, 0xCB, 0xEE, 0x47, 0x29,
                                                                   0x1C],
                                                           output: [0xA8, 0x00, 0x69, 0xE3, 0xB6,
                                                                    0x57, 0x86, 0x9F, 0x2A, 0x09,
                                                                    0x17, 0x16, 0xC4, 0x98, 0x00,
                                                                    0x12, 0xE9, 0xBA, 0xD5, 0x38,
                                                                    0x6E, 0x69, 0x19],
                                                       },
                                                       Test {
                                                           input: &[0xFF, 0xA3, 0x33, 0x34, 0x35,
                                                                    0x00],
                                                           cost: 5,
                                                           salt: &[0x05, 0x03, 0x00, 0x85, 0xD5,
                                                                   0xED, 0x4C, 0x17, 0x6B, 0x2A,
                                                                   0xC3, 0xCB, 0xEE, 0x47, 0x29,
                                                                   0x1C],
                                                           output: [0xA5, 0x38, 0xEF, 0xE2, 0x70,
                                                                    0x49, 0x4E, 0x3B, 0x7C, 0xD6,
                                                                    0x81, 0x2B, 0xFF, 0x16, 0x96,
                                                                    0xC7, 0x1B, 0xAC, 0xD2, 0x98,
                                                                    0x67, 0x87, 0xF8],
                                                       },
                                                       Test {
                                                           input: &[0xA3, 0x61, 0x62, 0x00],
                                                           cost: 5,
                                                           salt: &[0x05, 0x03, 0x00, 0x85, 0xD5,
                                                                   0xED, 0x4C, 0x17, 0x6B, 0x2A,
                                                                   0xC3, 0xCB, 0xEE, 0x47, 0x29,
                                                                   0x1C],
                                                           output: [0xF0, 0xA8, 0x67, 0x4A, 0x62,
                                                                    0xF4, 0xBE, 0xA4, 0xD7, 0x7B,
                                                                    0x7D, 0x30, 0x70, 0xFB, 0xC9,
                                                                    0x86, 0x4C, 0x2C, 0x00, 0x74,
                                                                    0xE7, 0x50, 0xA5],
                                                       },
                                                       Test {
                                                           input: &[0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
                                                                    0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
                                                                    0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
                                                                    0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
                                                                    0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
                                                                    0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
                                                                    0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
                                                                    0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
                                                                    0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
                                                                    0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
                                                                    0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
                                                                    0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
                                                                    0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
                                                                    0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
                                                                    0xAA, 0xAA],
                                                           cost: 5,
                                                           salt: &[0x05, 0x03, 0x00, 0x85, 0xD5,
                                                                   0xED, 0x4C, 0x17, 0x6B, 0x2A,
                                                                   0xC3, 0xCB, 0xEE, 0x47, 0x29,
                                                                   0x1C],
                                                           output: [0xBB, 0x24, 0x90, 0x2B, 0x59,
                                                                    0x50, 0x90, 0xBF, 0xC8, 0x24,
                                                                    0x64, 0x70, 0x8C, 0x69, 0xB1,
                                                                    0xB2, 0xD5, 0xB4, 0xC5, 0x88,
                                                                    0xC6, 0x3B, 0x3F],
                                                       },
                                                       Test {
                                                           input: &[0xAA, 0x55, 0xAA, 0x55, 0xAA,
                                                                    0x55, 0xAA, 0x55, 0xAA, 0x55,
                                                                    0xAA, 0x55, 0xAA, 0x55, 0xAA,
                                                                    0x55, 0xAA, 0x55, 0xAA, 0x55,
                                                                    0xAA, 0x55, 0xAA, 0x55, 0xAA,
                                                                    0x55, 0xAA, 0x55, 0xAA, 0x55,
                                                                    0xAA, 0x55, 0xAA, 0x55, 0xAA,
                                                                    0x55, 0xAA, 0x55, 0xAA, 0x55,
                                                                    0xAA, 0x55, 0xAA, 0x55, 0xAA,
                                                                    0x55, 0xAA, 0x55, 0xAA, 0x55,
                                                                    0xAA, 0x55, 0xAA, 0x55, 0xAA,
                                                                    0x55, 0xAA, 0x55, 0xAA, 0x55,
                                                                    0xAA, 0x55, 0xAA, 0x55, 0xAA,
                                                                    0x55, 0xAA, 0x55, 0xAA, 0x55,
                                                                    0xAA, 0x55],
                                                           cost: 5,
                                                           salt: &[0x05, 0x03, 0x00, 0x85, 0xD5,
                                                                   0xED, 0x4C, 0x17, 0x6B, 0x2A,
                                                                   0xC3, 0xCB, 0xEE, 0x47, 0x29,
                                                                   0x1C],
                                                           output: [0x4F, 0xFC, 0xED, 0x16, 0x59,
                                                                    0x34, 0x7B, 0x33, 0x9D, 0x48,
                                                                    0x6E, 0x1D, 0xAC, 0x0C, 0x62,
                                                                    0xB2, 0x76, 0xAB, 0x63, 0xBC,
                                                                    0xB3, 0xE3, 0x4D],
                                                       },
                                                       Test {
                                                           input: &[0x55, 0xAA, 0xFF, 0x55, 0xAA,
                                                                    0xFF, 0x55, 0xAA, 0xFF, 0x55,
                                                                    0xAA, 0xFF, 0x55, 0xAA, 0xFF,
                                                                    0x55, 0xAA, 0xFF, 0x55, 0xAA,
                                                                    0xFF, 0x55, 0xAA, 0xFF, 0x55,
                                                                    0xAA, 0xFF, 0x55, 0xAA, 0xFF,
                                                                    0x55, 0xAA, 0xFF, 0x55, 0xAA,
                                                                    0xFF, 0x55, 0xAA, 0xFF, 0x55,
                                                                    0xAA, 0xFF, 0x55, 0xAA, 0xFF,
                                                                    0x55, 0xAA, 0xFF, 0x55, 0xAA,
                                                                    0xFF, 0x55, 0xAA, 0xFF, 0x55,
                                                                    0xAA, 0xFF, 0x55, 0xAA, 0xFF,
                                                                    0x55, 0xAA, 0xFF, 0x55, 0xAA,
                                                                    0xFF, 0x55, 0xAA, 0xFF, 0x55,
                                                                    0xAA, 0xFF],
                                                           cost: 5,
                                                           salt: &[0x05, 0x03, 0x00, 0x85, 0xD5,
                                                                   0xED, 0x4C, 0x17, 0x6B, 0x2A,
                                                                   0xC3, 0xCB, 0xEE, 0x47, 0x29,
                                                                   0x1C],
                                                           output: [0xFE, 0xF4, 0x9B, 0xD5, 0xE2,
                                                                    0xE1, 0xA3, 0x9C, 0x25, 0xE0,
                                                                    0xFC, 0x4B, 0x06, 0x9E, 0xF3,
                                                                    0x9A, 0x3A, 0xEC, 0x36, 0xD3,
                                                                    0xAB, 0x60, 0x48],
                                                       },
                                                       Test {
                                                           input: &[0x00],
                                                           cost: 5,
                                                           salt: &[0x10, 0x41, 0x04, 0x10, 0x41,
                                                                   0x04, 0x10, 0x41, 0x04, 0x10,
                                                                   0x41, 0x04, 0x10, 0x41, 0x04,
                                                                   0x10],
                                                           output: [0xF7, 0x02, 0x36, 0x5C, 0x4D,
                                                                    0x4A, 0xE1, 0xD5, 0x3D, 0x97,
                                                                    0xCD, 0x28, 0xB0, 0xB9, 0x3F,
                                                                    0x11, 0xF7, 0x9F, 0xCE, 0x44,
                                                                    0xD5, 0x60, 0xFD],
                                                       }];

    #[test]
    fn openwall_test_vectors() {
        let mut output = [0u8; 24];
        for test in OPENWALL_TESTS {
            bcrypt(test.cost, &test.salt[..], &test.input[..], &mut output[..]);
            assert_eq!(&output[0..23], &test.output[..]);
        }
    }
}
