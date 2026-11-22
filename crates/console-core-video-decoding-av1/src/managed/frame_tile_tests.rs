use super::*;
use console_core_checksums::{Checksum, md5::Md5};

#[cfg(feature = "c-ffi")]
#[test]
fn sequence_header_round_trip_checks_profile_codes() -> crate::src::error::Rav1dResult {
  use crate::include::dav1d::headers::{Dav1dSequenceHeader, Rav1dSequenceHeader};
  let parsed = crate::src::obu::rav1d_parse_sequence_header(STREAM)?;
  let mut wire = Dav1dSequenceHeader::from(parsed.rav1d.clone());
  let round_trip = Rav1dSequenceHeader::try_from(wire.clone())?;
  assert!(round_trip.eq_without_operating_parameter_info(&parsed.rav1d));
  assert!(round_trip.operating_parameter_info == parsed.rav1d.operating_parameter_info);
  wire.profile = u8::MAX;
  assert!(matches!(
    Rav1dSequenceHeader::try_from(wire),
    Err(Rav1dError::EINVAL)
  ));

  Ok(())
}

#[test]
fn mastering_display_checks_the_primary_color_index() {
  let display = MasteringDisplay {
    primaries: [[35000, 15000], [12500, 37500], [10000, 2500]],
    white_point: [15635, 16450],
    max_luminance: 10_000_000,
    min_luminance: 100,
  };

  assert_eq!(display.primary_chromaticity(0), Ok([0.7, 0.3]));
  assert_eq!(display.primary_chromaticity(1), Ok([0.25, 0.75]));
  assert_eq!(display.primary_chromaticity(2), Ok([0.2, 0.05]));
  assert_eq!(display.primary_chromaticity(3), Err(PrimaryColorIndexError));
}

#[test]
fn a_partial_worker_start_joins_the_workers_that_started() -> Result<(), Error> {
  use console_core_parallel_execution::Failed;
  use std::cell::Cell;
  use std::io;
  use std::num::NonZeroU32;
  use std::sync::mpsc::{SyncSender, sync_channel};

  struct OneStart {
    joined: Cell<Option<SyncSender<()>>>,
  }

  impl Workers for OneStart {
    fn parallelism(&self) -> Result<NonZeroU32, Never> {
      Ok(NonZeroU32::MIN.saturating_add(1))
    }

    fn start(&self, name: String, work: impl FnOnce() + Send + 'static) -> io::Result<Worker> {
      match self.joined.take() {
        None => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        Some(joined) => {
          let started = console_concurrency::HostWorkers.start(name, work)?;
          let Ok(handle) = Worker::new(move || {
            started.join()?;
            joined.try_send(()).map_err(|_receiver_unavailable| Failed)
          });

          Ok(handle)
        }
      }
    }
  }

  let (sender, receiver) = sync_channel(1);
  let workers = OneStart { joined: Cell::new(Some(sender)) };
  let settings = Settings { threads: 2, ..Default::default() };
  let result = Decoder::with_workers(settings, &workers).map_err(|fault| fault.decompose().0);
  assert!(matches!(result, Err(Error::InitFailed)));
  receiver.try_recv().map_err(|_worker_not_joined| Error::InitFailed)
}

const STREAM: &[u8] =
  include_bytes!("../../tests/crash_vectors/tile_threading_cdef_lpf_race.obu");
const STREAM_MD5: [u8; 16] = [
  0x51, 0xb9, 0xc3, 0xab, 0x24, 0x6f, 0xda, 0x65, 0xe2, 0xc0, 0xa2, 0x15, 0x55, 0x88, 0xe9, 0xa5,
];

fn open_fixture(settings: Settings) -> Result<Decoder, Error> {
  Decoder::with_workers(settings, &console_concurrency::HostWorkers).map_err(|fault| fault.decompose().0)
}

fn decode_fixture(decoder: &mut Decoder) -> Result<Frame, Error> {
  let decoded = decoder.decode(STREAM).map_err(|fault| fault.decompose().0)?;

  decoded.ok_or(Error::NeedMoreData)
}

