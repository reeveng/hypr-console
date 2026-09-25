//! What has to be on the machine before a claimed type will actually open.
//!
//! The manifest describes what is installed. It does not describe what is
//! *needed*, and the difference is where this desktop has been bitten twice.
//! `desktop.conf` names `kew`, so an apply installs the packaged kew -- and the
//! music panel wants two answers only our fork gives, which nothing in the
//! manifest can say. A fresh machine comes up with next and previous dead and
//! no card explaining why.
//!
//! This panel had the same exposure and it was found before it shipped. Seven
//! image types are claimed, and on the machine this was written on all seven
//! decode -- so every test passed and a picture opened. But `gtk4` pulls
//! `gdk-pixbuf2`, which pulls `glycin`, and `libheif` is an **optional**
//! dependency of glycin. It is on this laptop because something else wanted
//! it. On a device rebuilt from the manifest alone it is not there, the
//! `glycin-heif` loader has nothing to load, and an `.avif` or a `.heic` --
//! which is what a phone camera writes -- opens a panel that cannot decode it.
//!
//! The one shape of fault worth naming: it is invisible on the machine that
//! develops this, by construction. A developer's machine has everything.
//!
//! # So the table is written down and crossed
//!
//! Every type this panel claims says which package decodes it, and a test
//! crosses that both ways against `desktop.conf`. A type claimed with no
//! decoder named is a file that will not open; a decoder named that the
//! manifest does not install is a package on the device by luck, which is the
//! same thing one apply later.
//!
//! It is a written-down table rather than a reading of the machine for the
//! reason `console_settings::defaults::KINDS` is: what a machine happens to
//! have says nothing about what a rebuilt one will have, and the rebuilt one
//! is the machine this file is about.
//!
//! # Why ffmpeg, and what it replaced
//!
//! It was glycin, which was not chosen so much as inherited: every picture on
//! this desktop went through gdk-pixbuf -- the row squares, the thumbnails, the
//! sleeve on the now-playing card -- and gdk-pixbuf now decodes through glycin,
//! in a sandbox, with libheif beside it for what a phone camera writes.
//!
//! The toolkit went, and the sandbox and the loader framework were the
//! toolkit's. What replaced them is what was already here for films: ffmpeg
//! reads every one of these types, scales while it reads, and writes out the
//! run of RGBA a surface puts in a buffer -- so one package decodes the whole
//! list, `console-pictures` is the one place that calls it, and the types a
//! phone writes are read by dav1d and hevc rather than by an optional
//! dependency of a dependency.
//!
//! What was lost with glycin is the sandbox, and it is worth writing down
//! rather than discovering: a decoder is a way in, and this device opens files
//! that came off the net through the download panel. ffmpeg runs as its own
//! process here, which is the boundary a crash stops at but not the one an
//! exploit does. Putting that process under the same seccomp and namespace
//! rules the notifications daemon already has is the backlog's, and it is one
//! unit file rather than a library choice.
//!
//! libvips was considered and is not being used. It is a processing library
//! and not a drawing one, so it would sit behind this rather than replace it.
//! What would change the answer is one measurement, and it is a device
//! measurement: a photograph at *its own size* or *four times* is held whole,
//! which for one off this machine's camera is the better part of a hundred
//! megabytes on an APU sharing its memory with the screen. libvips would pull
//! only the tile being looked at. If panning a zoomed photograph on the device
//! is slow, or the panel's RSS is ugly while one is open, that is the finding
//! that justifies it. Nothing here has measured it.

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decoder {
    pub mime: &'static str,
    pub package: &'static str,
    pub because: &'static str,
}

const READS_IT: &str = "one decoder for the whole list, already here for films";

const A_PHONE_WRITES_IT: &str = "read by dav1d and hevc inside ffmpeg, rather than by an optional dependency";

const A_DEMUXER: &str = "the container, opened by the same program that decodes what is in it";

pub const DECODERS: [Decoder; 14] = [
    Decoder { mime: "image/png", package: "ffmpeg", because: READS_IT },
    Decoder { mime: "image/jpeg", package: "ffmpeg", because: READS_IT },
    Decoder { mime: "image/webp", package: "ffmpeg", because: READS_IT },
    Decoder { mime: "image/gif", package: "ffmpeg", because: READS_IT },
    Decoder { mime: "image/tiff", package: "ffmpeg", because: READS_IT },
    Decoder { mime: "image/avif", package: "ffmpeg", because: A_PHONE_WRITES_IT },
    Decoder { mime: "image/heif", package: "ffmpeg", because: A_PHONE_WRITES_IT },
    Decoder { mime: "video/mp4", package: "ffmpeg", because: A_DEMUXER },
    Decoder { mime: "video/matroska", package: "ffmpeg", because: A_DEMUXER },
    Decoder { mime: "video/x-matroska", package: "ffmpeg", because: A_DEMUXER },
    Decoder { mime: "video/webm", package: "ffmpeg", because: A_DEMUXER },
    Decoder { mime: "video/quicktime", package: "ffmpeg", because: A_DEMUXER },
    Decoder { mime: "video/vnd.avi", package: "ffmpeg", because: A_DEMUXER },
    Decoder { mime: "video/ogg", package: "ffmpeg", because: A_DEMUXER },
];

pub const DRAWING: [Needed; 1] = [Needed {
    package: "librsvg",
    because: "the one thing ffmpeg does not read, which is most of an icon theme",
}];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Needed {
    pub package: &'static str,
    pub because: &'static str,
}

pub fn packages() -> Result<Vec<&'static str>, Never> {
    let mut every: Vec<&'static str> = DECODERS
        .iter()
        .map(|one| one.package)
        .chain(DRAWING.iter().map(|one| one.package))
        .collect();
    every.sort_unstable();
    every.dedup();

    Ok(every)
}

pub fn decoder(mime: &str) -> Result<Option<&'static Decoder>, Never> {
    Ok(DECODERS.iter().find(|one| one.mime == mime))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_claimed_type_says_what_decodes_it() {
        for one in DECODERS {
            assert!(!one.package.is_empty(), "{}", one.mime);
            assert!(!one.because.is_empty(), "{} says no reason", one.mime);
        }
    }

    #[test]
    fn the_two_a_phone_writes_are_not_left_to_luck() {
        assert_eq!(decoder("image/avif").map(|one| one.map(|one| one.package)), Ok(Some("ffmpeg")));
        assert_eq!(decoder("image/heif").map(|one| one.map(|one| one.package)), Ok(Some("ffmpeg")));
    }

    #[test]
    fn the_packages_are_a_list_with_no_repeats_in_it() {
        let Ok(every) = packages();
        let mut sorted = every.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(every, sorted);
        assert!(every.contains(&"ffmpeg"));
    }

    #[test]
    fn what_draws_an_icon_is_asked_for_as_well_as_what_reads_a_photograph() {
        let Ok(every) = packages();

        assert!(every.contains(&"librsvg"), "{every:?}");

        for one in DRAWING {
            assert!(!one.package.is_empty());
            assert!(!one.because.is_empty(), "{} says no reason", one.package);
        }
    }

    #[test]
    fn a_type_nothing_here_names_has_no_decoder() {
        assert_eq!(decoder("audio/mpeg"), Ok(None));
        assert_eq!(decoder("image/jxl"), Ok(None));
    }
}
