use anyhow::{Error, Result};
use std::cmp;

#[macro_export]
macro_rules! dh {
    // NOTE: We cannot use `concat!` to make a static string as a format argument
    // of `eprintln!` because `file!` could contain a `{` or
    // `$val` expression could be a block (`{ .. }`), in which case the `eprintln!`
    // will be malformed.
    () => {
        eprintln!("[{}:{}:{}]", file!(), line!(), column!());
    };
    ($val:expr $(,)?) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                eprintln!("[{}:{}:{}] {} = {:02x?}",
                    file!(), line!(), column!(), stringify!($val), &tmp);
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg_hex!($val)),+,)
    };
}

#[macro_export]
macro_rules! assert_eq_hex {
    ($left:expr, $right:expr $(,)?) => ({
        match (&$left, &$right) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    // The reborrows below are intentional. Without them, the stack slot for the
                    // borrow is initialized even before the values are compared, leading to a
                    // noticeable slow down.
                    panic!(r#"assertion `left == right` failed
  left: {:02x?}
 right: {:02x?}"#, &*left_val, &*right_val)
                }
            }
        }
    });
    ($left:expr, $right:expr, $($arg:tt)+) => ({
        match (&($left), &($right)) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    // The reborrows below are intentional. Without them, the stack slot for the
                    // borrow is initialized even before the values are compared, leading to a
                    // noticeable slow down.
                    panic!(r#"assertion `left == right` failed: {}
  left: {:02x?}
 right: {:02x?}"#, format_args!($($arg)+), &*left_val, &*right_val)
                }
            }
        }
    });
}


/// Converts chunk index to first byte offset within block
fn chunk_to_offset(idx: usize) -> usize {
    if idx > 0x200 {
        idx * 16 - 0x2008
    } else {
        idx * 16 + 0x1ff8
    }
}

/// Each block of 0x4000 bytes is split into 16-byte groups. First group starts in the middle of the block, reaching the end the groups wrap back to start of the block, splitting the boundary one into two halves.
fn block_get_chunk(data: &[u8], chunk_idx: usize) -> [u8; 16] {
    let block_idx = chunk_idx * 16 / 0x4000;
    let block = &data[block_idx * 0x4000..][..0x4000];
    let chunks_in_block = 0x4000 / 16;
    let chunk_idx = chunk_idx % chunks_in_block;
    let data_offset = chunk_to_offset(chunk_idx);
    let mut out = [0; 16];
    if data_offset == 0x3ff8 {
        out[0..8].copy_from_slice(&block[data_offset..][..8]);
        out[8..16].copy_from_slice(&block[0..8]);
    } else {
        out[0..16].copy_from_slice(&block[data_offset..][..16]);
    }
    out
}

fn iter_chunks(data: &[u8]) -> impl Iterator<Item=[u8; 16]> + '_ {
    (0..(data.len() / 0x4000)).map(|i| block_get_chunk(data, i))
}

macro_rules! to_lsb_mask {
    ($count:expr) => {
        !(!0 << count)
    };
}

#[derive(Debug, Clone)]
pub struct ReverseBits(pub [u8;16]);

impl ReverseBits {
    /// Gets up to 8 bits from the group. Starting with the last byte. Most significant bits of each byte go first into most significant bits of output. See test if this is confusing.
    pub fn get(&self, bit_index: usize, count: u8) -> u8 {
        let (data, byte_index, bit_offset) = self.get_internal(bit_index, count);
        let mask = !(!0u16 << count) as u8;
        (data >> bit_offset) as u8 & mask
    }
    
    fn get_internal(&self, bit_index: usize, count: u8) -> (u16, usize, usize) {
        let bit_index = 16*8 - bit_index - count as usize;
        let byte_index = bit_index / 8;
        let data
            = (*self.0.get(byte_index + 1).unwrap_or(&0) as u16) << 8
            | self.0[byte_index] as u16;
        let bit_offset = bit_index % 8;
        (data, byte_index, bit_offset)
    }
    
    pub fn set(&mut self, bit_index: usize, count: u8, value: u8) {
        let (data, byte_index, bit_offset) = self.get_internal(bit_index, count);
        let value = (value as u16) << bit_offset;
        let mask = !(!0u16 << count) << bit_offset;
        let data = ((data & !mask) | value).to_be_bytes();
        self.0.get_mut(byte_index + 1)
            .map(|v| *v = data[0]);
        self.0[byte_index] = data[1];
    }
}

