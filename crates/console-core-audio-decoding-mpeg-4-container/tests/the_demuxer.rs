mod dumps;

use std::fmt::Write;

use dumps::{Failure, Source, shortened, whole};

struct Fixture {
  name: &'static str,
  bytes: &'static [u8],
}

const FIXTURES: [Fixture; 23] = [
  Fixture {
    name: "aac.m4a",
    bytes: include_bytes!("fixtures/aac.m4a"),
  },
  Fixture {
    name: "alac.m4a",
    bytes: include_bytes!("fixtures/alac.m4a"),
  },
  Fixture {
    name: "av.mp4",
    bytes: include_bytes!("fixtures/av.mp4"),
  },
  Fixture {
    name: "dash-stream0.mp4",
    bytes: include_bytes!("fixtures/dash-stream0.mp4"),
  },
  Fixture {
    name: "flac.mp4",
    bytes: include_bytes!("fixtures/flac.mp4"),
  },
  Fixture {
    name: "frag.mp4",
    bytes: include_bytes!("fixtures/frag.mp4"),
  },
  Fixture {
    name: "frag2.mp4",
    bytes: include_bytes!("fixtures/frag2.mp4"),
  },
  Fixture {
    name: "mp3.mp4",
    bytes: include_bytes!("fixtures/mp3.mp4"),
  },
  Fixture {
    name: "opus.mp4",
    bytes: include_bytes!("fixtures/opus.mp4"),
  },
  Fixture {
    name: "mono.mov",
    bytes: include_bytes!("fixtures/mono.mov"),
  },
  Fixture {
    name: "pcm_f32le.mov",
    bytes: include_bytes!("fixtures/pcm_f32le.mov"),
  },
  Fixture {
    name: "pcm_f64le.mov",
    bytes: include_bytes!("fixtures/pcm_f64le.mov"),
  },
  Fixture {
    name: "pcm_s16be.mov",
    bytes: include_bytes!("fixtures/pcm_s16be.mov"),
  },
  Fixture {
    name: "pcm_s16le.mov",
    bytes: include_bytes!("fixtures/pcm_s16le.mov"),
  },
  Fixture {
    name: "pcm_s24le.mov",
    bytes: include_bytes!("fixtures/pcm_s24le.mov"),
  },
  Fixture {
    name: "pcm_s32le.mov",
    bytes: include_bytes!("fixtures/pcm_s32le.mov"),
  },
  Fixture {
    name: "pcm_u8.mov",
    bytes: include_bytes!("fixtures/pcm_u8.mov"),
  },
  Fixture {
    name: "aac_fast.m4a",
    bytes: include_bytes!("fixtures/aac_fast.m4a"),
  },
  Fixture {
    name: "alac_fast.m4a",
    bytes: include_bytes!("fixtures/alac_fast.m4a"),
  },
  Fixture {
    name: "av_fast.mp4",
    bytes: include_bytes!("fixtures/av_fast.mp4"),
  },
  Fixture {
    name: "flac_fast.mp4",
    bytes: include_bytes!("fixtures/flac_fast.mp4"),
  },
  Fixture {
    name: "opus_fast.mp4",
    bytes: include_bytes!("fixtures/opus_fast.mp4"),
  },
  Fixture {
    name: "pcm_s16le_fast.mov",
    bytes: include_bytes!("fixtures/pcm_s16le_fast.mov"),
  },
];

const STEP: u32 = 149;

const GOLDEN_SEEKABLE: &str = include_str!("golden/seekable.txt");

const GOLDEN_FORWARD: &str = include_str!("golden/forward.txt");

const GOLDEN_SHORTENED: &str = include_str!("golden/shortened.txt");

fn report_of(source: &Source) -> Result<String, Failure> {
  let mut report = String::new();

  for fixture in &FIXTURES {
    writeln!(report, "== {}", fixture.name)?;
    let dump = whole(fixture.bytes, source)?;
    report.push_str(&dump);
  }

  Ok(report)
}

fn shortened_report() -> Result<String, Failure> {
  let mut report = String::new();

  for fixture in &FIXTURES {
    writeln!(report, "== {} seekable", fixture.name)?;
    let seekable = shortened(fixture.bytes, STEP, &Source::Seekable)?;
    report.push_str(&seekable);
    writeln!(report, "== {} forward", fixture.name)?;
    let forward = shortened(fixture.bytes, STEP, &Source::Forward)?;
    report.push_str(&forward);
  }

  Ok(report)
}

#[test]
fn a_seekable_file_comes_out_as_it_always_did() -> Result<(), Failure> {
  let report = report_of(&Source::Seekable)?;
  assert_eq!(report, GOLDEN_SEEKABLE);
  Ok(())
}

#[test]
fn a_forward_only_stream_comes_out_as_it_always_did() -> Result<(), Failure> {
  let report = report_of(&Source::Forward)?;
  assert_eq!(report, GOLDEN_FORWARD);
  Ok(())
}

#[test]
fn a_cut_short_file_is_refused_or_ends_as_it_always_did() -> Result<(), Failure> {
  let report = shortened_report()?;
  assert_eq!(report, GOLDEN_SHORTENED);
  Ok(())
}
