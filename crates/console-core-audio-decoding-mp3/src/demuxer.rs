//! The reader of a layer three elementary stream: frames, one after another,
//! with nothing around them that says where they are.
//!
//! A stream like this has no index, so a seek is either a guess from the
//! proportions of the file, which a coarse seek makes and then walks forward
//! from, or a walk from the start, which an accurate one makes. Either lands
//! on the frame that holds the timestamp and then goes back as many frames as
//! its main data reaches into, because a decoder started there would
//! otherwise be missing the bytes the frame was coded against.

use std::io::{Seek, SeekFrom};

use console_core_audio_decoding_interfaces::codecs::CodecParameters;
use console_core_audio_decoding_interfaces::errors::{Result, SeekErrorKind, seek_error};
use console_core_audio_decoding_interfaces::formats::prelude::*;
use console_core_audio_decoding_interfaces::formats::util::trim_packet;
use console_core_audio_decoding_interfaces::io::{
  MediaSource, MediaSourceStream, ReadBytes, SeekBuffered, Seeking,
};
use console_core_audio_decoding_interfaces::meta::{Metadata, MetadataLog};
use console_core_audio_decoding_interfaces::probe::{Descriptor, Instantiate, QueryDescriptor};
use console_core_audio_decoding_interfaces::support_format;
use console_core_iteration::{Step, iterate};
use console_core_mpeg_audio_frames::header::{MPEG_HEADER_LEN, parse_frame_header};
use console_core_mpeg_audio_frames::xing::{
  Contents, XingInfoTag, contents, read_info_tag, read_vbri_tag,
};
use console_core_number_conversion::{fitted, index};

use crate::common::{FrameHeader, Infallible, channel_set, codec, finished};
use crate::frames::{
  estimate_num_mpeg_frames, read_main_data_begin, read_mpeg_frame, read_mpeg_frame_strict,
};
use crate::header::{MAX_MPEG_FRAME_SIZE, sync_frame};

const MAX_REF_FRAMES: u32 = 4;

pub struct MpaReader {
  reader: MediaSourceStream,
  tracks: Vec<Track>,
  cues: Vec<Cue>,
  metadata: MetadataLog,
  options: FormatOptions,
  first_packet_pos: u64,
  next_packet_ts: u64,
}

#[derive(Copy, Clone, Default)]
struct FramePos {
  ts: u64,
  pos: u64,
}

#[derive(Default)]
struct Scan {
  ring: [FramePos; 4],
  parsed: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Scanned {
  Found,
  Further,
}

impl QueryDescriptor for MpaReader {
  fn query() -> std::result::Result<&'static [Descriptor], console_core_never::Never> {
    Ok(&[
      support_format!(
        "mp1",
        "MPEG Audio Layer 1 Native",
        &["mp1"],
        &["audio/mpeg", "audio/mp1"],
        &[
          &[0xff, 0xfe],
          &[0xff, 0xff],
          &[0xff, 0xf6],
          &[0xff, 0xf7],
          &[0xff, 0xe6],
          &[0xff, 0xe7]
        ]
      ),
      support_format!(
        "mp2",
        "MPEG Audio Layer 2 Native",
        &["mp2"],
        &["audio/mpeg", "audio/mp2"],
        &[
          &[0xff, 0xfc],
          &[0xff, 0xfd],
          &[0xff, 0xf4],
          &[0xff, 0xf5],
          &[0xff, 0xe4],
          &[0xff, 0xe5]
        ]
      ),
      support_format!(
        "mp3",
        "MPEG Audio Layer 3 Native",
        &["mp3"],
        &["audio/mpeg", "audio/mp3"],
        &[
          &[0xff, 0xfa],
          &[0xff, 0xfb],
          &[0xff, 0xf2],
          &[0xff, 0xf3],
          &[0xff, 0xe2],
          &[0xff, 0xe3]
        ]
      ),
    ])
  }

  fn score(_context: &[u8]) -> std::result::Result<u8, console_core_never::Never> {
    Ok(255)
  }
}

