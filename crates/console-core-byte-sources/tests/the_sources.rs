use std::io::{self, Cursor, ErrorKind, Read, Seek, SeekFrom};
use std::sync::Arc;

use console_core_byte_sources::{ByteSource, Empty, FileSource, ReadSeekSource};
use console_core_never::Never;

type Failure = Box<dyn std::error::Error>;

fn boundaries(source: &impl ByteSource) -> Result<(), Failure> {
    let mut bytes = [0u8; 2];

    let length = source.len()?;
    let empty = source.is_empty()?;

    assert_eq!(length, 4);
    assert_eq!(empty, Empty::No);

    source.read_at(1, &mut bytes)?;

    assert_eq!(bytes, [2, 3]);

    source.read_at(0, &mut bytes)?;

    assert_eq!(bytes, [1, 2]);

    source.read_at(2, &mut bytes)?;

    assert_eq!(bytes, [3, 4]);

    source.read_at(4, &mut [])?;

    assert_eq!(source.read_at(3, &mut bytes).map_err(|fault| fault.kind()), Err(ErrorKind::UnexpectedEof));
    assert_eq!(source.read_at(u64::MAX, &mut bytes).map_err(|fault| fault.kind()), Err(ErrorKind::UnexpectedEof));

    Ok(())
}

#[test]
fn memory_and_seekable_sources_answer_the_same_reads() -> Result<(), Failure> {
    let bytes = vec![1u8, 2, 3, 4];

    boundaries(&bytes)?;
    boundaries(&bytes.as_slice())?;
    boundaries(&Arc::<[u8]>::from(bytes.clone()))?;
    boundaries(&bytes.clone().into_boxed_slice())?;
    let source = ReadSeekSource::new(Cursor::new(bytes))?;

    boundaries(&source)?;

    let empty = ByteSource::is_empty([].as_slice())?;

    assert_eq!(empty, Empty::Yes);

    Ok(())
}

#[test]
fn files_are_read_without_moving_the_stream_position() -> Result<(), Failure> {
    let directory = console_core_temporary_directories::fresh("byte-sources-file")?;
    let path = directory.join("bytes");

    console_core_atomic_writes::whole(&path, &[1, 2, 3, 4])?;

    let mut file = std::fs::File::open(&path)?;

    file.seek(SeekFrom::Start(1))?;
    boundaries(&file)?;

    let position = file.stream_position()?;

    assert_eq!(position, 1);

    let source = FileSource::new(file)?;

    boundaries(&source)?;

    let Ok(mut returned_file) = source.into_inner();

    let returned_position = returned_file.stream_position()?;

    assert_eq!(returned_position, 1);

    let unreadable = std::fs::File::open(directory)?;

    assert_eq!(unreadable.read_at(0, &mut [0u8; 1]).map_err(|fault| fault.kind()), Err(ErrorKind::IsADirectory));

    Ok(())
}

struct Reader {
    bytes: Cursor<Vec<u8>>,
    seeks: u32,
    reads: u32,
    fault: Option<ErrorKind>,
}

impl Reader {
    fn new(fault: Option<ErrorKind>) -> Result<Self, Never> {
        Ok(Self { bytes: Cursor::new(vec![1, 2, 3, 4]), seeks: 0, reads: 0, fault })
    }
}

impl Read for Reader {
    #[cfg_attr(
        dylint_lib = "explicit051_no_machine_width",
        allow(explicit051_no_machine_width, reason = "std::io::Read requires a usize byte count; source offsets and lengths are held as u64")
    )]
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.reads = self.reads.saturating_add(1);

        let read_fault = match self.reads {
            2 => self.fault.take(),
            _another_read => None,
        };

        match read_fault {
            Some(fault) => {
                self.bytes.read(&mut [0u8; 1])?;

                Err(io::Error::from(fault))
            },
            None => self.bytes.read(into),
        }
    }
}

impl Seek for Reader {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.seeks = self.seeks.saturating_add(1);

        self.bytes.seek(to)
    }
}

#[test]
fn consecutive_reads_seek_once_and_a_rewind_seeks_again() -> Result<(), Failure> {
    let Ok(reader) = Reader::new(None);
    let source = ReadSeekSource::new(reader)?;
    let mut bytes = [0u8; 2];

    source.read_at(0, &mut bytes)?;

    assert_eq!(bytes, [1, 2]);

    source.read_at(2, &mut bytes)?;

    assert_eq!(bytes, [3, 4]);

    source.read_at(0, &mut bytes)?;

    assert_eq!(bytes, [1, 2]);

    let Ok(returned_reader) = source.into_inner();

    assert_eq!(returned_reader.seeks, 3);

    Ok(())
}

#[test]
fn a_read_that_moves_then_fails_is_rewound_before_retrying() -> Result<(), Failure> {
    let Ok(reader) = Reader::new(Some(ErrorKind::Other));
    let source = ReadSeekSource::new(reader)?;
    let mut bytes = [0u8; 2];

    source.read_at(0, &mut bytes)?;

    assert_eq!(bytes, [1, 2]);
    assert_eq!(source.read_at(2, &mut bytes).map_err(|fault| fault.kind()), Err(ErrorKind::Other));

    source.read_at(2, &mut bytes)?;

    assert_eq!(bytes, [3, 4]);

    let Ok(returned_reader) = source.into_inner();

    assert_eq!(returned_reader.seeks, 3);

    Ok(())
}
