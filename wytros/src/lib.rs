use std::ops::Shl;

use anyhow::Result;

/// Converts chunk index to first byte offset within block
fn chunk_to_offset(idx: usize) -> usize {
    if idx > 0x200 {
        idx * 16 - 0x2008
    } else {
        idx * 16 + 0x1ff8
    }
}

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

struct ReverseBits([u8;16]);

impl ReverseBits {
    fn get(&self, bit_index: usize, count: u8) -> u8 {
        let bit_index = 16*8 - bit_index - count as usize;
        let byte_index = bit_index / 8;
        let data
            = (*self.0.get(byte_index + 1).unwrap_or(&0) as u16) << 8
            | self.0[byte_index] as u16;
        let bit_offset = bit_index % 8;
        let mask = !(!0u16 << count) as u8;
        (data >> bit_offset) as u8 & mask
    }
}

fn decode_chunk(bits: ReverseBits) -> [u16; 14] {
    let mut out = [0; u16];
    out[0] = bits.get(0, 8) << 4 | bits.get(8, 4);
    out[1] = bits.get(12, 8) << 4 | bits.get(20, 4);
    for diffidx in 0..4 {
        let shift = 4 >> (3 - bits.get(24 + diffidx * 18, 2));
        let magnitude = 0x80 << shift;
        for pxidx in 0..3 {
            let px_allidx = 2 + diffidx * 3 + pxidx;
            let prev = out[px_allidx - 2];
            let j = bits.get(24 + diffidx * 18 + pxidx * 8, 8) as u16;
            let px = if magnitude > prev | shift == 4 {
                j << shift | prev & !(!0 << shift)
            } else {
                prev - magnitude + j
            };
            out[px_allidx] = px;
        }
    }
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
        let ar = ReverseBits(ar);
        assert_eq!(ar.get(0, 8), 0x0b);
        assert_eq!(ar.get(8, 4), 0xf);
        assert_eq!(ar.get(12, 8), 0x0c);
        assert_eq!(ar.get(20, 4), 0x6);
    }
    
    #[test]
    fn cto() {
    assert_eq!(chunk_to_offset(0), 0x1ff8);
    assert_eq!(chunk_to_offset(0x200), 0x3ff8);
    assert_eq!(chunk_to_offset(0x201), 0x8);
    assert_eq!(chunk_to_offset(0x3ff), 0x1fe8);
    }
}