fn scaled([value, by]: [u64; 2], over: u64) -> Infallible<Option<u64>> {
  let product = u128::from(value).saturating_mul(u128::from(by));

  Ok(product.checked_div(u128::from(over)).map(|scaled| {
    let Ok(scaled_2) = fitted::<u128, u64>(scaled);

    scaled_2
  }))
}

fn apply_info(
  params: &mut CodecParameters,
  tag: &XingInfoTag,
  header: &FrameHeader,
  options: &FormatOptions,
) -> Infallible<()> {
  let (delay, padding) = match &tag.lame {
    Some(lame) => {
      let Ok(_configured_with_delay) = params.with_delay(lame.enc_delay);
      let Ok(_configured_with_padding) = params.with_padding(lame.enc_padding);

      (lame.enc_delay, lame.enc_padding)
    }
    None => (0, 0),
  };

  for frames in tag.num_frames.into_iter() {
    let total = u64::from(frames).saturating_mul(header.duration);

    let counted = match options.gapless {
      GaplessPlayback::Enabled => total
        .saturating_sub(u64::from(delay))
        .saturating_sub(u64::from(padding)),
      GaplessPlayback::Disabled => total,
    };

    let Ok(_configured_with_n_frames) = params.with_n_frames(counted);
  }

  Ok(())
}

fn describe(
  source: &mut MediaSourceStream,
  params: &mut CodecParameters,
  header: &FrameHeader,
  packet: &[u8],
  options: &FormatOptions,
) -> Result<()> {
  let Ok(info) = read_info_tag(packet, header);
  let Ok(vbri) = read_vbri_tag(packet, header);

  match (info, vbri) {
    (Some(tag), _) => {
      let Ok(()) = apply_info(params, &tag, header, options);
    }
    (None, Some(tag)) => {
      let Ok(_configured_with_n_frames) =
        params.with_n_frames(u64::from(tag.num_mpeg_frames).saturating_mul(header.duration));
    }
    (None, None) => {
      let Ok(()) =
        source.seek_buffered_rev(u64::from(MPEG_HEADER_LEN.saturating_add(header.frame_size)));

      match source.seeking() {
        Seeking::Anywhere => {
          let Ok(estimate) = estimate_num_mpeg_frames(source);

          for frames in estimate.into_iter() {
            let Ok(_configured_with_n_frames) =
              params.with_n_frames(frames.saturating_mul(header.duration));
          }
        }
        Seeking::ForwardOnly => {}
      }
    }
  }

  Ok(())
}

impl FormatReader for MpaReader {
  fn try_new(mut source: MediaSourceStream, options: &FormatOptions) -> Result<Self> {
    let (header, packet) = read_mpeg_frame_strict(&mut source)?;
    let time_base = TimeBase::new(1, header.sample_rate)?;

    let Ok(mut params) = CodecParameters::new();

    let Ok(codec) = codec(header.layer);

    let Ok(_configured) = params.for_codec(codec);
    let Ok(_configured) = params.with_sample_rate(header.sample_rate);
    let Ok(_configured) = params.with_time_base(time_base);
    let Ok(channels) = channel_set(header.channel_mode);
    let Ok(_configured) = params.with_channels(channels);

    describe(&mut source, &mut params, &header, &packet, options)?;

    let first_packet_pos = source.pos();

    let Ok(value_1) = Track::new(0, params);

    Ok(MpaReader {
      reader: source,
      tracks: vec![value_1],
      cues: Vec::new(),
      metadata: MetadataLog::default(),
      options: *options,
      first_packet_pos,
      next_packet_ts: 0,
    })
  }

  fn next_packet(&mut self) -> Result<Packet> {
    let audio = iterate(&mut self.reader, |reader| {
      Ok(match read_mpeg_frame(reader) {
        Err(fault) => Step::Halt(Err(fault)),
        Ok((header, packet)) => {
          let Ok(found) = contents(&packet, &header);

          match found {
            Contents::Tag => Step::Again(reader),
            Contents::Audio => Step::Halt(Ok((header, packet))),
          }
        }
      })
    });

    let (header, packet) = finished(audio)?;

    let ts = self.next_packet_ts;

    self.next_packet_ts = ts.saturating_add(header.duration);

    let Ok(mut packet_2) =
      Packet::new_from_boxed_slice(0, ts, header.duration, packet.into_boxed_slice());

    match self.options.gapless {
      GaplessPlayback::Enabled => {
        for track in self.tracks.first().into_iter() {
          let delay = match track.codec_params.delay {
            Some(delay) => delay,
            None => 0,
          };

          let Ok(()) = trim_packet(&mut packet_2, delay, track.codec_params.n_frames);
        }
      }
      GaplessPlayback::Disabled => {}
    }

    Ok(packet_2)
  }