#[test]
fn a_truncated_packet_returns_invalid_data_and_the_next_frame_decodes() -> Result<(), Error> {
  let mut decoder = Decoder::new().map_err(|fault| fault.decompose().0)?;
  let (_, truncated) = STREAM.split_last().ok_or(Error::InvalidData)?;
  let result = decoder
    .decode(truncated)
    .map_err(|fault| fault.decompose().0);
  assert_eq!(result.map(|_| ()), Err(Error::InvalidData));

  let decoded = decoder
    .decode(STREAM)
    .map_err(|fault| fault.decompose().0)?;
  let frame = decoded.ok_or(Error::NeedMoreData)?;
  let digest = hash(&frame)?;
  assert_eq!(digest, STREAM_MD5);

  Ok(())
}

#[test]
fn plane_access_agrees_with_rows_and_checks_coordinates() -> Result<(), Error> {
  let mut decoder = Decoder::new().map_err(|fault| fault.decompose().0)?;
  let decoded = decoder
    .decode(STREAM)
    .map_err(|fault| fault.decompose().0)?;
  let frame = decoded.ok_or(Error::NeedMoreData)?;
  let planes = frame.planes()?;
  let luma = match planes {
    Planes::Depth8(planes_2) => {
      let Ok(view) = planes_2.y();
      view
    }
    Planes::Depth16(_) => return Err(Error::InvalidData),
  };
  let Ok(width) = luma.width();
  let Ok(height) = luma.height();
  let Ok(mut rows) = luma.rows();
  let Ok(first) = luma.row(0);

  assert_eq!(first, rows.next());
  assert_eq!(
    luma.pixel(Point { x: 0, y: 0 }),
    Ok(first.and_then(|row| row.first().copied()))
  );
  assert_eq!(luma.pixel(Point { x: width, y: 0 }), Ok(None));
  assert_eq!(luma.row(height), Ok(None));

  Ok(())
}

fn assert_picture_policy(frame: &Frame, threads: u32) -> Result<(), Error> {
  use crate::include::dav1d::picture::{PictureGuardMode, PictureThreading};

  let data = frame.inner.data.as_ref().ok_or(Error::InvalidData)?;

  for plane in &data.data {
    let Ok(declared) = plane.threading_policy();
    let policy = declared.ok_or(Error::InvalidData)?;
    assert_eq!(
      matches!(policy, PictureThreading::MultipleWorkersOneTile | PictureThreading::MultipleWorkersMultipleTiles),
      threads > 1,
    );
    let Ok(mode) = plane.guard_mode();
    let expected = match threads {
      0..=1 => PictureGuardMode::WholeHull,
      2.. => PictureGuardMode::SeparateRows,
    };
    assert_eq!(mode, expected);
  }

  Ok(())
}

#[test]
fn picture_policy_is_local_and_survives_decoder_lifetimes() -> Result<(), Error> {
  let mut retained = Vec::new();

  for order in [[8, 1], [1, 8]] {
    let decoded_workers: Result<Vec<_>, Error> = order
      .into_iter()
      .map(|threads| {
        let settings = Settings {
          threads,
          max_frame_delay: 1,
          ..Default::default()
        };
        open_fixture(settings)
      })
      .collect();
    let mut decoders = decoded_workers?;

    for _ in 0..2 {
      for (decoder, threads) in decoders.iter_mut().zip(order) {
        let frame = decode_fixture(decoder)?;
        assert_picture_policy(&frame, threads)?;
        let digest = hash(&frame)?;
        assert_eq!(digest, STREAM_MD5);
        retained.push((frame, threads));
      }
    }

    drop(decoders);

    for (frame, threads) in &retained {
      assert_picture_policy(frame, *threads)?;
      let mut copy = Rav1dPicture::default();
      crate::src::picture::rav1d_picture_alloc_copy(
        &None,
        &mut copy,
        frame.inner.p.w,
        &frame.inner,
      ).map_err(Error::from)?;
      assert_picture_policy(&Frame { inner: copy }, *threads)?;
    }
  }

  Ok(())
}

#[test]
fn copied_picture_can_allocate_after_decoder_and_source_drop() -> Result<(), Error> {
  let settings = Settings {
    threads: 4,
    max_frame_delay: 1,
    ..Default::default()
  };
  let mut decoder = open_fixture(settings)?;
  let mut frame = decode_fixture(&mut decoder)?;
  drop(decoder);

  for _ in 0..16 {
    let mut copy = Rav1dPicture::default();
    crate::src::picture::rav1d_picture_alloc_copy(
      &None,
      &mut copy,
      frame.inner.p.w,
      &frame.inner,
    ).map_err(Error::from)?;
    drop(frame);
    frame = Frame { inner: copy };
    assert_picture_policy(&frame, 4)?;
  }

  Ok(())
}

