use std::ops::Shl;

use anyhow::Result;

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
}