  fn metadata(&mut self) -> std::result::Result<Metadata<'_>, console_core_never::Never> {
    let Ok(value_2) = self.metadata.metadata();

    Ok(value_2)
  }

  fn cues(&self) -> std::result::Result<&[Cue], console_core_never::Never> {
    Ok(&self.cues)
  }

  fn tracks(&self) -> std::result::Result<&[Track], console_core_never::Never> {
    Ok(&self.tracks)
  }

  fn seek(&mut self, mode: SeekMode, to: SeekTo) -> Result<SeekedTo> {
    let desired_ts = self.desired_timestamp(to)?;
    let Ok(delay) = self.gapless(|params| params.delay);
    let required_ts = desired_ts.saturating_add(delay);
    let seeking = self.reader.seeking();

    match (seeking, required_ts < self.next_packet_ts) {
      (Seeking::ForwardOnly, true) => return seek_error(SeekErrorKind::ForwardOnly),
      (Seeking::Anywhere, _) | (Seeking::ForwardOnly, false) => {}
    }

    let preseek = match (mode, seeking) {
      (SeekMode::Coarse, Seeking::Anywhere) => self.preseek_coarse(required_ts),
      (SeekMode::Accurate, Seeking::Anywhere | Seeking::ForwardOnly) => {
        self.preseek_accurate(required_ts)
      }
      (SeekMode::Coarse, Seeking::ForwardOnly) => Ok(()),
    };

    preseek?;

    let found = iterate((&mut *self, Scan::default()), |(reader, mut scan)| {
      Ok(match reader.scan_frame(required_ts, &mut scan) {
        Ok(Scanned::Further) => Step::Again((reader, scan)),
        Ok(Scanned::Found) => Step::Halt(Ok(())),
        Err(fault) => Step::Halt(Err(fault)),
      })
    });

    finished(found)?;

    Ok(SeekedTo {
      track_id: 0,
      required_ts: required_ts.saturating_sub(delay),
      actual_ts: self.next_packet_ts.saturating_sub(delay),
    })
  }

  fn into_inner(
    self: Box<Self>,
  ) -> std::result::Result<MediaSourceStream, console_core_never::Never> {
    Ok(self.reader)
  }
}

impl MpaReader {
  fn desired_timestamp(&self, to: SeekTo) -> Result<u64> {
    match to {
      SeekTo::TimeStamp { ts, .. } => Ok(ts),
      SeekTo::Time { time, .. } => match self
        .tracks
        .first()
        .and_then(|track| track.codec_params.sample_rate)
      {
        Some(rate) => {
          let base = TimeBase::new(1, rate)?;

          let Ok(value_3) = base.calc_timestamp(time);

          Ok(value_3)
        }
        None => seek_error(SeekErrorKind::Unseekable),
      },
    }
  }

  fn gapless(&self, pick: fn(&CodecParameters) -> Option<u32>) -> Infallible<u64> {
    let chosen = self
      .tracks
      .first()
      .and_then(|track| pick(&track.codec_params));

    Ok(match (self.options.gapless, chosen) {
      (GaplessPlayback::Enabled, Some(chosen_2)) => u64::from(chosen_2),
      (GaplessPlayback::Enabled, None) | (GaplessPlayback::Disabled, _) => 0,
    })
  }

