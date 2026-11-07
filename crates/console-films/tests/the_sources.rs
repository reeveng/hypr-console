use std::io::{self, ErrorKind};

use console_core_byte_sources::ByteSource;
use console_core_video_decoding_matroska_container as matroska;
use console_core_video_decoding_mpeg_4_container as mpeg4;

struct Unmeasured;

impl ByteSource for Unmeasured {
    fn len(&self) -> io::Result<u64> {
        Err(io::Error::from(ErrorKind::PermissionDenied))
    }

    fn read_at(&self, _offset: u64, _into: &mut [u8]) -> io::Result<()> {
        Err(io::Error::from(ErrorKind::UnexpectedEof))
    }
}

#[test]
fn a_source_that_cannot_be_measured_is_an_io_fault_in_both_containers() {
    let mp4 = mpeg4::open(Unmeasured);
    let mkv = matroska::open(Unmeasured);

    assert!(matches!(mp4, Err(mpeg4::Error::Io(fault)) if fault.kind() == ErrorKind::PermissionDenied));
    assert!(matches!(mkv, Err(matroska::Error::Io(fault)) if fault.kind() == ErrorKind::PermissionDenied));
}
