//! Lossless OpenJPEG fixtures cover ordinary, RESET and RESET+RESTART coding.

use console_core_jpeg_2000_files::{DecodeSettings, Image};

#[test]
fn mq_coding_and_context_resets_preserve_every_pixel() -> Result<(), Box<dyn std::error::Error>> {
    let expected: Vec<u8> = (0u8..16)
        .flat_map(|y| {
            (0u8..16).map(move |x| {
                x.wrapping_mul(17)
                    .wrapping_add(y.wrapping_mul(29))
                    .wrapping_add(x.wrapping_mul(y).wrapping_mul(3))
            })
        })
        .collect();

    for bytes in [
        include_bytes!("fixtures/ordinary.j2k").as_slice(),
        include_bytes!("fixtures/reset.j2k").as_slice(),
        include_bytes!("fixtures/reset-restart.j2k").as_slice(),
    ] {
        let settings = DecodeSettings {
            strict: true,
            ..DecodeSettings::default()
        };
        let image = Image::new(bytes, &settings)?;
        assert_eq!((image.width(), image.height()), (16, 16));
        assert_eq!(image.decode()?, expected);
    }

    Ok(())
}
