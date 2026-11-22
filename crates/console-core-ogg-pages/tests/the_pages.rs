use std::cmp::Ordering;
use std::io::{Cursor, Seek};

use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_ogg_pages::{Packets, Page, PageError, PageHeader, Placement};

type Failure = Box<dyn std::error::Error>;

const SERIAL: u32 = 1234;

const WRITTEN_PAGE: u64 = 32 * 255;

fn opus_identification() -> Result<Vec<u8>, Never> {
  let mut page = b"OggS\0\x02".to_vec();

  page.extend(0u64.to_le_bytes());
  page.extend(1_759_377_061u32.to_le_bytes());
  page.extend(0u32.to_le_bytes());
  page.extend(3_579_522_525u32.to_le_bytes());
  page.extend([1, 19]);
  page.extend(b"OpusHead\x01\x02\x38\x01\x80\xBB\0\0\0\0\0");

  Ok(page)
}

fn packets(contents: Vec<Vec<u8>>) -> Result<Packets, Failure> {
  let count = u8::try_from(contents.len())?;
  let mut page = b"OggS\0\0".to_vec();

  page.extend([0u8; 20]);
  page.push(count);
  page.extend(contents.iter().map(|_empty| 0u8));

  let mut stream = Packets::read_count(&mut Cursor::new(page), u64::from(count))?;

  for (at, content) in (0u64..).zip(contents) {
    stream.set(at, content)?;
  }

  Ok(stream)
}

fn written(contents: Vec<Vec<u8>>, placement: Placement) -> Result<Vec<u8>, Failure> {
  let mut bytes = Vec::new();

  let stream = packets(contents)?;

  stream.write_to(&mut bytes, SERIAL, 0, placement)?;

  Ok(bytes)
}

fn kind(page: &Page) -> Result<Option<u8>, Never> {
  let Ok(bytes) = page.bytes();

  Ok(bytes.get(5).copied())
}

fn pages(bytes: &[u8]) -> Result<Vec<Page>, Failure> {
  let mut reader = Cursor::new(bytes);
  let mut read = Vec::new();

  for _page in bytes {
    let at = reader.stream_position()?;
    let end = u64::try_from(bytes.len())?;

    match at.cmp(&end) {
      Ordering::Equal => break,
      Ordering::Less | Ordering::Greater => {}
    }

    let page = Page::read(&mut reader)?;

    read.push(page);
  }

  Ok(read)
}

fn segments(page: &Page) -> Result<Vec<u8>, Never> {
  let Ok(bytes) = page.bytes();
  let listed = match bytes.get(26) {
    Some(count) => *count,
    None => 0,
  };
  let Ok(count) = index(listed);

  Ok(
    bytes
      .get(27..)
      .into_iter()
      .flatten()
      .take(count)
      .copied()
      .collect(),
  )
}

#[test]
fn a_real_page_reads_as_it_was_written_and_checks_to_the_crc_it_carries() -> Result<(), Failure> {
  let Ok(original) = opus_identification();
  let mut page = Page::read(&mut Cursor::new(original.clone()))?;

  assert_eq!(page.header.stream_serial, 1_759_377_061);
  assert_eq!(page.header.granule_position, 0);
  assert_eq!(page.header.sequence_number, 0);
  assert_eq!(page.header.start, 0);
  assert_eq!(page.bytes(), Ok(original.clone()));

  let Ok(()) = page.update_checksum();

  assert_eq!(page.bytes(), Ok(original));

  Ok(())
}

#[test]
fn a_reader_that_is_not_at_a_page_says_which_way_it_is_not() -> Result<(), Failure> {
  let Ok(page) = opus_identification();
  let mut versioned = page.clone();
  let mut unlaced = page.clone();

  versioned.splice(4..5, [1]);
  unlaced.splice(26..27, [0]);

  assert!(matches!(
    PageHeader::read(&mut Cursor::new(b"OggT\0\x02".to_vec())),
    Err(PageError::MissingCapturePattern)
  ));
  assert!(matches!(
    PageHeader::read(&mut Cursor::new(versioned)),
    Err(PageError::UnknownVersion)
  ));
  assert!(matches!(
    PageHeader::read(&mut Cursor::new(unlaced)),
    Err(PageError::NoSegments)
  ));
  assert!(matches!(
    PageHeader::read(&mut Cursor::new(b"OggS\0".to_vec())),
    Err(PageError::Io(_))
  ));

  Ok(())
}

