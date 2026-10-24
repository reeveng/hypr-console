//! The film's file, read where it lies.
//!
//! Both containers ask for bytes at an offset rather than reading a stream, so
//! the file is asked with a positioned read and never wound back and forth.
//! Each container names the question in its own trait, and the one file
//! answers both.

use std::fs::File;
use std::os::unix::fs::FileExt;

use console_core_video_decoding_matroska_container as matroska;
use console_core_video_decoding_mpeg_4_container as mpeg4;

pub(crate) struct Opened {
    file: File,
    length: u64,
}

impl From<File> for Opened {
    fn from(file: File) -> Opened {
        let length = match file.metadata() {
            Ok(about) => about.len(),
            Err(_unmeasured) => 0,
        };

        Opened { file, length }
    }
}

impl Opened {
    pub(crate) fn read_into(&self, offset: u64, into: &mut [u8]) -> std::io::Result<()> {
        self.file.read_exact_at(into, offset)
    }
}

impl mpeg4::ByteSource for Opened {
    fn len(&self) -> u64 {
        self.length
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> std::io::Result<()> {
        self.read_into(offset, buf)
    }
}

impl matroska::ByteSource for Opened {
    fn len(&self) -> u64 {
        self.length
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> std::io::Result<()> {
        self.read_into(offset, buf)
    }
}
