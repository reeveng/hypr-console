use std::error::Error;
use std::path::Path;

use console_core_checksums::crc32;

#[test]
fn cabac_fixtures_keep_every_decoded_pixel() -> Result<(), Box<dyn Error>> {
    let mut seen = Vec::new();

    for named in [
        "red-then-blue.h264.mp4",
        "red-then-blue.hevc.mp4",
        "dark-then-green.h264.mp4",
        "turned.h264.mp4",
    ] {
        let at = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/films")
            .join(named);
        let opened = console_films::open(&at)?;
        let film = opened.ok_or("fixture was not recognized")?;
        let track = film.pictures(0.0)?;
        let mut pictures = track.ok_or("fixture has no pictures")?;
        let mut pixels = Vec::new();
        let mut frames = 0u32;

        'frames: for _frame in 0..120 {
            let next = pictures.next_picture()?;
            let decoded = match next {
                Some(picture) => picture,
                None => break 'frames,
            };
            let Ok(bitmap) = decoded.rgba();

            frames = frames.saturating_add(1);
            pixels.extend_from_slice(&bitmap.rgba);
        }

        let Ok(checksum) = crc32::of(&pixels);

        seen.push((named, frames, checksum));
    }

    assert_eq!(
        seen,
        vec![
            ("red-then-blue.h264.mp4", 50, 480537549),
            ("red-then-blue.hevc.mp4", 50, 480537549),
            ("dark-then-green.h264.mp4", 101, 890606424),
            ("turned.h264.mp4", 50, 480537549),
        ]
    );

    Ok(())
}