fn decode_j(j: u16, shift: u8, prev: u16) -> u16 {
    let magnitude = 0x80 << shift;
    if j != 0 {
        // This is the lossy part. 
        if magnitude > prev || shift == 4 {
            // If shift > 0 then previous pixel data gets replaced, accidental LSBs get carried from old value.
            dh!((dh!(j) << shift) | (prev & !(!0 << shift)))
        } else {
            // If shift > 0 then the encoder dropped the LSBs.

            // Pretty-print for the actual difference value.
            // I'm not using this exact calculation to stay in u16
            // dh!((j << shift) as i16 - magnitude as i16);
            prev - magnitude + (dh!(j) << shift)
        }
    } else {
        prev
    }
}

fn decode_chunk(bits: ReverseBits) -> [u16; 14] {
    /* This is written in a non-streaming, immutable fashion: every access to bits calculates the position again.
     * The advantage is that the input data will never get globally out of sync when some data accidentally gets digested (although ReverseBits being bounded already controls that to an extent). The cost is all the indexing multipliers.
     * 
     * To convert to a correct streaming version, make sure that no stream read operations are conditional. The format doesn't call for it: the stream is always the same size and shape.
     * Conversion should be easy, it's already this way anyway.
    */
    let mut out = [0u16; 14];
    // 2 pixels stored losslessly
    out[0] = (bits.get(0, 8) as u16) << 4 | bits.get(8, 4) as u16;
    out[1] = (bits.get(12, 8) as u16) << 4 | bits.get(20, 4) as u16;
    // 4 independent differential groups in every chunk
    for diffidx in 0..4 {
        let shift = dbg!(bits.get(24 + diffidx * (2+3*8), 2));
        let shift = 4 >> (3 - shift);
        // 3 pixels in every group, chained to the previous pixel of the same color
        for pxidx in 0..3 {
            let px_allidx = 2 + diffidx * 3 + pxidx;
            let prev = out[px_allidx - 2];
            let j = bits.get(24 + 2 + diffidx * (2 + 3 * 8) + pxidx * 8, 8) as u16;
            let px = decode_j(dh!(j), dbg!(shift), dh!(prev));
            /* TODO: dcraw code does an odd thing:
             * it will read extra 4 bits for the last 2 pixels if there's all 0's in the chunk. This should send the stream out of whack.
             * The pana_bits reader strongly suggests that the stream of data is separated into 16-byte chunks, so reading another byte (or half-byte if interrupted) would contradict it.
            */
            out[px_allidx] = dh!(px);
        }
    }
    out
}

/// `pxs` is 2 previously encoded pixels + 3 to-be-encoded
fn calculate_shift(pxs: &[u16]) -> (u8, [i16; 3]) {
    let mut diffs = [0; 3];
    pxs[..3].iter()
        .zip(pxs[2..][..3].iter())
        .map(|(p, n)| *n as i16 - *p as i16)
        .zip(diffs.iter_mut())
        .for_each(|(d, o)| *o = d);
    let maxdiff = dh!(diffs).iter().map(|d| d.abs() as u16).max().unwrap();
    let maxpx = pxs[2..5].iter().max().unwrap();
    let px_shift = [2, 1, 0].into_iter()
        .filter(|shift| (0xffu16 << shift) > *maxpx)
        .next()
        .unwrap_or(4);
    let diff_magnitude = dh!((maxdiff+1).next_power_of_two());
    let shift = match diff_magnitude >> 8 {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 4,
    }; // if we tried to encode 4, then: 4 >> 3.saturating_sub((mag>>8) .neg().trailing_zeros())
    (cmp::min(shift, px_shift), diffs)
}

