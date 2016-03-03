// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The `aessafe` module implements the AES algorithm completely in software without using any table
//! lookups or other timing dependant mechanisms. This module actually contains two seperate
//! implementations - an implementation that works on a single block at a time and a second
//! implementation that processes 8 blocks in parallel. Some block encryption modes really only work if
//! you are processing a single blocks (CFB, OFB, and CBC encryption for example) while other modes
//! are trivially parallelizable (CTR and CBC decryption). Processing more blocks at once allows for
//! greater efficiency, especially when using wide registers, such as the XMM registers available in
//! x86 processors.
//!
//! ## AES Algorithm
//!
//! There are lots of places to go to on the internet for an involved description of how AES works.
//! For the purposes of this description, it sufficies to say that AES is just a block cipher that
//! takes a key of 16, 24, or 32 bytes and uses that to either encrypt or decrypt a block
//! of 16 bytes. An encryption or decryption operation consists of a number of rounds which involve
//! some combination of the following 4 basic operations:
//!
//! - ShiftRows
//! - MixColumns
//! - SubBytes
//! - AddRoundKey
//!
//! ## Timing problems
//!
//! Most software implementations of AES use a large set of lookup tables - generally at least the
//! SubBytes step is implemented via lookup tables; faster implementations generally implement the
//! MixColumns step this way as well. This is largely a design flaw in the AES implementation as it was
//! not realized during the NIST standardization process that table lookups can lead to security
//! problems [1]. The issue is that not all table lookups occur in constant time - an address that was
//! recently used is looked up much faster than one that hasn't been used in a while. A careful
//! adversary can measure the amount of time that each AES operation takes and use that information to
//! help determine the secret key or plain text information. More specifically, its not table lookups
//! that lead to these types of timing attacks - the issue is table lookups that use secret information
//! as part of the address to lookup. A table lookup that is performed the exact same way every time
//! regardless of the key or plaintext doesn't leak any information. This implementation uses no data
//! dependant table lookups.
//!
//! ## Bit Slicing
//!
//! Bit Slicing is a technique that is basically a software emulation of hardware implementation
//! techniques. One of the earliest implementations of this technique was for a DES implementation [4].
//! In hardware, table lookups do not present the same timing problems as they do in software, however
//! they present other problems - namely that a 256 byte S-box table takes up a huge amount of space on
//! a chip. Hardware implementations, thus, tend to avoid table lookups and instead calculate the
//! contents of the S-Boxes as part of every operation. So, the key to an efficient Bit Sliced software
//! implementation is to re-arrange all of the bits of data to process into a form that can easily be
//! applied in much the same way that it would be in hardeware. It is fortunate, that AES was designed
//! such that these types of hardware implementations could be very efficient - the contents of the
//! S-boxes are defined by a mathematical formula.
//!
//! A hardware implementation works on single bits at a time. Unlike adding variables in software,
//! however, that occur generally one at a time, hardware implementations are extremely parallel and
//! operate on many, many bits at once. Bit Slicing emulates that by moving all "equivalent" bits into
//! common registers and then operating on large groups of bits all at once. Calculating the S-box value
//! for a single bit is extremely expensive, but its much cheaper when you can amortize that cost over
//! 128 bits (as in an XMM register). This implementation follows the same strategy as in [5] and that
//! is an excellent source for more specific details. However, a short description follows.
//!
//! The input data is simply a collection of bytes. Each byte is comprised of 8 bits, a low order bit
//! (bit 0) through a high order bit (bit 7). Bit slicing the input data simply takes all of the low
//! order bits (bit 0) from the input data, and moves them into a single register (eg: XMM0). Next, all
//! of them 2nd lowest bits are moved into their own register (eg: XMM1), and so on. After completion,
//! we're left with 8 variables, each of which contains an equivalent set of bits. The exact order of
//! those bits is irrevent for the implementation of the SubBytes step, however, it is very important
//! for the MixColumns step. Again, see [5] for details. Due to the design of AES, its them possible to
//! execute the entire AES operation using just bitwise exclusive ors and rotates once we have Bit
//! Sliced the input data. After the completion of the AES operation, we then un-Bit Slice the data
//! to give us our output. Clearly, the more bits that we can process at once, the faster this will go -
//! thus, the version that processes 8 blocks at once is roughly 8 times faster than processing just a
//! single block at a time.
//!
//! The ShiftRows step is fairly straight-forward to implement on the Bit Sliced state. The MixColumns
//! and especially the SubBytes steps are more complicated. This implementation draws heavily on the
//! formulas from [5], [6], and [7] to implement these steps.
//!
//! ## Implementation
//!
//! Both implementations work basically the same way and share pretty much all of their code. The key
//! is first processed to create all of the round keys where each round key is just a 16 byte chunk of
//! data that is combined into the AES state by the AddRoundKey step as part of each encryption or
//! decryption round. Processing the round key can be expensive, so this is done before encryption or
//! decryption. Before encrypting or decrypting data, the data to be processed by be Bit Sliced into 8
//! seperate variables where each variable holds equivalent bytes from the state. This Bit Sliced state
//! is stored as a Bs8State<T>, where T is the type that stores each set of bits. The first
//! implementation stores these bits in a u32 which permits up to 8 * 32 = 1024 bits of data to be
//! processed at once. This implementation only processes a single block at a time, so, in reality, only
//! 512 bits are processed at once and the remaining 512 bits of the variables are unused. The 2nd
//! implementation uses u32x4s - vectors of 4 u32s. Thus, we can process 8 * 128 = 4096 bits at once,
//! which corresponds exactly to 8 blocks.
//!
//! The Bs8State struct implements the AesOps trait, which contains methods for each of the 4 main steps
//! of the AES algorithm. The types, T, each implement the AesBitValueOps trait, which containts methods
//! necessary for processing a collection or bit values and the AesOps trait relies heavily on this
//! trait to perform its operations.
//!
//! The Bs4State and Bs2State struct implement operations of various subfields of the full GF(2^(8))
//! finite field which allows for efficient computation of the AES S-Boxes. See [7] for details.
//!
//! ## References
//!
//! [1]: http://www.jbonneau.com/doc/BM06-CHES-aes_cache_timing.pdf '"Cache-Collision Timing Attacks Against AES". Joseph Bonneau and Ilya Mironov.'
//! [2]: http://eprint.iacr.org/2006/052.pdf '"Software mitigations to hedge AES against cache-based software side channel vulnerabilities". Ernie Brickell, et al.'
//! [3]: http://tau.ac.il/~tromer/papers/cache.pdf '"Cache Attacks and Countermeasures: the Case of AES (Extended Version)". Dag Arne Osvik, et al.'
//! [4]: http://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.52.5429&rep=rep1&type=pdf '"A Fast New DES Implementation in Software". Eli Biham.'
//! [5]: http://www.chesworkshop.org/ches2009/presentations/01_Session_1/CHES2009_ekasper.pdf '"Faster and Timing-Attack Resistant AES-GCM". Emilia K ̈asper and Peter Schwabe.'
//! [6]: http://webcache.googleusercontent.com/search?q=cache:ld_f8pSgURcJ:csusdspace.calstate.edu/bitstream/handle/10211.9/1224/Vinit_Azad_MS_Report.doc%3Fsequence%3D2+&cd=4&hl=en&ct=clnk&gl=us&client=ubuntu '"FAST AES DECRYPTION". Vinit Azad.'
//! [7]: http://www.dtic.mil/cgi-bin/GetTRDoc?AD=ADA434781 '"A Very Compact Rijndael S-box". D. Canright.'

