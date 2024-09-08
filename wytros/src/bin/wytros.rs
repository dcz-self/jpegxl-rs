use anyhow::Result;
use jpegxl_rs as jxl;
use libopenraw::{rawfile_from_file, Bitmap, CfaPattern, Image, RenderingOptions};
use log::LevelFilter;
use simple_logger::SimpleLogger;
use std::env;
use std::fs::File;
use std::io::Write;
use wytros::decode;

fn main() {
    run().unwrap();
}

fn bayer_to_rg1bg2(bayer: &[u16], width: u32, height: u32, mosaic_pattern: &CfaPattern) -> Vec<u16> {
    // Bayer sizing notation is different from rgb... only one channel per pixel in bayer.
    let out_w = width / 2;
    let out_h = height / 2;
    let mut out = Vec::with_capacity((out_h * out_w) as usize * 4);
    out.resize((out_w * out_h) as usize * 4, 0);
    assert_eq!(*mosaic_pattern, CfaPattern::Gbrg);
    let bayer_pitch = width;
    let rgb_pitch = out_w * 4;
    
    // iterating over the output to take advantage of SIMD, which needs a predictable write pattern
    out.iter_mut().enumerate()
        .map(|(i, out)| {
            let subpx = i % 4;
            let x = (i as u32 % rgb_pitch) / 4;
            let y = i as u32 / rgb_pitch;
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
            let bayer_row = (y * 2) | suby;
            let bayer_column = (x * 2) | subx;
            let bayer_index = bayer_row * bayer_pitch + bayer_column;
            if bayer_index as usize >= bayer.len() {
                dbg!(i);
                dbg!(bayer_index);
                dbg!(bayer.len());
                dbg!(bayer_row, y);
                dbg!(bayer_column, x);
                panic!();
            }
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
        let bayer_buffer = decode(&img.data8().unwrap())?;
        dbg!(bayer_buffer.len());
        let swizzled = bayer_to_rg1bg2(
            &bayer_buffer[..(img.width() as usize * img.height() as usize)],
            img.width(),
            img.height(),
            img.mosaic_pattern(),
        );
        dbg!(swizzled.len());
        
        let mut enc = jxl::encoder_builder()
            // we're compressing raw, duh
            .lossless(true)
            .uses_original_profile(true)
            .speed(jxl::encode::EncoderSpeed::Squirrel)//Tortoise)
            .use_container(true)
            // not really true for raw sensor data but doesn't hurt I guess.
            // I don't know what it changes apart from color profile in metadata anyway
            .color_encoding(jxl::encode::ColorEncoding::LinearSrgb)
            .build()?;
        let encoded = enc.encode::<u16, u16>(&swizzled[..], img.width() / 2, img.height() / 2)?;
        let mut out = File::create("/mnt/space/rhn/out.jxl")?;
        out.write_all(&encoded)?;
    }
    Ok(())
}