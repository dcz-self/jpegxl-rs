/*! Generate a test pattern */
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
    let height = 16;
    let width = 16;
    
    let mut swizzled = Vec::<u16>::with_capacity(width * height * 4 as usize);
    for y in 0..(height as usize) {
        for i in 0..(width as usize) {
            swizzled.extend_from_slice(
                &if i < width / 4 {
                    [1024<<5,0,0]//,1<<15]
                } else if i < width / 2 {
                    [0,1024<<5,0]//,1<<15]
                } else if i < 3 * width / 4 {
                    [0,0, 1024<<5]//,1<<15]
                } else {
                    [1024,0,0]//,1<<15]
                }[..]
                //&[1<<15, 0,0, 1 << 15]
            );
                
            /*swizzled.push(if y % 2 == 0{
                (1 - (i as u16 % 2)) * 1024
            } else {
                0
            });*/
        }
    }
    //dbg!(&swizzled);

    
    let mut enc = jxl::encoder_builder()
        // we're compressing raw, duh
        .lossless(true)
        .uses_original_profile(true)
        .speed(jxl::encode::EncoderSpeed::Squirrel)//Tortoise)
        .use_container(true)
        // not really true for raw sensor data but doesn't hurt I guess.
        // I don't know what it changes apart from color profile in metadata anyway
        .color_encoding(jxl::encode::ColorEncoding::LinearSrgb)
        // this should be carried in a channel other than alpha, but let's use what's there for now
        //.has_alpha(true)
        .build()?;

    let frame = jxl::encode::EncoderFrame::new(&swizzled[..]).num_channels(3);
    let encoded = enc.encode_frame::<u16, u16>(&frame, width as u32, height as u32)?;
    let mut out = File::create("/mnt/space/rhn/out.jxl")?;
    out.write_all(&encoded)?;
    Ok(())
}