use std::ops::{BitAnd, BitXor, Not};

use byteorder::{LittleEndian, ByteOrder};
use typenum::consts::U16;

use block::{BlockEncrypt, BlockDecrypt};

use self::simd::*;
use self::gf::*;

mod simd;
mod gf;

macro_rules! define_aes_struct {
    ($name:ident, $rounds:expr) => {
        #[derive(Clone, Copy)]
        pub struct $name {
            sk: [Gf8<u16>; ($rounds + 1)]
        }
    }
}

macro_rules! define_aes_impl {
    ($name:ident, $mode:ident, $rounds:expr, $key_size:expr) => {
        impl $name {
            pub fn new(key: &[u8]) -> $name {
                let mut a = $name {
                    sk: [Gf8::default(); ($rounds + 1)]
                };
                let mut tmp = [[0u32; 4]; ($rounds + 1)];
                create_round_keys(key, KeyType::$mode, &mut tmp);
                for (subkey, tmp) in a.sk.iter_mut().zip(&tmp) {
                    *subkey = bit_slice_4x4_with_u16(tmp[0], tmp[1], tmp[2], tmp[3]);
                }
                a
            }
        }
    }
}

macro_rules! define_aes_enc {
    ($name:ident, $rounds:expr) => {
        impl BlockEncrypt for $name {
            type BlockSize = U16;

            fn encrypt_block<I, O>(&self, input: I, mut output: O)
                where I: AsRef<[u8]>,
                      O: AsMut<[u8]>
                {
                    let mut bs = bit_slice_1x16_with_u16(input.as_ref());
                    bs = encrypt_core(&bs, &self.sk);
                    un_bit_slice_1x16_with_u16(&bs, output.as_mut());
                }
        }
    }
}

macro_rules! define_aes_dec {
    ($name:ident, $rounds:expr) => {
        impl BlockDecrypt for $name {
            type BlockSize = U16;

            fn decrypt_block<I, O>(&self, input: I, mut output: O)
                where I: AsRef<[u8]>,
                      O: AsMut<[u8]>
                {
                    let mut bs = bit_slice_1x16_with_u16(input.as_ref());
                    bs = decrypt_core(&bs, &self.sk);
                    un_bit_slice_1x16_with_u16(&bs, output.as_mut());
                }
        }
    }
}

define_aes_struct!(AesSafe128Encryptor, 10);
define_aes_struct!(AesSafe128Decryptor, 10);
define_aes_impl!(AesSafe128Encryptor, Encryption, 10, 16);
define_aes_impl!(AesSafe128Decryptor, Decryption, 10, 16);
define_aes_enc!(AesSafe128Encryptor, 10);
define_aes_dec!(AesSafe128Decryptor, 10);

