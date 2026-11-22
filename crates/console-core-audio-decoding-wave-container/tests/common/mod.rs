use std::io::Cursor;
use std::num::TryFromIntError;

use console_core_audio_decoding_interfaces::errors::Error;
use console_core_audio_decoding_interfaces::formats::{
  FormatOptions, FormatReader, SeekMode, SeekTo, SeekedTo,
};
use console_core_audio_decoding_interfaces::units::Time;
use console_core_byte_streams::{MediaSourceStream, MediaSourceStreamOptions, ReadOnlySource};
use console_core_never::Never;

pub type Failure = Box<dyn std::error::Error>;

const SEEK_TIMESTAMPS: [u64; 8] = [0, 1, 1151, 1152, 1153, 2304, 3000, 100_000_000];

pub fn length(body: &[u8]) -> Result<u32, TryFromIntError> {
  u32::try_from(body.len())
}

pub fn samples(count: u32) -> Result<Vec<u8>, Never> {
  Ok(
    (0..count)
      .map(|index| index.wrapping_mul(7))
      .filter_map(|value| value.to_le_bytes().first().copied())
      .collect(),
  )
}

fn seekable(bytes: &[u8]) -> Result<MediaSourceStream, Never> {
  MediaSourceStream::new(
    Box::new(Cursor::new(bytes.to_vec())),
    MediaSourceStreamOptions::default(),
  )
}

fn unseekable(bytes: &[u8]) -> Result<MediaSourceStream, Never> {
  let Ok(forward) = ReadOnlySource::new(Cursor::new(bytes.to_vec()));

  MediaSourceStream::new(Box::new(forward), MediaSourceStreamOptions::default())
}

fn shown(outcome: Result<SeekedTo, Error>) -> Result<String, Never> {
  Ok(outcome.map_or_else(
    |error| error.to_string(),
    |seeked| {
      format!(
        "{} {} {}",
        seeked.track_id, seeked.required_ts, seeked.actual_ts
      )
    },
  ))
}

fn seek_fresh<R: FormatReader>(bytes: &[u8], to: SeekTo) -> Result<SeekedTo, Error> {
  let Ok(stream) = seekable(bytes);
  let mut reader = R::try_new(stream, &FormatOptions::default())?;

  reader.seek(SeekMode::Accurate, to)
}

fn forward_then_back<R: FormatReader>(bytes: &[u8]) -> Result<String, Never> {
  let Ok(stream) = unseekable(bytes);
  let outcome = R::try_new(stream, &FormatOptions::default()).and_then(|mut reader| {
    let first = reader.seek(
      SeekMode::Accurate,
      SeekTo::TimeStamp {
        ts: 1152,
        track_id: 0,
      },
    )?;
    let second = reader.seek(SeekMode::Accurate, SeekTo::TimeStamp { ts: 0, track_id: 0 })?;

    Ok((first.actual_ts, second.actual_ts))
  });

  Ok(format!(
    "unseekable forward then back: {}",
    outcome.map_or_else(|error| error.to_string(), |seeked| format!("{seeked:?}"))
  ))
}

fn seeks<R: FormatReader>(bytes: &[u8]) -> Result<Vec<String>, Never> {
  let Ok(value_1) = Time::new(0, 0.0);
  let Ok(value_2) = Time::new(0, 0.25);
  let Ok(value_3) = Time::new(1, 0.5);

  let times = [value_1, value_2, value_3];
  let mut lines = Vec::new();

  for ts in SEEK_TIMESTAMPS {
    let Ok(text) = shown(seek_fresh::<R>(
      bytes,
      SeekTo::TimeStamp { ts, track_id: 0 },
    ));

    lines.push(format!("seek ts {ts}: {text}"));
  }

  for time in times {
    let Ok(text) = shown(seek_fresh::<R>(
      bytes,
      SeekTo::Time {
        time,
        track_id: None,
      },
    ));

    lines.push(format!("seek time {time:?}: {text}"));
  }

  let Ok(last) = forward_then_back::<R>(bytes);

  lines.push(last);

  Ok(lines)
}

fn describe_packets<R: FormatReader>(reader: &mut R) -> Result<Vec<String>, Never> {
  let mut lines = Vec::new();

  for _ in 0..6 {
    match reader.next_packet() {
      Ok(packet) => {
        let Ok(value_4) = packet.track_id();
        let Ok(value_5) = packet.dur();
        let Ok(value_6) = packet.buf();
        let Ok(value_7) = packet.buf();

        lines.push(format!(
          "packet track={} ts={} dur={} len={} head={:?}",
          value_4,
          packet.ts,
          value_5,
          value_6.len(),
          value_7.iter().take(4).collect::<Vec<_>>()
        ))
      }
      Err(_ended) => break,
    }
  }

  lines.push(match reader.next_packet() {
    Ok(_) => "more packets".to_string(),
    Err(error) => format!("then {error}"),
  });

  Ok(lines)
}

fn describe_open<R: FormatReader>(mut reader: R, bytes: &[u8]) -> Result<String, Never> {
  let mut lines = Vec::new();

  let Ok(value_8) = reader.tracks();

  for track in value_8 {
    let params = &track.codec_params;

    lines.push(format!(
            "track {} {:?} rate={:?} base={:?} frames={:?} bits={:?} coded={:?} channels={:?} max={:?} block={:?}",
            track.id,
            params.codec,
            params.sample_rate,
            params.time_base,
            params.n_frames,
            params.bits_per_sample,
            params.bits_per_coded_sample,
            params.channels.map(|channels| {
                let Ok(bits) = channels.bits();

                bits
            }),
            params.max_frames_per_packet,
            params.frames_per_block,
        ));
  }

  let Ok(value_9) = reader.cues();

  lines.push(format!("cues {}", value_9.len()));

  let Ok(current) = reader.metadata();
  let Ok(revision) = current.current();

  match revision {
    Some(revision_2) => {
      let Ok(value_10) = revision_2.tags();

      lines.extend(value_10.iter().map(|tag| format!("tag {tag:?}")))
    }
    None => (),
  }

  let Ok(packets) = describe_packets(&mut reader);
  let Ok(sought) = seeks::<R>(bytes);

  lines.extend(packets);
  lines.extend(sought);

  Ok(lines.join("\n"))
}

pub fn describe<R: FormatReader>(bytes: Vec<u8>) -> Result<String, Never> {
  let Ok(stream) = seekable(&bytes);

  match R::try_new(stream, &FormatOptions::default()) {
    Ok(reader) => describe_open(reader, &bytes),
    Err(error) => Ok(format!("open failed: {error}")),
  }
}
