//! Both document image filters reach the shared MQ engine through a rendered PDF.

use console_books::portable_document::{Scale, opened};
use console_core_never::Never;
use console_core_number_conversion::fitted;

fn image_document(encoded: &[u8], filter: &str, depth: u8) -> Result<Vec<u8>, Never> {
    let mut image = format!("<< /Type /XObject /Subtype /Image /Width 16 /Height 16 /ColorSpace /DeviceGray /BitsPerComponent {depth} /Filter /{filter} /Length {} >>\nstream\n", encoded.len()).into_bytes();
    image.extend_from_slice(encoded);
    image.extend_from_slice(b"\nendstream");

    let content = "q 16 0 0 16 0 0 cm /Image Do Q";
    let objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 16 16] /Contents 4 0 R /Resources << /XObject << /Image 5 0 R >> >> >>".to_vec(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).into_bytes(),
        image,
    ];
    let mut document = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();

    for (number, object) in (1u32..).zip(&objects) {
        let Ok(offset) = fitted::<_, u64>(document.len());

        offsets.push(offset);
        document.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
        document.extend_from_slice(object);
        document.extend_from_slice(b"\nendobj\n");
    }

    let Ok(table) = fitted::<_, u64>(document.len());

    document.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");

    for offset in offsets {
        document.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }

    document.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{table}\n%%EOF\n").as_bytes(),
    );

    Ok(document)
}

#[test]
fn jbig2_and_jpeg2000_document_images_keep_their_pixels() -> Result<(), Box<dyn std::error::Error>>
{
    let folder = console_core_temporary_directories::fresh("books-mq-images")?;
    let jbig2: Vec<u8> = include_bytes!(
        "../../../forks/console-core-black-and-white-images/tests/fixtures/template-0.pixels"
    )
    .iter()
    .filter(|byte| **byte != b'\n')
    .map(|byte| match byte {
        b'1' => 0,
        _white => 255,
    })
    .collect();
    let jpeg2000: Vec<u8> = (0u8..16)
        .flat_map(|y| {
            (0u8..16).map(move |x| {
                x.wrapping_mul(17)
                    .wrapping_add(y.wrapping_mul(29))
                    .wrapping_add(x.wrapping_mul(y).wrapping_mul(3))
            })
        })
        .collect();

    for (filter, depth, encoded, expected) in [
        (
            "JBIG2Decode",
            1,
            include_bytes!(
                "../../../forks/console-core-black-and-white-images/tests/fixtures/template-0.jb2"
            )
            .as_slice(),
            jbig2,
        ),
        (
            "JPXDecode",
            8,
            include_bytes!(
                "../../../forks/console-core-jpeg-2000-files/tests/fixtures/reset-restart.j2k"
            )
            .as_slice(),
            jpeg2000,
        ),
    ] {
        let Ok(bytes) = image_document(encoded, filter, depth);
        let at = folder.join(format!("{filter}.pdf"));

        console_core_atomic_writes::whole(&at, &bytes)?;

        let document = opened(&at)?;
        let Ok(drawn) = document.drawn(0, Scale::Tall(16));
        let pixels = drawn.ok_or("the document image was not drawn")?;

        assert_eq!((pixels.width, pixels.height), (16, 16));
        assert_eq!(pixels.bytes.len(), expected.len().saturating_mul(4));

        for (pixel, gray) in pixels.bytes.chunks_exact(4).zip(expected) {
            assert_eq!(pixel, [gray, gray, gray, 255], "{filter}");
        }
    }

    Ok(())
}