define_aes_struct!(AesSafe192Encryptor, 12);
define_aes_struct!(AesSafe192Decryptor, 12);
define_aes_impl!(AesSafe192Encryptor, Encryption, 12, 24);
define_aes_impl!(AesSafe192Decryptor, Decryption, 12, 24);
define_aes_enc!(AesSafe192Encryptor, 12);
define_aes_dec!(AesSafe192Decryptor, 12);

define_aes_struct!(AesSafe256Encryptor, 14);
define_aes_struct!(AesSafe256Decryptor, 14);
define_aes_impl!(AesSafe256Encryptor, Encryption, 14, 32);
define_aes_impl!(AesSafe256Decryptor, Decryption, 14, 32);
define_aes_enc!(AesSafe256Encryptor, 14);
define_aes_dec!(AesSafe256Decryptor, 14);

macro_rules! define_aes_struct_x8 {
    ($name:ident, $rounds:expr) => {
        #[derive(Clone, Copy)]
        pub struct $name {
            sk: [Gf8<u32x4>; ($rounds + 1)]
        }
    }
}

macro_rules! define_aes_impl_x8 {
    ($name:ident, $mode:ident, $rounds:expr, $key_size:expr) => {
        impl $name {
            pub fn new(key: &[u8]) -> $name {
                let mut a =  $name {
                    sk: [Gf8::default(); ($rounds + 1)]
                };
                let mut tmp = [[0u32; 4]; ($rounds + 1)];
                create_round_keys(key, KeyType::$mode, &mut tmp);
                for i in 0..$rounds + 1 {
                    a.sk[i] = bit_slice_fill_4x4_with_u32x4(
                        tmp[i][0],
                        tmp[i][1],
                        tmp[i][2],
                        tmp[i][3]);
                }
                a
            }
        }
    }
}

macro_rules! define_aes_enc_x8 {
    ($name:ident, $rounds:expr) => {
        impl BlockEncryptorX8 for $name {
            fn block_size(&self) -> usize { 16 }
            fn encrypt_block_x8(&self, input: &[u8], output: &mut [u8]) {
                let bs = bit_slice_1x128_with_u32x4(input);
                let bs2 = encrypt_core(&bs, &self.sk);
                un_bit_slice_1x128_with_u32x4(bs2, output);
            }
        }
    }
}

macro_rules! define_aes_dec_x8 {
    ( $name:ident, $rounds:expr) => {
        impl BlockDecryptorX8 for $name {
            fn block_size(&self) -> usize { 16 }
            fn decrypt_block_x8(&self, input: &[u8], output: &mut [u8]) {
                let bs = bit_slice_1x128_with_u32x4(input);
                let bs2 = decrypt_core(&bs, &self.sk);
                un_bit_slice_1x128_with_u32x4(bs2, output);
            }
        }
    }
}

// define_aes_struct_x8!(AesSafe128EncryptorX8, 10);
// define_aes_struct_x8!(AesSafe128DecryptorX8, 10);
// define_aes_impl_x8!(AesSafe128EncryptorX8, Encryption, 10, 16);
// define_aes_impl_x8!(AesSafe128DecryptorX8, Decryption, 10, 16);
// define_aes_enc_x8!(AesSafe128EncryptorX8, 10);
// define_aes_dec_x8!(AesSafe128DecryptorX8, 10);

// define_aes_struct_x8!(AesSafe192EncryptorX8, 12);
// define_aes_struct_x8!(AesSafe192DecryptorX8, 12);
// define_aes_impl_x8!(AesSafe192EncryptorX8, Encryption, 12, 24);
// define_aes_impl_x8!(AesSafe192DecryptorX8, Decryption, 12, 24);
// define_aes_enc_x8!(AesSafe192EncryptorX8, 12);
// define_aes_dec_x8!(AesSafe192DecryptorX8, 12);

// define_aes_struct_x8!(AesSafe256EncryptorX8, 14);
// define_aes_struct_x8!(AesSafe256DecryptorX8, 14);
// define_aes_impl_x8!(AesSafe256EncryptorX8, Encryption, 14, 32);
// define_aes_impl_x8!(AesSafe256DecryptorX8, Decryption, 14, 32);
// define_aes_enc_x8!(AesSafe256EncryptorX8, 14);
// define_aes_dec_x8!(AesSafe256DecryptorX8, 14);

fn ffmulx(x: u32) -> u32 {
    let m1: u32 = 0x80808080;
    let m2: u32 = 0x7f7f7f7f;
    let m3: u32 = 0x0000001b;
    ((x & m2) << 1) ^ (((x & m1) >> 7) * m3)
}

fn inv_mcol(x: u32) -> u32 {
    let f2 = ffmulx(x);
    let f4 = ffmulx(f2);
    let f8 = ffmulx(f4);
    let f9 = x ^ f8;

    f2 ^ f4 ^ f8 ^ (f2 ^ f9).rotate_right(8) ^ (f4 ^ f9).rotate_right(16) ^ f9.rotate_right(24)
}

fn sub_word(x: u32) -> u32 {
    let bs = bit_slice_4x1_with_u16(x).sub_bytes();
    un_bit_slice_4x1_with_u16(&bs)
}

