//! Embedded arithmetic-coded generic regions exercise all four JBIG2 templates.

use console_core_black_and_white_images::{Decoder, DecoderContext, Image};

#[derive(Default)]
struct Pixels {
    pixels: Vec<u8>,
    rows: u32,
}

impl Decoder for Pixels {
    fn push_pixel(&mut self, black: bool) {
        self.pixels.push(u8::from(black));
    }
    fn push_pixel_chunk(&mut self, black: bool, chunk_count: u32) {
        for _pixel in 0..chunk_count * 8 {
            self.push_pixel(black);
        }
    }
    fn next_line(&mut self) {
        self.rows += 1;
    }
}

#[test]
fn all_arithmetic_templates_preserve_the_document_bitmap() -> Result<(), Box<dyn std::error::Error>>
{
    let mut context = DecoderContext::default();

    for (bytes, expected) in [
        (
            include_bytes!("fixtures/template-0.jb2").as_slice(),
            include_bytes!("fixtures/template-0.pixels").as_slice(),
        ),
        (
            include_bytes!("fixtures/template-1.jb2").as_slice(),
            include_bytes!("fixtures/template-1.pixels").as_slice(),
        ),
        (
            include_bytes!("fixtures/template-2.jb2").as_slice(),
            include_bytes!("fixtures/template-2.pixels").as_slice(),
        ),
        (
            include_bytes!("fixtures/template-3.jb2").as_slice(),
            include_bytes!("fixtures/template-3.pixels").as_slice(),
        ),
    ] {
        let image = Image::new_embedded(bytes, None)?;
        assert_eq!((image.width(), image.height()), (16, 16));
        let mut output = Pixels::default();
        image.decode_with(&mut output, &mut context)?;
        assert_eq!(output.rows, 16);
        let expected: Vec<u8> = expected
            .iter()
            .filter(|byte| **byte != b'\n')
            .map(|byte| byte - b'0')
            .collect();
        assert_eq!(output.pixels, expected);

        let mut repeated = Pixels::default();
        image.decode_with(&mut repeated, &mut context)?;
        assert_eq!(repeated.pixels, expected);
        assert_eq!(repeated.rows, 16);
    }
    Ok(())
}
