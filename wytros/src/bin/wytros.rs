use anyhow::Result;
use jpegxl_rs as jxl;
use libopenraw::{rawfile_from_file, Bitmap, Image, RenderingOptions};
use log::LevelFilter;
use simple_logger::SimpleLogger;
use std::env;
use wytros::decode;

fn main() {
    run().unwrap();
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
        dbg!(decode(&img.data8().unwrap())?);
    }
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
    Ok(())
}