enum KeyType {
    Encryption,
    Decryption,
}

// This array is not accessed in any key-dependant way, so there are no timing problems inherent in
// using it.
const RCON: [u32; 10] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];

// The round keys are created without bit-slicing the key data. The individual implementations bit
// slice the round keys returned from this function. This function, and the few functions above, are
// derived from the BouncyCastle AES implementation.
fn create_round_keys(key: &[u8], key_type: KeyType, round_keys: &mut [[u32; 4]]) {
    let (key_words, rounds) = match key.len() {
        16 => (4, 10),
        24 => (6, 12),
        32 => (8, 14),
        _ => panic!("Invalid AES key size."),
    };

    // The key is copied directly into the first few round keys
    for (i, subkey) in key.chunks(4).enumerate() {
        round_keys[i / 4][i % 4] = (subkey[0] as u32) |
                                   ((subkey[1] as u32) << 8 | (subkey[2] as u32) << 16 |
                                    (subkey[3] as u32) << 24);
    }

    // Calculate the rest of the round keys
    for i in key_words..(rounds + 1) * 4 {
        let mut tmp = round_keys[(i - 1) / 4][(i - 1) % 4];
        if (i % key_words) == 0 {
            tmp = sub_word(tmp.rotate_right(8)) ^ RCON[(i / key_words) - 1];
        } else if (key_words == 8) && ((i % key_words) == 4) {
            // This is only necessary for AES-256 keys
            tmp = sub_word(tmp);
        }
        round_keys[i / 4][i % 4] = round_keys[(i - key_words) / 4][(i - key_words) % 4] ^ tmp;
    }

    // Decryption round keys require extra processing
    if let KeyType::Decryption = key_type {
            for key in &mut round_keys[1..rounds] {
                for v in &mut key[..] {
                    *v = inv_mcol(*v);
                }
            }
    }
}

// This trait defines all of the operations needed for a type to be processed as part of an AES
// encryption or decryption operation.
trait AesOps {
    fn sub_bytes(self) -> Self;
    fn inv_sub_bytes(self) -> Self;

    fn shift_rows(self) -> Self;
    fn inv_shift_rows(self) -> Self;

    fn mix_columns(self) -> Self;
    fn inv_mix_columns(self) -> Self;

    fn add_round_key(self, rk: &Self) -> Self;
}

fn encrypt_core<S: AesOps + Copy>(state: &S, sk: &[S]) -> S {
    let last = sk.len() - 1;

    // Round 0 - add round key
    let mut tmp = state.add_round_key(&sk[0]);

    // Remaining rounds (except last round)
    for subkey in &sk[1..last] {
        tmp = tmp.sub_bytes();
        tmp = tmp.shift_rows();
        tmp = tmp.mix_columns();
        tmp = tmp.add_round_key(subkey);
    }

    // Last round
    tmp = tmp.sub_bytes();
    tmp = tmp.shift_rows();
    tmp = tmp.add_round_key(&sk[last]);

    tmp
}

fn decrypt_core<S: AesOps + Copy>(state: &S, sk: &[S]) -> S {
    let last = sk.len() - 1;

    // Round 0 - add round key
    let mut tmp = state.add_round_key(&sk[last]);

    // Remaining rounds (except last round)
    for subkey in sk[1..last].iter().rev() {
        tmp = tmp.inv_sub_bytes();
        tmp = tmp.inv_shift_rows();
        tmp = tmp.inv_mix_columns();
        tmp = tmp.add_round_key(subkey);
    }

    // Last round
    tmp = tmp.inv_sub_bytes();
    tmp = tmp.inv_shift_rows();
    tmp = tmp.add_round_key(&sk[0]);

    tmp
}

// Bit Slice data in the form of 4 u32s in column-major order
fn bit_slice_4x4_with_u16(a: u32, b: u32, c: u32, d: u32) -> Gf8<u16> {
    fn pb(x: u32, bit: u32, shift: u32) -> u16 {
        (((x >> bit) & 1) as u16) << shift
    }

    fn construct(a: u32, b: u32, c: u32, d: u32, bit: u32) -> u16 {
        pb(a, bit, 0) | pb(b, bit, 1) | pb(c, bit, 2) | pb(d, bit, 3) | pb(a, bit + 8, 4) |
        pb(b, bit + 8, 5) | pb(c, bit + 8, 6) | pb(d, bit + 8, 7) |
        pb(a, bit + 16, 8) | pb(b, bit + 16, 9) |
        pb(c, bit + 16, 10) | pb(d, bit + 16, 11) | pb(a, bit + 24, 12) |
        pb(b, bit + 24, 13) | pb(c, bit + 24, 14) | pb(d, bit + 24, 15)
    }

    let x0 = construct(a, b, c, d, 0);
    let x1 = construct(a, b, c, d, 1);
    let x2 = construct(a, b, c, d, 2);
    let x3 = construct(a, b, c, d, 3);
    let x4 = construct(a, b, c, d, 4);
    let x5 = construct(a, b, c, d, 5);
    let x6 = construct(a, b, c, d, 6);
    let x7 = construct(a, b, c, d, 7);

    Gf8(x0, x1, x2, x3, x4, x5, x6, x7)
}