#[cfg(feature = "c-ffi")]
#[test]
fn retained_allocator_allocates_after_decoder_drop() -> Result<(), Error> {
  let settings = Settings {
    threads: 1,
    max_frame_delay: 1,
    ..Default::default()
  };
  let decoder = open_fixture(settings)?;
  let allocator = decoder.ctx.allocator.clone();
  drop(decoder);

  use crate::include::dav1d::picture::{Dav1dPicAllocator, Rav1dPicAllocator};
  let exported = Dav1dPicAllocator::from(Rav1dPicAllocator::default());
  let imported = Rav1dPicAllocator::try_from(exported).map_err(Error::from)?;

  assert!(imported.default_pool.is_none());

  for allocator_2 in [allocator, imported] {
    let seq_hdr = crate::src::obu::rav1d_parse_sequence_header(STREAM).map_err(Error::from)?;
    #[cfg_attr(
      dylint_lib = "explicit081_no_shared_ownership",
      expect(explicit081_no_shared_ownership, reason = "the allocated C picture needs a retained sequence header across copies and source release")
    )]
    let shared_header = Arc::new(seq_hdr);
    let mut picture = allocator_2
      .alloc_picture_data(16, 16, shared_header, None)
      .map_err(Error::from)?;
    drop(allocator_2);

    for _ in 0..4 {
      let mut next = Rav1dPicture::default();
      crate::src::picture::rav1d_picture_alloc_copy(&None, &mut next, 16, &picture).map_err(Error::from)?;
      drop(picture);
      picture = next;
    }

    let frame = Frame { inner: picture };
    let planes = frame.planes()?;

    match planes {
      Planes::Depth8(planes_2) => {
        let Ok(y) = planes_2.y();
        let Ok(mut rows) = y.rows();
        let first_row = rows.next().ok_or(Error::InvalidData)?;
        assert_eq!(first_row.len(), 16);
      }
      Planes::Depth16(_) => return Err(Error::InvalidData),
    }
  }

  Ok(())
}

#[test]
#[cfg_attr(
  dylint_lib = "explicit103_no_thread_per_item",
  expect(
    explicit103_no_thread_per_item,
    reason = "the fixture starts exactly two decoders, one for each fixed threading configuration"
  )
)]
fn serial_and_threaded_decoders_run_concurrently_with_local_policies() -> Result<(), Error> {
  let start = console_core_synchronization::nonpoison::RwLock::new(());
  let completed = std::thread::scope(|scope| {
    let paused = start.write();
    let start_gate = &start;
    let mut handles = Vec::new();

    for threads in [1, 8] {
      let settings = Settings {
        threads,
        max_frame_delay: 1,
        ..Default::default()
      };
      let mut decoder = Decoder::with_workers(settings, &console_concurrency::HostWorkers)
        .map_err(|fault| fault.decompose().0)?;

      handles.push(scope.spawn(move || {
        drop(start_gate.read());
        let mut frames = Vec::new();

        for _ in 0..8 {
          let decoded = decoder
            .decode(STREAM)
            .map_err(|fault| fault.decompose().0)?;
          let frame = decoded.ok_or(Error::NeedMoreData)?;
          assert_picture_policy(&frame, threads)?;
          let digest = hash(&frame)?;
          assert_eq!(digest, STREAM_MD5);
          frames.push((frame, threads));
        }

        Ok::<_, Error>((decoder, frames))
      }));
    }

    drop(paused);
    let mut finished = Vec::new();

    for handle in handles {
      #[cfg_attr(
        dylint_lib = "explicit057_no_unbounded_wait",
        expect(explicit057_no_unbounded_wait, reason = "both fixture workers decode a fixed eight-frame stream and are scoped to this test")
      )]
      let joined = match handle.join() {
        Ok(worker_result) => worker_result,
        Err(panic) => std::panic::resume_unwind(panic),
      };
      let worker = joined?;
      finished.push(worker);
    }

    Ok::<_, Error>(finished)
  })?;
  let retained: Vec<_> = completed
    .into_iter()
    .flat_map(|(_decoder, frames)| frames)
    .collect();
  assert_eq!(retained.len(), 16);

  for (frame, threads) in &retained {
    assert_picture_policy(frame, *threads)?;
  }

  Ok(())
}

