use jpegxl_rs as jxl;

fn main() {
    run().unwrap();
}
fn run() -> Result<(), ()> {
    let mut enc = jxl::encoder_builder()
        // we're compressing raw, duh
        .lossless(true)
        .uses_original_profile(true)
        .speed(jxl::EncoderSpeed::Tortoise)
        .use_container(true)
        // not really true for raw sensor data but doesn't hurt I guess.
        // I don't know what it changes apart from color profile in metadata anyway
        .color_encoding(jxl::ColorEncoding::LinearSRGB)
        .build()?;
    
}