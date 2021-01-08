/*
This file is part of jpegxl-rs.

jpegxl-rs is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

jpegxl-rs is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with jpegxl-rs.  If not, see <https://www.gnu.org/licenses/>.
*/

//! Common types used across the crate

use jpegxl_sys::*;

/// Pixel data type.
/// Currently u8, u16, u32 and f32 are supported.
pub trait PixelType: Clone + Default + 'static {
    /// Return the c const
    fn pixel_type() -> JxlDataType;
}
impl PixelType for u8 {
    fn pixel_type() -> JxlDataType {
        JxlDataType_JXL_TYPE_UINT8
    }
}
impl PixelType for u16 {
    fn pixel_type() -> JxlDataType {
        JxlDataType_JXL_TYPE_UINT16
    }
}
impl PixelType for u32 {
    fn pixel_type() -> JxlDataType {
        JxlDataType_JXL_TYPE_UINT32
    }
}
impl PixelType for f32 {
    fn pixel_type() -> JxlDataType {
        JxlDataType_JXL_TYPE_FLOAT
    }
}

/// Endinness
#[repr(C)]
#[derive(Copy, Clone)]
pub enum Endianness {
    /// Native Endian
    Native = 0,
    /// Little Endian
    Little,
    /// Big Endian
    Big,
}

impl From<Endianness> for u32 {
    fn from(endianness: Endianness) -> Self {
        endianness as u32
    }
}