/// Fucking ENCODER. Because otherwise how do I reconstruct the original?
fn encode_chunk(pxs: &[u16; 14]) -> [u8; 16] {
    let mut bits = ReverseBits([0; 16]);
    // 2 pixels stored losslessly
    bits.set(0, 8, (pxs[0] >> 4) as u8);
    bits.set(8, 4, (pxs[0] & 0xf) as u8);
    bits.set(12, 8, (pxs[1] >> 4) as u8);
    bits.set(20, 4, (pxs[1] & 0xf) as u8);
    // 4 independent differential groups in every chunk
    for diffidx in 0..4 {
        let inpxs = &pxs[diffidx * 3..][..5];
        let mut outpxs = [inpxs[0], inpxs[1], 0, 0, 0];
        let (shift, diffs) = calculate_shift(dh!(inpxs));
        bits.set(24 + diffidx * (2+3*8), 2, cmp::min(shift, 3));
        let magnitude = 0x80 << shift;
        // 3 pixels in every group, chained to the previous pixel of the same color
        for (px_diffidx, diff) in diffs.iter().enumerate() {
            let prev = outpxs[px_diffidx];
            let px = inpxs[px_diffidx + 2];
            let j = if dh!(prev) < magnitude || shift == 4 {
                px >> shift
            } else {
                ((diff + magnitude as i16) as u16) >> shift
            };
            bits.set(24 + 2 + diffidx * (2 + 3 * 8) + px_diffidx * 8, 8, j as u8);
            outpxs[px_diffidx + 2] = decode_j(dh!(j), dbg!(shift), dh!(prev));
        }
    }
    bits.0
}

fn compare(a: &[u8; 16], b: &[u8; 16]) {
    let a = ReverseBits(a.clone());
    let b = ReverseBits(b.clone());
    
    let g = |i, c, d| assert_eq_hex!(a.get(i, c), b.get(i, c), "test {}", d);
    for diffidx in 0..4 {
        g(24+diffidx * (2+3*8), 2, diffidx);
    }
}