// Bit slice a single u32 value - this is used to calculate the SubBytes step when creating the
// round keys.
fn bit_slice_4x1_with_u16(a: u32) -> Gf8<u16> {
    bit_slice_4x4_with_u16(a, 0, 0, 0)
}

// Bit slice a 16 byte array in column major order
fn bit_slice_1x16_with_u16(data: &[u8]) -> Gf8<u16> {
    let a = LittleEndian::read_u32(&data[0..4]);
    let b = LittleEndian::read_u32(&data[4..8]);
    let c = LittleEndian::read_u32(&data[8..12]);
    let d = LittleEndian::read_u32(&data[12..16]);

    bit_slice_4x4_with_u16(a, b, c, d)
}

// Un Bit Slice into a set of 4 u32s
fn un_bit_slice_4x4_with_u16(bs: &Gf8<u16>) -> (u32, u32, u32, u32) {
    fn pb(x: u16, bit: u32, shift: u32) -> u32 {
        (((x >> bit) & 1) as u32) << shift
    }

    fn deconstruct(bs: &Gf8<u16>, bit: u32) -> u32 {
        let Gf8(x0, x1, x2, x3, x4, x5, x6, x7) = *bs;

        pb(x0, bit, 0) | pb(x1, bit, 1) | pb(x2, bit, 2) | pb(x3, bit, 3) | pb(x4, bit, 4) |
        pb(x5, bit, 5) | pb(x6, bit, 6) | pb(x7, bit, 7) |
        pb(x0, bit + 4, 8) | pb(x1, bit + 4, 9) | pb(x2, bit + 4, 10) |
        pb(x3, bit + 4, 11) | pb(x4, bit + 4, 12) | pb(x5, bit + 4, 13) |
        pb(x6, bit + 4, 14) |
        pb(x7, bit + 4, 15) | pb(x0, bit + 8, 16) | pb(x1, bit + 8, 17) |
        pb(x2, bit + 8, 18) | pb(x3, bit + 8, 19) |
        pb(x4, bit + 8, 20) | pb(x5, bit + 8, 21) | pb(x6, bit + 8, 22) |
        pb(x7, bit + 8, 23) | pb(x0, bit + 12, 24) |
        pb(x1, bit + 12, 25) | pb(x2, bit + 12, 26) |
        pb(x3, bit + 12, 27) | pb(x4, bit + 12, 28) |
        pb(x5, bit + 12, 29) | pb(x6, bit + 12, 30) | pb(x7, bit + 12, 31)
    }

    let a = deconstruct(bs, 0);
    let b = deconstruct(bs, 1);
    let c = deconstruct(bs, 2);
    let d = deconstruct(bs, 3);

    (a, b, c, d)
}

// Un Bit Slice into a single u32. This is used when creating the round keys.
fn un_bit_slice_4x1_with_u16(bs: &Gf8<u16>) -> u32 {
    un_bit_slice_4x4_with_u16(bs).0
}

// Un Bit Slice into a 16 byte array
fn un_bit_slice_1x16_with_u16(bs: &Gf8<u16>, output: &mut [u8]) {
    let (a, b, c, d) = un_bit_slice_4x4_with_u16(bs);

    LittleEndian::write_u32(&mut output[0..4], a);
    LittleEndian::write_u32(&mut output[4..8], b);
    LittleEndian::write_u32(&mut output[8..12], c);
    LittleEndian::write_u32(&mut output[12..16], d);
}

