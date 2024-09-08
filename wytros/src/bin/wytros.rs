use anyhow::Result;
use jpegxl_rs as jxl;
use libopenraw::{rawfile_from_file, Bitmap, CfaPattern, Image, RenderingOptions};
use log::LevelFilter;
use simple_logger::SimpleLogger;
use std::env;
use wytros::decode;

fn main() {
    run().unwrap();
}

fn bayer_to_rg1bg2(bayer: &[u16], width: u32, mosaic_pattern: &CfaPattern) -> Vec<u16> {
    let mut out = Vec::with_capacity(dbg!(bayer.len()));
    out.resize(bayer.len(), 0);
    // iterating over the output to take advantage of SIMD, which needs a predictable write pattern
    assert_eq!(*mosaic_pattern, CfaPattern::Gbrg);
    let subpx_width = width * 4;
    out.iter_mut().enumerate()
        .map(|(i, out)| {
            let i = i as u32;
            let x = i % subpx_width;
            let y = i / subpx_width;
            let subpx = i % 4;
            /* R G1 B G2
             * ⇓
             * G1 B
             * R G2
             * 
             * This means double the rows, half the columns (still 2* pixels).
             */
            let (subx, suby) = match subpx {
                0 => (0, 1), // r
                1 => (0, 0), // g1,
                2 => (1, 0), // b,
                3 => (1, 1), // g2
                _ => unreachable!(),
            };
            let bayer_row = y * 2 | suby;
            let bayer_column = x * 2 | subx;
            let bayer_index = bayer_row * width * 2 + bayer_column;
            (bayer_index as usize, out)
        })
        .for_each(|(idx, out)| *out = bayer[idx]);
    out
}

fn run() -> Result<()> {
    SimpleLogger::new()
        .with_module_level("libopenraw", LevelFilter::Debug)
        .init()
        .unwrap();
    let path = env::args().skip(1).next().unwrap();
    if let Ok(rawfile) = rawfile_from_file(path, None) {
        let img = rawfile.raw_data(false)?;
        dbg!(img.active_area());
        dbg!(img.mosaic_pattern());
        dbg!(img.data_type());
        dbg!(img.compression());
        dbg!(img.bpc());
        dbg!(img.data_size());
        dbg!(img.width() * img.height());
        dbg!(img.width());
        dbg!(img.height());
        dbg!(img.data8().unwrap().len());
        dbg!(&img.data8().unwrap()[..16]);
        let bayer_buffer = decode(&img.data8().unwrap())?;
        dbg!(bayer_buffer.len());
        let swizzled = bayer_to_rg1bg2(&bayer_buffer, img.width(), img.mosaic_pattern());
        
        let mut enc = jxl::encoder_builder()
            // we're compressing raw, duh
            .lossless(true)
            .uses_original_profile(true)
            .speed(jxl::encode::EncoderSpeed::Tortoise)
            .use_container(true)
            // not really true for raw sensor data but doesn't hurt I guess.
            // I don't know what it changes apart from color profile in metadata anyway
            .color_encoding(jxl::encode::ColorEncoding::LinearSrgb)
            .build()?;
        let encoded = enc.encode::<u16, u16>(&swizzled[..], img.width() / 2, img.height() / 2)?;
    }
    Ok(())
}