fn hash(frame: &Frame) -> Result<[u8; 16], Error> {
  let mut hash = Md5::default();
  let planes = frame.planes()?;

  match planes {
    Planes::Depth8(planes_2) => {
      let Ok(luma) = planes_2.y();
      let Ok(blue) = planes_2.u();
      let Ok(red) = planes_2.v();

      for plane in [Some(luma), blue, red].into_iter().flatten() {
        let Ok(rows) = plane.rows();

        for row in rows {
          let Ok(()) = hash.process_buf_bytes(row);
        }
      }
    }
    Planes::Depth16(planes_3) => {
      let Ok(luma) = planes_3.y();
      let Ok(blue) = planes_3.u();
      let Ok(red) = planes_3.v();

      for plane in [Some(luma), blue, red].into_iter().flatten() {
        let Ok(rows) = plane.rows();

        for pixel in rows.flatten() {
          let Ok(()) = hash.process_buf_bytes(&pixel.to_le_bytes());
        }
      }
    }
  }

  let Ok(digest) = hash.md5();

  Ok(digest)
}

fn submit_fixture_packet(decoder: &mut Decoder, output: &mut Vec<Frame>) -> Result<(), Error> {
  use std::time::{Duration, Instant};

  let timeout = Duration::from_secs(30);
  let deadline = Instant::now().checked_add(timeout).ok_or(Error::InvalidData)?;
  let attempts = (0..timeout.as_nanos()).take_while(|_| Instant::now() < deadline);

  for _ in attempts {
    match decoder.decode(STREAM) {
      Ok(frame) => {
        output.extend(frame);
        return Ok(());
      }
      Err(fault) if matches!(fault.error(), Error::NeedMoreData) => {
        let next = decoder.get_frame().map_err(|error| error.decompose().0)?;
        output.extend(next);
      }
      Err(fault) => return Err(fault.decompose().0),
    }
  }

  Err(Error::Other("input backpressure did not clear".to_string()))
}

#[test]
fn parallel_frame_tile_contexts_preserve_frames() -> Result<(), Error> {
  let mut serial = Decoder::new().map_err(|fault| fault.decompose().0)?;
  let reference = decode_fixture(&mut serial)?;
  let frame_header = reference.inner.frame_hdr.as_ref().ok_or(Error::InvalidData)?;
  let header = &frame_header.tiling;
  let Ok(cols) = console_core_number_conversion::wrapped_index(header.cols);
  let Ok(rows) = console_core_number_conversion::wrapped_index(header.rows);
  assert!(
    cols.wrapping_mul(rows) > 1,
    "fixture must have multiple tiles"
  );
  let reference_digest = hash(&reference)?;
  let delimiter = STREAM.get(..2).ok_or(Error::InvalidData)?;
  assert_eq!(
    delimiter,
    &[0x12, 0],
    "temporal delimiter required for repetition"
  );
  let mut modes = vec![(8, 1)];

  match cfg!(feature = "unchecked").then_some(()) {
    Some(()) => {
      modes.extend([(8, 2), (8, 4)]);
    }
    None => {}
  }

  for (threads, max_frame_delay) in modes {
    let settings = Settings {
      threads,
      max_frame_delay,
      ..Default::default()
    };
    let mut decoder = open_fixture(settings)?;
    assert_eq!(
      decoder.ctx.fc.len(),
      ({
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(max_frame_delay);
        numeric_value
      })
    );
    assert_eq!(
      decoder.ctx.tc.len(),
      ({
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(threads);
        numeric_value
      })
    );
    let mut output = Vec::new();

    for _ in 0..12 {
      submit_fixture_packet(&mut decoder, &mut output)?;
    }

    let remaining = decoder.flush().map_err(|fault| fault.decompose().0)?;
    output.extend(remaining);
    assert_eq!(output.len(), 12, "frame contexts={max_frame_delay}");

    for frame in &output {
      let digest = hash(frame)?;
      assert_eq!(digest, reference_digest, "frame contexts={max_frame_delay}");
    }
  }

  Ok(())
}