// Bit Slice a 128 byte array of eight 16 byte blocks. Each block is in column major order.
fn bit_slice_1x128_with_u32x4(data: &[u8]) -> Gf8<u32x4> {
    let bit0 = u32x4::filled(0x01010101);
    let bit1 = u32x4::filled(0x02020202);
    let bit2 = u32x4::filled(0x04040404);
    let bit3 = u32x4::filled(0x08080808);
    let bit4 = u32x4::filled(0x10101010);
    let bit5 = u32x4::filled(0x20202020);
    let bit6 = u32x4::filled(0x40404040);
    let bit7 = u32x4::filled(0x80808080);

    let t0 = u32x4::read_row_major(&data[0..16]);
    let t1 = u32x4::read_row_major(&data[16..32]);
    let t2 = u32x4::read_row_major(&data[32..48]);
    let t3 = u32x4::read_row_major(&data[48..64]);
    let t4 = u32x4::read_row_major(&data[64..80]);
    let t5 = u32x4::read_row_major(&data[80..96]);
    let t6 = u32x4::read_row_major(&data[96..112]);
    let t7 = u32x4::read_row_major(&data[112..128]);

    let x0 = (t0 & bit0) | (t1.rotate_left(1) & bit1) | (t2.rotate_left(2) & bit2) |
             (t3.rotate_left(3) & bit3) | (t4.rotate_left(4) & bit4) |
             (t5.rotate_left(5) & bit5) | (t6.rotate_left(6) & bit6) |
             (t7.rotate_left(7) & bit7);
    let x1 = (t0.rotate_right(1) & bit0) | (t1 & bit1) | (t2.rotate_left(1) & bit2) |
             (t3.rotate_left(2) & bit3) | (t4.rotate_left(3) & bit4) |
             (t5.rotate_left(4) & bit5) | (t6.rotate_left(5) & bit6) |
             (t7.rotate_left(6) & bit7);
    let x2 = (t0.rotate_right(2) & bit0) | (t1.rotate_right(1) & bit1) | (t2 & bit2) |
             (t3.rotate_left(1) & bit3) | (t4.rotate_left(2) & bit4) |
             (t5.rotate_left(3) & bit5) | (t6.rotate_left(4) & bit6) |
             (t7.rotate_left(5) & bit7);
    let x3 = (t0.rotate_right(3) & bit0) | (t1.rotate_right(2) & bit1) |
             (t2.rotate_right(1) & bit2) | (t3 & bit3) | (t4.rotate_left(1) & bit4) |
             (t5.rotate_left(2) & bit5) |
             (t6.rotate_left(3) & bit6) | (t7.rotate_left(4) & bit7);
    let x4 = (t0.rotate_right(4) & bit0) | (t1.rotate_right(3) & bit1) |
             (t2.rotate_right(2) & bit2) | (t3.rotate_right(1) & bit3) |
             (t4 & bit4) | (t5.rotate_left(1) & bit5) | (t6.rotate_left(2) & bit6) |
             (t7.rotate_left(3) & bit7);
    let x5 = (t0.rotate_right(5) & bit0) | (t1.rotate_right(4) & bit1) |
             (t2.rotate_right(3) & bit2) | (t3.rotate_right(2) & bit3) |
             (t4.rotate_right(1) & bit4) | (t5 & bit5) | (t6.rotate_left(1) & bit6) |
             (t7.rotate_left(2) & bit7);
    let x6 = (t0.rotate_right(6) & bit0) | (t1.rotate_right(5) & bit1) |
             (t2.rotate_right(4) & bit2) | (t3.rotate_right(3) & bit3) |
             (t4.rotate_right(2) & bit4) | (t5.rotate_right(1) & bit5) | (t6 & bit6) |
             (t7.rotate_left(1) & bit7);
    let x7 = (t0.rotate_right(7) & bit0) | (t1.rotate_right(6) & bit1) |
             (t2.rotate_right(5) & bit2) | (t3.rotate_right(4) & bit3) |
             (t4.rotate_right(3) & bit4) | (t5.rotate_right(2) & bit5) |
             (t6.rotate_right(1) & bit6) | (t7 & bit7);

    Gf8(x0, x1, x2, x3, x4, x5, x6, x7)
}

// Bit slice a set of 4 u32s by filling a full 128 byte data block with those repeated values. This
// is used as part of bit slicing the round keys.
fn bit_slice_fill_4x4_with_u32x4(a: u32, b: u32, c: u32, d: u32) -> Gf8<u32x4> {
    let mut tmp = [0u8; 128];
    for i in 0..8 {
        LittleEndian::write_u32(&mut tmp[i * 16..i * 16 + 4], a);
        LittleEndian::write_u32(&mut tmp[i * 16 + 4..i * 16 + 8], b);
        LittleEndian::write_u32(&mut tmp[i * 16 + 8..i * 16 + 12], c);
        LittleEndian::write_u32(&mut tmp[i * 16 + 12..i * 16 + 16], d);
    }
    bit_slice_1x128_with_u32x4(&tmp)
}

