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
//! # Why glycin, and what would change it
//!
//! It was not chosen so much as inherited: every picture on this desktop
//! already goes through gdk-pixbuf -- the row squares, the thumbnails, the
//! sleeve on the now-playing card -- and a viewer that decoded some other way
//! would be a second picture path in a tree whose whole habit is one place for
//! one thing.
//!
//! The argument found afterwards is the better one. `gdk-pixbuf2` now depends
//! on glycin, which decodes in a sandbox: bubblewrap and seccomp are hard
//! dependencies of it. Image decoders have a long history of being the way in,
//! and this device opens files that came off the net through the download
//! panel, so decoding them in a sandbox is worth something and is already paid
//! for.
//!
//! libvips was considered and is not being used. It is a processing library
//! and not a drawing one -- it hands out pixels and something else has to put
//! them on a screen -- so it would sit behind this rather than replace it, and
//! it would want a bridge into `GdkTexture` that gdk-pixbuf does not need.
//!
//! What would change the answer is one measurement, and it is a device
//! measurement: a photograph at *its own size* or *four times* is held whole,
//! which for one off this machine's camera is the better part of a hundred
//! megabytes on an APU sharing its memory with the screen. libvips would pull
//! only the tile being looked at. If panning a zoomed photograph on the device
//! is slow, or the panel's RSS is ugly while one is open, that is the finding
//! that justifies it -- and it would be worth adding for the zoomed path
//! alone, not for the whole desktop's pictures. Nothing here has measured it.

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decoder {
    pub mime: &'static str,
    pub package: &'static str,
    pub because: &'static str,
}

pub const DECODERS: [Decoder; 13] = [
    Decoder {
        mime: "image/png",
        package: "glycin",
        because: "gdk-pixbuf's loader framework; glycin-image-rs reads it",
    },
    Decoder {
        mime: "image/jpeg",
        package: "glycin",
        because: "gdk-pixbuf's loader framework; glycin-image-rs reads it",
    },
    Decoder {
        mime: "image/webp",
        package: "glycin",
        because: "gdk-pixbuf's loader framework; glycin-image-rs reads it",
    },
    Decoder {
        mime: "image/gif",
        package: "glycin",
        because: "gdk-pixbuf's loader framework; glycin-image-rs reads it",
    },
    Decoder {
        mime: "image/tiff",
        package: "glycin",
        because: "gdk-pixbuf's loader framework; glycin-image-rs reads it",
    },
    Decoder {
        mime: "image/avif",
        package: "libheif",
        because: "an optional dependency of glycin, so not installed unless asked for",
    },
    Decoder {
        mime: "image/heif",
        package: "libheif",
        because: "an optional dependency of glycin, so not installed unless asked for",
    },
    Decoder {
        mime: "video/mp4",
        package: "gst-libav",
        because: "the ffmpeg decoders, as GStreamer elements",
    },
    Decoder {
        mime: "video/matroska",
        package: "gst-plugins-good",
        because: "the matroska demuxer",
    },
    Decoder { mime: "video/webm", package: "gst-plugins-good", because: "the matroska demuxer" },
    Decoder {
        mime: "video/quicktime",
        package: "gst-plugins-good",
        because: "the quicktime demuxer",
    },
    Decoder { mime: "video/vnd.avi", package: "gst-plugins-good", because: "the avi demuxer" },
    Decoder { mime: "video/ogg", package: "gst-plugins-base", because: "the ogg demuxer" },
];

pub const DRAWING: [Needed; 1] = [Needed {
    package: "gst-plugin-gtk4",
    because: "the sink that hands back a paintable; GTK here is built with no media backend",
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
        assert_eq!(decoder("image/avif").map(|one| one.map(|one| one.package)), Ok(Some("libheif")));
        assert_eq!(decoder("image/heif").map(|one| one.map(|one| one.package)), Ok(Some("libheif")));
    }

    #[test]
    fn the_packages_are_a_list_with_no_repeats_in_it() {
        let Ok(every) = packages();
        let mut sorted = every.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(every, sorted);
        assert!(every.contains(&"libheif"));
        assert!(every.contains(&"glycin"));
    }

    #[test]
    fn what_a_film_is_drawn_on_is_asked_for_as_well_as_what_reads_it() {
        let Ok(every) = packages();

        assert!(every.contains(&"gst-plugin-gtk4"), "{every:?}");

        for one in DRAWING {
            assert!(!one.package.is_empty());
            assert!(!one.because.is_empty(), "{} says no reason", one.package);
        }
    }

    #[test]
    fn the_surface_is_not_named_as_a_decoder_for_anything() {
        for one in DECODERS {
            assert_ne!(one.package, "gst-plugin-gtk4", "{}", one.mime);
        }
    }

    #[test]
    fn a_type_nothing_here_names_has_no_decoder() {
        assert_eq!(decoder("audio/mpeg"), Ok(None));
        assert_eq!(decoder("image/jxl"), Ok(None));
    }
}