pub fn decode(data: &[u8]) -> Result<Vec<u16>>{
    if data.len() % 0x4000 == 0 {
        let mut out = Vec::with_capacity(data.len() * 14 / 16);
        iter_chunks(data)
            .enumerate()
            .map(|(i, data)| {
                dh!(i);
                let bits = ReverseBits(dh!(data));
                let out = decode_chunk(bits);
                assert_eq_hex!(&data, &encode_chunk(&out));
                out
            })
            .for_each(|chunk| out.extend_from_slice(&chunk[..]));
        Ok(out)
    } else {
        Err(Error::msg(format!("Bad size {}", data.len())))
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use assert_matches::assert_matches;

    #[test]
    fn revbits() {
        let mut ar = [0; 16];
        ar[15] = 0x0b;
        ar[14] = 0xf0;
        ar[13] = 0xc6;
        ar[12] = 0x20;
        ar[11] = 0x1f;
        let ar = ReverseBits(ar);
        assert_eq!(ar.get(0, 8), 0x0b);
        assert_eq!(ar.get(8, 4), 0xf);
        assert_eq!(ar.get(12, 8), 0x0c);
        assert_eq!(ar.get(20, 4), 0x6);
        assert_eq!(ar.get(24, 2), 0x0);
        assert_eq!(ar.get(26, 8), 0x80);
    }
    
    #[test]
    fn revset() {
        let mut ar = ReverseBits([0;16]);
        ar.set(0, 8, 0x0b);
        assert_eq!(ar.get(0, 8), 0x0b);
        ar.set(8, 4, 0xf);
        assert_eq!(ar.get(8, 4), 0xf);
        ar.set(12, 8, 0x0c);
        assert_eq!(ar.get(12, 8), 0x0c);
        ar.set(20, 4, 0x6);
        assert_eq!(ar.get(20, 4), 0x6);
        ar.set(24, 2, 0x0);
        assert_eq!(ar.get(24, 2), 0x0);
        ar.set(26, 8, 0x80);
        assert_eq!(ar.get(26, 8), 0x80);
    }
    
    #[test]
    fn cto() {
        assert_eq!(chunk_to_offset(0), 0x1ff8);
        assert_eq!(chunk_to_offset(0x200), 0x3ff8);
        assert_eq!(chunk_to_offset(0x201), 0x8);
        assert_eq!(chunk_to_offset(0x3ff), 0x1fe8);
    }
    
    #[test]
    fn decode() {
        let ar = ReverseBits([0x90, 0x7A, 0x8A, 0x18, 0x02, 0x26, 0x92, 0xC7, 0xB7, 0x48, 0x20, 0x1F, 0x20, 0xC6, 0xF0, 0x0B]);
        let pixels = decode_chunk(ar);
        assert_eq!(
            pixels,
            [0xbf, 0xc6, 0xbf, 0xc2, 0xc0, 0xcd, 0xbc, 0xc6, 0xc5, 0xc6, 0xcb, 0xd0, 0xc5, 0xe0],
            "{:#x?}", &pixels,
        );
        
        let ar = ReverseBits([0x66, 0x73, 0xd2, 0x21, 0x22, 0x1d, 0xc9, 0x24, 0xd2, 0x55, 0x9a, 0x70, 0x7a, 0x4b, 0xf1, 0x17]);
        let pixels = decode_chunk(ar);
        assert_eq!(
            pixels,
            [0x17f, 0x14b, 0x251, 0x1cf, 0x223, 0x189, 0x167, 0x121, 0x11f, 0x121, 0x223, 0x1c5, 0x209, 0x191],
            "{:#x?}", &pixels,
        );
    }
    
    #[test]
    fn reencode() {
        let ar = [0x90, 0x7A, 0x8A, 0x18, 0x02, 0x26, 0x92, 0xC7, 0xB7, 0x48, 0x20, 0x1F, 0x20, 0xC6, 0xF0, 0x0B];
        
        assert_eq_hex!(
            encode_chunk(&decode_chunk(ReverseBits(ar))),
            ar,
        );
    }
    
    #[test]
    fn reencode1() {
        let ar = [0x21, 0x16, 0x47, 0x8f, 0x2d, 0x09, 0xa1, 0x26, 0x29, 0x6c, 0x61, 0x17, 0x30, 0xaf, 0xd3, 0x17];
        
        assert_eq_hex!(
            encode_chunk(&decode_chunk(ReverseBits(ar))),
            ar,
        );
    }
    
    #[test]
    fn reencode2() {
        let ar = [0x89, 0x91, 0x7a, 0xe8, 0x11, 0xf6, 0x31, 0x59, 0x88, 0x84, 0x5f, 0xbb, 0xac, 0x01, 0x90, 0x15];
        
        assert_eq_hex!(
            encode_chunk(&decode_chunk(ReverseBits(ar))),
            ar,
        );
    }
    
    #[test]
    fn reencode3() {
        let ar = [0x74, 0x89, 0x7f, 0xb0, 0x01, 0x1e, 0x52, 0x58, 0x57, 0x89, 0xa0, 0x6b, 0xf4, 0x01, 0xd0, 0x11];
        
        assert_eq_hex!(
            encode_chunk(&decode_chunk(ReverseBits(ar))),
            ar,
        );
    }
    
    #[test]
    fn enc_diff_shift() {
        assert_matches!(calculate_shift(&[0xbf, 0xc6, 0xbf, 0xc2, 0xc0][..]), (0, _));
        assert_matches!(calculate_shift(&[0xc2, 0xc0, 0xcd, 0xbc, 0xc6][..]), (0, _));
        assert_matches!(calculate_shift(&[0xbc, 0xc6, 0xc5, 0xc6, 0xcb][..]), (0, _));
        assert_matches!(calculate_shift(&[0xc6, 0xcb, 0xd0, 0xc5, 0xe0][..]), (0, _));
    }
    #[test]
    fn enc_diff_shift2() {
        assert_matches!(calculate_shift(&[0x3c1, 0x312, 0x3a9, 0x2f7, 0x3f1][..]), (0, _));
    }
    #[test]
    fn enc_diff_shift3() {
        assert_matches!(calculate_shift(&[0x20f, 0x17f, 0x1af, 0x197, 0x2c3][..]), (2, _));
    }
    #[test]
    fn enc_diff_shift4() {
        // raw mag = 0x400, shift = 4
        //diffs = [170, 3b4, fff8]
        // j recalculation does not overflow when shift is reduced
        // how does it even succeed with shift=2? 0x3b4 is > 0x200
        // but 0xff << 2 = 0x3fc so ok in replacement mode
        assert_matches!(calculate_shift(&[0x159, 0x01, 0x2c9, 0x3b5, 0x2c1][..]), (2, _));
    }
    #[test]
    fn enc_diff_shift5() {
        // raw mag = 0x200
        assert_matches!(calculate_shift(&[0x167, 0x121, 0x11f, 0x121, 0x223][..]), (2, _));
    }
    #[test]
    fn enc_diff_shift6() {
        // raw mag = 0x200
        assert_matches!(calculate_shift(&[0x2d8, 0x128, 0x1b4, 0xf0, 0x174][..]), (2, _));
    }
    #[test]
    fn enc_diff_shift7() {
        // raw mag = 0x400, shift = 4
        // diffs = [70, ff80, fda0]
        // j recalculation overflows when shift is reduced, unlike 4
        assert_matches!(calculate_shift(&[0x407, 0x1ef, 0x477, 0x16f, 0x217][..]), (4, _));
    }
}