// Un bit slice into a 128 byte buffer.
fn un_bit_slice_1x128_with_u32x4(bs: Gf8<u32x4>, output: &mut [u8]) {
    let Gf8(t0, t1, t2, t3, t4, t5, t6, t7) = bs;

    let bit0 = u32x4::filled(0x01010101);
    let bit1 = u32x4::filled(0x02020202);
    let bit2 = u32x4::filled(0x04040404);
    let bit3 = u32x4::filled(0x08080808);
    let bit4 = u32x4::filled(0x10101010);
    let bit5 = u32x4::filled(0x20202020);
    let bit6 = u32x4::filled(0x40404040);
    let bit7 = u32x4::filled(0x80808080);

    // decode the individual blocks, in row-major order
    // TODO: this is identical to the same block in bit_slice_1x128_with_u32x4
    let x0 = (t0 & bit0) | (t1.rotate_left(1) & bit1) | (t2.rotate_left(2) & bit2) |
             (t3.rotate_left(3) & bit3) | (t4.rotate_left(4) & bit4) |
             (t5.rotate_left(5) & bit5) | (t6.rotate_left(6) & bit6) |
             (t7.rotate_left(7) & bit7);
    let x1 = (t0.rotate_right(1) & bit0) | (t1 & bit1) | (t2.rotate_left(1) & bit2) |
             (t3.rotate_left(2) & bit3) | (t4.rotate_left(3) & bit4) |
             (t5.rotate_left(4) & bit5) | (t6.rotate_left(5) & bit6) |
             (t7.rotate_left(6) & bit7);
    let x2 = (t0.rotate_right(2) & bit0) | (t1.rotate_right(1) & bit1) | (t2 & bit2) |
             (t3.rotate_left(1) & bit3) | (t4.rotate_left(2) & bit4) |
             (t5.rotate_left(3) & bit5) | (t6.rotate_left(4) & bit6) |
             (t7.rotate_left(5) & bit7);
    let x3 = (t0.rotate_right(3) & bit0) | (t1.rotate_right(2) & bit1) |
             (t2.rotate_right(1) & bit2) | (t3 & bit3) | (t4.rotate_left(1) & bit4) |
             (t5.rotate_left(2) & bit5) |
             (t6.rotate_left(3) & bit6) | (t7.rotate_left(4) & bit7);
    let x4 = (t0.rotate_right(4) & bit0) | (t1.rotate_right(3) & bit1) |
             (t2.rotate_right(2) & bit2) | (t3.rotate_right(1) & bit3) |
             (t4 & bit4) | (t5.rotate_left(1) & bit5) | (t6.rotate_left(2) & bit6) |
             (t7.rotate_left(3) & bit7);
    let x5 = (t0.rotate_right(5) & bit0) | (t1.rotate_right(4) & bit1) |
             (t2.rotate_right(3) & bit2) | (t3.rotate_right(2) & bit3) |
             (t4.rotate_right(1) & bit4) | (t5 & bit5) | (t6.rotate_left(1) & bit6) |
             (t7.rotate_left(2) & bit7);
    let x6 = (t0.rotate_right(6) & bit0) | (t1.rotate_right(5) & bit1) |
             (t2.rotate_right(4) & bit2) | (t3.rotate_right(3) & bit3) |
             (t4.rotate_right(2) & bit4) | (t5.rotate_right(1) & bit5) | (t6 & bit6) |
             (t7.rotate_left(1) & bit7);
    let x7 = (t0.rotate_right(7) & bit0) | (t1.rotate_right(6) & bit1) |
             (t2.rotate_right(5) & bit2) | (t3.rotate_right(4) & bit3) |
             (t4.rotate_right(3) & bit4) | (t5.rotate_right(2) & bit5) |
             (t6.rotate_right(1) & bit6) | (t7 & bit7);

    x0.write_row_major(&mut output[0..16]);
    x1.write_row_major(&mut output[16..32]);
    x2.write_row_major(&mut output[32..48]);
    x3.write_row_major(&mut output[48..64]);
    x4.write_row_major(&mut output[64..80]);
    x5.write_row_major(&mut output[80..96]);
    x6.write_row_major(&mut output[96..112]);
    x7.write_row_major(&mut output[112..128])
}

// // The Gf2Ops, Gf4Ops, and Gf8Ops traits specify the functions needed to calculate the AES S-Box
// // values. This particuar implementation of those S-Box values is taken from [7], so that is where
// // to look for details on how all that all works. This includes the transformations matrices defined
// // below for the change_basis operation on the u32 and u32x4 types.

impl<T: AesBitValueOps + Copy> AesOps for Gf8<T> {
    fn sub_bytes(self) -> Self {
        let nb  = self.rebase::<A2X>();
        let inv = nb.inv();
        let nb2 = inv.rebase::<X2S>();
        nb2.xor_x63()
    }

    fn inv_sub_bytes(self) -> Self {
        let t = self.xor_x63();
        let nb = t.rebase::<S2X>();
        let inv = nb.inv();
        inv.rebase::<X2A>()
    }

    fn shift_rows(self) -> Self {
        let Gf8(x0, x1, x2, x3, x4, x5, x6, x7) = self;
        Gf8(x0.shift_row(),
                 x1.shift_row(),
                 x2.shift_row(),
                 x3.shift_row(),
                 x4.shift_row(),
                 x5.shift_row(),
                 x6.shift_row(),
                 x7.shift_row())
    }

    fn inv_shift_rows(self) -> Self {
        let Gf8(x0, x1, x2, x3, x4, x5, x6, x7) = self;
        Gf8(x0.inv_shift_row(),
                 x1.inv_shift_row(),
                 x2.inv_shift_row(),
                 x3.inv_shift_row(),
                 x4.inv_shift_row(),
                 x5.inv_shift_row(),
                 x6.inv_shift_row(),
                 x7.inv_shift_row())
    }

    // Formula from [5]
    fn mix_columns(self) -> Self {
        let Gf8(x0, x1, x2, x3, x4, x5, x6, x7) = self;

        let x0out = x7 ^ x7.ror1() ^ x0.ror1() ^ (x0 ^ x0.ror1()).ror2();
        let x1out = x0 ^ x0.ror1() ^ x7 ^ x7.ror1() ^ x1.ror1() ^ (x1 ^ x1.ror1()).ror2();
        let x2out = x1 ^ x1.ror1() ^ x2.ror1() ^ (x2 ^ x2.ror1()).ror2();
        let x3out = x2 ^ x2.ror1() ^ x7 ^ x7.ror1() ^ x3.ror1() ^ (x3 ^ x3.ror1()).ror2();
        let x4out = x3 ^ x3.ror1() ^ x7 ^ x7.ror1() ^ x4.ror1() ^ (x4 ^ x4.ror1()).ror2();
        let x5out = x4 ^ x4.ror1() ^ x5.ror1() ^ (x5 ^ x5.ror1()).ror2();
        let x6out = x5 ^ x5.ror1() ^ x6.ror1() ^ (x6 ^ x6.ror1()).ror2();
        let x7out = x6 ^ x6.ror1() ^ x7.ror1() ^ (x7 ^ x7.ror1()).ror2();

        Gf8(x0out, x1out, x2out, x3out, x4out, x5out, x6out, x7out)
    }