#[test]
fn a_packet_read_stops_just_past_its_last_byte_even_mid_page() -> Result<(), Failure> {
  let mut page = b"OggS\0\x02".to_vec();

  page.extend([0u8; 20]);
  page.extend([2, 3, 2]);
  page.extend(b"abcdefg");

  let mut reader = Cursor::new(page);
  let packets = Packets::read_count(&mut reader, 1)?;

  assert_eq!(packets.get(0), Ok(Some(b"abc".as_slice())));
  assert_eq!(packets.get(1), Ok(None));
  let stopped = reader.stream_position()?;

  assert_eq!(stopped, 32);

  let mut again = Cursor::new(reader.into_inner());
  let both = Packets::read_count(&mut again, 2)?;

  assert_eq!(both.get(1), Ok(Some(b"de".as_slice())));
  assert!(matches!(
    Packets::read_count(&mut Cursor::new(again.into_inner()), 3),
    Err(PageError::TooFewPackets)
  ));

  Ok(())
}

#[test]
fn packets_written_out_read_back_as_the_same_packets() -> Result<(), Failure> {
  let long: Vec<u8> = (0u32..20_000)
    .map(|at| at.to_le_bytes())
    .map(|[low, ..]| low)
    .collect();
  let packets = vec![
    b"OpusHead".to_vec(),
    long,
    Vec::from([7u8; 255]),
    b"OpusTags".to_vec(),
  ];
  let bytes = written(packets.clone(), Placement::BeginningOfStream)?;
  let read = Packets::read_count(&mut Cursor::new(bytes), 4)?;

  assert_eq!(
    (&read).into_iter().map(<[u8]>::to_vec).collect::<Vec<_>>(),
    packets
  );

  Ok(())
}

#[test]
fn a_long_packet_is_cut_into_numbered_pages_that_say_it_goes_on() -> Result<(), Failure> {
  const LENGTH: u64 = 129 * WRITTEN_PAGE + 5 * 255 + 171;

  let packet = (0..LENGTH).map(|_| 0).collect();
  let bytes = written(vec![packet], Placement::InsideStream)?;
  let read = pages(&bytes)?;

  assert_eq!(read.len(), 130);

  for (number, page) in (0u32..).zip(&read) {
    let Ok(laced) = segments(page);
    let expected = match number {
      129 => (0, [vec![255; 5], vec![171]].concat()),
      _continues => (u64::MAX, vec![255; 32]),
    };

    assert_eq!(page.header.stream_serial, SERIAL);
    assert_eq!(page.header.sequence_number, number);
    assert_eq!((page.header.granule_position, laced), expected);

    assert_eq!(
      kind(page),
      Ok(Some(match number {
        0 => 0,
        _later => 1,
      }))
    );
  }

  Ok(())
}

#[test]
fn a_packet_that_fills_its_pages_exactly_is_ended_by_an_empty_one() -> Result<(), Failure> {
  let packet = (0..WRITTEN_PAGE.saturating_mul(20)).map(|_| 0).collect();
  let bytes = written(vec![packet], Placement::InsideStream)?;
  let read = pages(&bytes)?;

  assert_eq!(read.len(), 21);

  let last = read.last().map(segments);

  assert_eq!(last, Some(Ok(vec![0])));

  let mut total = 0u64;

  for page in &read {
    let Ok(laced) = segments(page);

    total = laced
      .into_iter()
      .fold(total, |sum, segment| sum.saturating_add(u64::from(segment)));
  }

  assert_eq!(total, WRITTEN_PAGE.saturating_mul(20));

  let whole_segments = (0..17u64.saturating_mul(255)).map(|_| 0).collect();
  let short = written(vec![whole_segments], Placement::InsideStream)?;
  let one = pages(&short)?;
  let first = one.first().map(segments);

  assert_eq!(one.len(), 1);
  assert_eq!(first, Some(Ok([vec![255; 17], vec![0]].concat())));

  Ok(())
}

#[test]
fn the_first_page_of_a_stream_says_so_and_no_other_does() -> Result<(), Failure> {
  let bytes = written(
    vec![b"OpusHead".to_vec(), b"OpusTags".to_vec()],
    Placement::BeginningOfStream,
  )?;
  let read = pages(&bytes)?;
  let kinds: Vec<Result<Option<u8>, Never>> = read.iter().map(kind).collect();

  assert_eq!(kinds, [Ok(Some(2)), Ok(Some(0))]);

  Ok(())
}

#[test]
fn a_packet_is_replaced_only_where_there_is_one() -> Result<(), Failure> {
  let bytes = written(
    vec![b"one".to_vec(), b"two".to_vec()],
    Placement::BeginningOfStream,
  )?;
  let mut packets = Packets::read_count(&mut Cursor::new(bytes), 2)?;

  packets.set(1, b"three".to_vec())?;

  assert_eq!(packets.get(1), Ok(Some(b"three".as_slice())));
  assert!(matches!(
    packets.set(2, Vec::new()),
    Err(PageError::NoSuchPacket)
  ));

  Ok(())
}