  fn preseek_coarse(&mut self, required_ts: u64) -> Result<()> {
    let Ok(delay) = self.gapless(|params| params.delay);
    let Ok(padding) = self.gapless(|params| params.padding);

    let total_byte_len = match self.reader.byte_len() {
      Some(len) => len,
      None => return seek_error(SeekErrorKind::Unseekable),
    };

    let duration = match self
      .tracks
      .first()
      .and_then(|track| track.codec_params.n_frames)
    {
      Some(frames) => frames.saturating_add(delay).saturating_add(padding),
      None => return seek_error(SeekErrorKind::Unseekable),
    };

    let audio_byte_len = total_byte_len.saturating_sub(self.first_packet_pos);
    let Ok(located) = scaled([required_ts, audio_byte_len], duration);

    let packet_pos = match located {
      Some(packet_pos) => packet_pos,
      None => return seek_error(SeekErrorKind::Unseekable),
    };

    let seek_pos = packet_pos
      .saturating_sub(MAX_MPEG_FRAME_SIZE)
      .saturating_add(self.first_packet_pos);

    self.reader.seek(SeekFrom::Start(seek_pos))?;

    let (header, _) = read_mpeg_frame_strict(&mut self.reader)?;

    let seeked = self.reader.pos().saturating_sub(self.first_packet_pos);
    let Ok(estimated) = scaled([seeked, duration], audio_byte_len);

    let ts = match estimated {
      Some(ts) => ts,
      None => return seek_error(SeekErrorKind::Unseekable),
    };

    self.next_packet_ts = ts
      .div_euclid(header.duration)
      .saturating_mul(header.duration);

    Ok(())
  }

  fn preseek_accurate(&mut self, required_ts: u64) -> Result<()> {
    match ::core::primitive::bool::then_some(required_ts < self.next_packet_ts, ()) {
      ::core::option::Option::None => Ok(()),
      ::core::option::Option::Some(()) => {
        let seeked_pos = self.reader.seek(SeekFrom::Start(self.first_packet_pos))?;

        match ::core::primitive::bool::then_some(seeked_pos == self.first_packet_pos, ()) {
          ::core::option::Option::None => seek_error(SeekErrorKind::Unseekable),
          ::core::option::Option::Some(()) => {
            self.next_packet_ts = 0;

            Ok(())
          }
        }
      }
    }
  }

  fn scan_frame(&mut self, required_ts: u64, scan: &mut Scan) -> Result<Scanned> {
    let word = sync_frame(&mut self.reader)?;
    let header = parse_frame_header(word)?;
    let pos = self.reader.pos().saturating_sub(u64::from(MPEG_HEADER_LEN));
    let Ok(slot) = index(scan.parsed & MAX_REF_FRAMES.saturating_sub(1));

    for entry in scan.ring.get_mut(slot).into_iter() {
      *entry = FramePos {
        pos,
        ts: self.next_packet_ts,
      };
    }

    scan.parsed = scan.parsed.saturating_add(1);

    match ::core::primitive::bool::then_some(
      self.next_packet_ts.saturating_add(header.duration) > required_ts,
      (),
    ) {
      ::core::option::Option::Some(()) => self.reference(&header, pos, scan),
      ::core::option::Option::None => self.pass(&header),
    }
  }

  fn pass(&mut self, header: &FrameHeader) -> Result<Scanned> {
    self.reader.ignore_bytes(u64::from(header.frame_size))?;

    self.next_packet_ts = self.next_packet_ts.saturating_add(header.duration);

    Ok(Scanned::Further)
  }

  fn reference(&mut self, header: &FrameHeader, pos: u64, scan: &Scan) -> Result<Scanned> {
    let begin = read_main_data_begin(&mut self.reader, header)?;
    let main_data_begin = u64::from(begin);

    let newest_first = (0..scan.parsed.min(MAX_REF_FRAMES)).filter_map(|back| {
      let Ok(slot) =
        index(scan.parsed.wrapping_sub(back).wrapping_sub(1) & MAX_REF_FRAMES.wrapping_sub(1));

      scan.ring.get(slot).copied()
    });

    let chosen = match main_data_begin {
      0 => newest_first.clone().next(),
      reach => newest_first
        .clone()
        .find(|frame| pos.saturating_sub(frame.pos) >= reach)
        .or_else(|| newest_first.clone().next_back()),
    };

    match chosen {
      Some(chosen_2) => {
        self.next_packet_ts = chosen_2.ts;
        self.reader.seek_buffered(chosen_2.pos);

        Ok(Scanned::Found)
      }
      None => seek_error(SeekErrorKind::OutOfRange),
    }
  }
}