    // Formula from [6]
    fn inv_mix_columns(self) -> Self {
        let Gf8(x0, x1, x2, x3, x4, x5, x6, x7) = self;

        let x0out = x5 ^ x6 ^ x7 ^ (x5 ^ x7 ^ x0).ror1() ^ (x0 ^ x5 ^ x6).ror2() ^ (x5 ^ x0).ror3();
        let x1out = x5 ^ x0 ^ (x6 ^ x5 ^ x0 ^ x7 ^ x1).ror1() ^ (x1 ^ x7 ^ x5).ror2() ^
                    (x6 ^ x5 ^ x1).ror3();
        let x2out = x6 ^ x0 ^ x1 ^ (x7 ^ x6 ^ x1 ^ x2).ror1() ^ (x0 ^ x2 ^ x6).ror2() ^
                    (x7 ^ x6 ^ x2).ror3();
        let x3out = x0 ^ x5 ^ x1 ^ x6 ^ x2 ^ (x0 ^ x5 ^ x2 ^ x3).ror1() ^
                    (x0 ^ x1 ^ x3 ^ x5 ^ x6 ^ x7).ror2() ^
                    (x0 ^ x5 ^ x7 ^ x3).ror3();
        let x4out = x1 ^ x5 ^ x2 ^ x3 ^ (x1 ^ x6 ^ x5 ^ x3 ^ x7 ^ x4).ror1() ^
                    (x1 ^ x2 ^ x4 ^ x5 ^ x7).ror2() ^
                    (x1 ^ x5 ^ x6 ^ x4).ror3();
        let x5out = x2 ^ x6 ^ x3 ^ x4 ^ (x2 ^ x7 ^ x6 ^ x4 ^ x5).ror1() ^
                    (x2 ^ x3 ^ x5 ^ x6).ror2() ^ (x2 ^ x6 ^ x7 ^ x5).ror3();
        let x6out = x3 ^ x7 ^ x4 ^ x5 ^ (x3 ^ x7 ^ x5 ^ x6).ror1() ^ (x3 ^ x4 ^ x6 ^ x7).ror2() ^
                    (x3 ^ x7 ^ x6).ror3();
        let x7out = x4 ^ x5 ^ x6 ^ (x4 ^ x6 ^ x7).ror1() ^ (x4 ^ x5 ^ x7).ror2() ^ (x4 ^ x7).ror3();

        Gf8(x0out, x1out, x2out, x3out, x4out, x5out, x6out, x7out)
    }

    fn add_round_key(self, rk: &Self) -> Self {
        self + *rk
    }
}

trait AesBitValueOps: BitXor<Output = Self> + BitAnd<Output = Self> + Not<Output = Self> + Default + Sized {
    fn shift_row(self) -> Self;
    fn inv_shift_row(self) -> Self;
    fn ror1(self) -> Self;
    fn ror2(self) -> Self;
    fn ror3(self) -> Self;
}

impl AesBitValueOps for u16 {
    fn shift_row(self) -> Self {
        // first 4 bits represent first row - don't shift
        (self & 0x000f) | ((self & 0x00e0) >> 1) | ((self & 0x0010) << 3) |
        ((self & 0x0c00) >> 2) | ((self & 0x0300) << 2) | ((self & 0x8000) >> 3) |
        ((self & 0x7000) << 1)
    }

    fn inv_shift_row(self) -> Self {
        // first 4 bits represent first row - don't shift
        (self & 0x000f) | ((self & 0x0080) >> 3) | ((self & 0x0070) << 1) |
        ((self & 0x0c00) >> 2) | ((self & 0x0300) << 2) | ((self & 0xe000) >> 1) |
        ((self & 0x1000) << 3)
    }

    fn ror1(self) -> Self {
        self.rotate_right(4)
    }

    fn ror2(self) -> Self {
        self.rotate_right(8)
    }

    fn ror3(self) -> Self {
        self.rotate_right(12)
    }
}

impl AesBitValueOps for u32x4 {
    fn shift_row(self) -> u32x4 {
        u32x4(self.0,
              self.1.rotate_right(8),
              self.2.rotate_right(16),
              self.3.rotate_right(24))
    }

    fn inv_shift_row(self) -> u32x4 {
        u32x4(self.0,
              self.1.rotate_left(8),
              self.2.rotate_left(16),
              self.3.rotate_left(24))
    }

    fn ror1(self) -> u32x4 {
        u32x4(self.1, self.2, self.3, self.0)
    }

    fn ror2(self) -> u32x4 {
        u32x4(self.2, self.3, self.0, self.1)
    }

    fn ror3(self) -> u32x4 {
        u32x4(self.3, self.0, self.1, self.2)
    }
}