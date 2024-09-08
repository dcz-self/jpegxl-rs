use std::ops::Shl;

use anyhow::Result;

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
                eprintln!("[{}:{}:{}] {} = {:#x?}",
                    file!(), line!(), column!(), stringify!($val), &tmp);
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg_hex!($val)),+,)
    };
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
    }
    out
}

macro_rules! to_lsb_mask {
    ($count:expr) => {
        !(!0 << count)
    };
}

#[derive(Debug, Clone)]
struct ReverseBits([u8;16]);

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
        let shift = bits.get(24 + diffidx * (2+3*8), 2);
        let shift = 4 >> (3 - shift);
        let magnitude = 0x80 << shift;
        // 3 pixels in every group, chained to the previous pixel of the same color
        for pxidx in 0..3 {
            let px_allidx = 2 + diffidx * 3 + pxidx;
            let prev = out[px_allidx - 2];
            let j = bits.get(24 + 2 + diffidx * (2 + 3 * 8) + pxidx * 8, 8) as u16;
            let px = if j != 0 {
                // This is the lossy part. 
                if (magnitude > prev) | (shift == 4) {
                    // If shift > 0 then previous pixel data gets replaced, accidental LSBs get carried from old value.
                    j << shift | prev & !(!0 << shift)
                } else {
                    // If shift > 0 then the encoder dropped the LSBs
                    prev - magnitude + j << shift
                }
            } else {
                prev
            };
            /* TODO: dcraw code does an odd thing:
             * it will read extra 4 bits for the last 2 pixels if there's all 0's in the chunk. This should send the stream out of whack.
             * The pana_bits reader strongly suggests that the stream of data is separated into 16-byte chunks, so reading another byte (or half-byte if interrupted) would contradict it.
            */
            out[px_allidx] = px;
        }
    }
    out
}

fn encode_chunk(data: &[u16; 14]) -> [u8; 16] {
    todo!();
}

pub fn decode(data: &[u8]) -> Result<()>{
    panic!();
}

#[cfg(test)]
mod test {
    use crate::*;

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
    }
}