//! Exercise the installed playback graph against a private, virtual output.

#![cfg(target_os = "linux")]

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, narrow_f32};
use console_waiting::{Outcome, Ready, Schedule};

const RATE: u32 = 48_000;
const SEGMENT: u32 = RATE.wrapping_mul(3).wrapping_div(5);
const FRAME_BYTES: u32 = 8;
const SILENCE: f64 = 0.0;
const SOFT: f64 = 0.0005;
const NORMAL: f64 = 0.018;
const LOUD: f64 = 0.25;
const FLOOR: f64 = 0.00005;
const MIX_CEILING: f64 = 0.05682;
const LEVELS: [f64; 14] = [
  NORMAL, SILENCE, LOUD, SILENCE, SOFT, SILENCE, NORMAL, LOUD, SOFT, LOUD, NORMAL, SOFT, SILENCE,
  FLOOR,
];
const AUDIO: [&str; 9] = [
  "--raw",
  "--format",
  "f32",
  "--rate",
  "48000",
  "--channels",
  "2",
  "--latency",
  "20ms",
];

#[derive(Clone, Copy)]
enum Leveling {
  Enabled,
  Disabled,
}

#[derive(Clone, Copy)]
enum Transient {
  Noise,
  InterSample,
}

#[derive(Clone, Copy)]
struct Playback<'a> {
  properties: &'a str,
  transients: &'a [Range<u32>],
  leveling: Leveling,
  transient: Transient,
}

impl Playback<'_> {
  const DEFAULT: Self = Self {
    properties: "{}",
    transients: &[],
    leveling: Leveling::Enabled,
    transient: Transient::Noise,
  };
}

type StereoFrame = (f64, f64);

struct Session {
  path: PathBuf,
  files: PathBuf,
  children: Vec<Child>,
}

#[derive(Clone, Copy)]
enum Log {
  Pipewire,
  Pulse,
  Wireplumber,
  Monitor,
  Player,
  Gain,
}

impl Log {
  fn filename(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Pipewire => "pipewire.log",
      Self::Pulse => "pulse.log",
      Self::Wireplumber => "wireplumber.log",
      Self::Monitor => "monitor.log",
      Self::Player => "player.log",
      Self::Gain => "gain.log",
    })
  }
}

impl Session {
  fn new(test: &str) -> io::Result<Self> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .and_then(Path::parent)
      .ok_or_else(|| io::Error::other("cannot locate repository files"))?;
    let path = console_core_temporary_directories::private(test).map_err(io::Error::other)?;
    let session = Self {
      path,
      files: root.join("files"),
      children: Vec::new(),
    };
    let config = session.path.join("wireplumber.conf.d/90-test.conf");
    console_core_atomic_writes::whole_with_folders(
      &config,
      b"wireplumber.profiles = { main = { hardware.audio = disabled hardware.bluetooth = disabled hardware.video-capture = disabled } }",
    ).map_err(io::Error::other)?;

    Ok(session)
  }

  fn command(&self, name: &str) -> Result<Command, Never> {
    let mut command = Command::new(name);
    command
      .env("XDG_CONFIG_HOME", &self.path)
      .env("XDG_STATE_HOME", &self.path)
      .env("XDG_RUNTIME_DIR", &self.path)
      .env("PIPEWIRE_RUNTIME_DIR", &self.path)
      .env("PIPEWIRE_REMOTE", "pipewire-0")
      .env("PIPEWIRE_CONFIG_DIR", "/usr/share/pipewire")
      .env("LADSPA_PATH", self.path.join("console-playback-gain"))
      .env(
        "DBUS_SESSION_BUS_ADDRESS",
        format!("unix:path={}/private-bus", self.path.display()),
      )
      .env(
        "WIREPLUMBER_CONFIG_DIR",
        format!(
          "{}:{}/etc/wireplumber:/usr/share/wireplumber",
          self.path.display(),
          self.files.display()
        ),
      )
      .env(
        "WIREPLUMBER_DATA_DIR",
        format!(
          "{}/usr/local/share/wireplumber:/usr/share/wireplumber",
          self.files.display()
        ),
      );

    Ok(command)
  }

  fn log_file(&self, log: Log) -> Result<PathBuf, Never> {
    Ok(match log {
      Log::Pipewire => self.path.join("pipewire.log"),
      Log::Pulse => self.path.join("pulse.log"),
      Log::Wireplumber => self.path.join("wireplumber.log"),
      Log::Monitor => self.path.join("monitor.log"),
      Log::Player => self.path.join("player.log"),
      Log::Gain => self.path.join("gain.log"),
    })
  }

  fn start(&mut self, process: Log, mut command: Command) -> io::Result<()> {
    let Ok(path) = self.log_file(process);
    let log = fresh_stream(&path)?;
    let child = command.stderr(log).spawn()?;
    self.children.push(child);

    Ok(())
  }

  fn linked(&self, name: &str) -> Result<Ready, Never> {
    let Ok(mut command) = self.command("pw-link");
    let output = match command.arg("-l").output() {
      Ok(output) => output,
      Err(_link_query_unavailable) => return Ok(Ready::NotYet),
    };

    match std::str::from_utf8(&output.stdout) {
      Ok(list) => Ok(
        match (output.status.success() && list.contains(name)).then_some(()) {
          Some(()) => Ready::Yes,
          None => Ready::NotYet,
        },
      ),
      Err(_not_pipewire_text) => Ok(Ready::NotYet),
    }
  }

  fn failure(&self, fault: io::Error) -> Result<io::Error, Never> {
    let mut report = fault.to_string();

    for log in [
      Log::Pipewire,
      Log::Pulse,
      Log::Wireplumber,
      Log::Monitor,
      Log::Player,
      Log::Gain,
    ] {
      let Ok(path) = self.log_file(log);
      let Ok(name) = log.filename();
      let detail = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(read_fault) => format!("unavailable: {read_fault}"),
      };
      report.push_str(&format!("\n{name}\n{detail}"));
    }

    Ok(io::Error::other(report))
  }
}

#[cfg_attr(
  dylint_lib = "explicit040_no_torn_write",
  expect(
    explicit040_no_torn_write,
    reason = "each child writes a new stream in its private test directory; recordings are inspected only after reaching the expected byte count"
  )
)]
fn fresh_stream(path: &Path) -> io::Result<File> {
  File::create_new(path)
}

impl Drop for Session {
  fn drop(&mut self) {
    for child in self.children.iter_mut().rev() {
      drop(child.kill());
      drop(child.wait());
    }

    drop(fs::remove_dir_all(&self.path));
  }
}

fn wait_for(what: &str, ready: impl FnMut() -> Result<Ready, Never>) -> io::Result<()> {
  let Ok(schedule) = Schedule::asking_every(Duration::from_secs(15), Duration::from_millis(25));
  let Ok(outcome) = console_waiting::until(schedule, ready);

  match outcome {
    Outcome::Happened => Ok(()),
    Outcome::RanOut => Err(io::Error::new(io::ErrorKind::TimedOut, what)),
  }
}

fn command_succeeded(output: Output) -> io::Result<()> {
  match output.status.success().then_some(()) {
    Some(()) => Ok(()),
    None => {
      let detail = match String::from_utf8(output.stderr) {
        Ok(text) => text,
        Err(invalid) => format!("non-UTF-8 process output: {:?}", invalid.into_bytes()),
      };
      Err(io::Error::other(detail))
    }
  }
}

fn noise() -> Result<Vec<f64>, Never> {
  let mut state = 123_u32;
  let mut low_pass = 0.0;
  let mut samples = Vec::new();

  for _frame in 0..SEGMENT {
    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    let white = f64::from(state) / f64::from(u32::MAX) * 2.0 - 1.0;
    low_pass = 0.55 * low_pass + 0.45 * white;
    samples.push(low_pass);
  }

  Ok(samples)
}

fn fixture(levels: &[f64], variant: Playback<'_>) -> Result<Vec<u8>, Never> {
  let Ok(samples) = noise();
  let power = samples.iter().map(|sample| sample * sample).sum::<f64>();
  let rms = (power / f64::from(SEGMENT)).sqrt();
  let impulses: BTreeSet<_> = variant.transients.iter().cloned().flatten().collect();
  let mut playback = Vec::new();
  let mut frame = 0_u32;

  for &level in levels {
    for sample in &samples {
      let value = match impulses.contains(&frame).then_some(()) {
        Some(()) => match variant.transient {
          Transient::Noise => sample.signum(),
          Transient::InterSample => match frame.wrapping_rem(4) {
            0 | 1 => 1.0,
            _ => -1.0,
          },
        },
        None => sample * level / rms,
      };
      let Ok(left) = narrow_f32(value);
      playback.extend_from_slice(&left.to_le_bytes());
      playback.extend_from_slice(&(left * -0.5).to_le_bytes());
      frame = frame.saturating_add(1);
    }
  }

  Ok(playback)
}

fn prepare(session: &mut Session, leveling: Leveling) -> io::Result<()> {
  let Ok(mut installer) = session.command(env!("CARGO_BIN_EXE_console-sound-effect"));
  let installed = installer.arg("--prepare-gain-meter").output()?;
  command_succeeded(installed)?;
  let Ok(pipewire) = session.command("pipewire");
  session.start(Log::Pipewire, pipewire)?;
  let socket = session.path.join("pipewire-0");
  wait_for("private PipeWire socket", || {
    Ok(match fs::metadata(&socket) {
      Ok(_) => Ready::Yes,
      Err(_not_listening_yet) => Ready::NotYet,
    })
  })?;
  let Ok(wireplumber) = session.command("wireplumber");
  session.start(Log::Wireplumber, wireplumber)?;

  let Ok(mut cli) = session.command("pw-cli");
  let created = cli.args([
    "create-node", "adapter", "{ factory.name = support.null-audio-sink node.name = test-output media.class = Audio/Sink object.linger = true audio.position = [ FL FR ] }",
  ]).output()?;
  command_succeeded(created)?;
  wait_for("balanced filter linked to the test output", || {
    session.linked("console.balance.")
  })?;

  match leveling {
    Leveling::Enabled => Ok(()),
    Leveling::Disabled => {
      let Ok(mut control) = session.command("wpctl");
      let changed = control
        .args([
          "settings",
          "console.balanced-playback.automatic-leveling",
          "false",
        ])
        .output()?;
      command_succeeded(changed)
    }
  }
}

fn record(session: &mut Session, levels: &[f64], playback: Playback<'_>) -> io::Result<Vec<u8>> {
  prepare(session, playback.leveling)?;
  let source = session.path.join("playback.f32");
  let Ok(fixture) = fixture(levels, playback);
  console_core_atomic_writes::whole(&source, &fixture).map_err(io::Error::other)?;
  let Ok(segments) = fitted::<_, u32>(levels.len());
  let record_frames = segments.saturating_mul(SEGMENT).saturating_add(RATE);
  let captured = session.path.join("captured.f32");
  let Ok(mut monitor) = session.command("pw-cat");
  let recording = fresh_stream(&captured)?;
  monitor
    .arg("--record")
    .args(AUDIO)
    .args([
      "--target",
      "test-output",
      "--properties",
      "{ stream.capture.sink = true node.name = test-monitor }",
      "--sample-count",
      &record_frames.to_string(),
      "-",
    ])
    .stdout(recording);
  session.start(Log::Monitor, monitor)?;
  wait_for("monitor linked to the test output", || {
    session.linked("test-monitor")
  })?;

  let Ok(mut player) = session.command("pw-cat");
  let input = File::open(source)?;
  player
    .arg("--playback")
    .args(AUDIO)
    .args([
      "--target",
      "test-output",
      "--properties",
      playback.properties,
      "-",
    ])
    .stdin(input)
    .stdout(Stdio::null());
  session.start(Log::Player, player)?;
  let bytes = u64::from(record_frames).saturating_mul(u64::from(FRAME_BYTES));
  wait_for("complete monitor recording", || {
    Ok(match fs::metadata(&captured) {
      Ok(data) => match (data.len() >= bytes).then_some(()) {
        Some(()) => Ready::Yes,
        None => Ready::NotYet,
      },
      Err(_recording_not_ready) => Ready::NotYet,
    })
  })?;
  fs::read(captured)
}

fn monitored(test: &str, levels: &[f64], playback: Playback<'_>) -> io::Result<Vec<StereoFrame>> {
  let mut session = Session::new(test)?;
  let captured = record(&mut session, levels, playback).map_err(|fault| {
    let Ok(report) = session.failure(fault);
    report
  })?;
  let (frames, _) = captured.as_chunks::<8>();
  let samples: Vec<_> = frames
    .iter()
    .map(|frame| {
      let [a, b, c, d, e, f, g, h] = *frame;
      (
        f64::from(f32::from_le_bytes([a, b, c, d])),
        f64::from(f32::from_le_bytes([e, f, g, h])),
      )
    })
    .collect();
  let start = samples
    .iter()
    .zip(0_u64..)
    .find_map(|((left, _), position)| (left.abs() > 0.000_001).then_some(position))
    .ok_or_else(|| io::Error::other("playback never reached the output"))?;
  let Ok(segments) = fitted::<_, u64>(levels.len());
  let end = start.saturating_add(segments.saturating_mul(u64::from(SEGMENT)));
  let Ok(from) = index(start);
  let Ok(to) = index(end);
  let measured = samples
    .get(from..to)
    .ok_or_else(|| io::Error::other("the monitor stopped before playback ended"))?;

  Ok(measured.to_vec())
}

#[test]
#[ignore = "starts private WirePlumber and PipeWire sessions on a Linux host"]
fn full_scale_surprises_stay_bounded_after_quiet_playback() -> io::Result<()> {
  let levels = [
    NORMAL, SOFT, SOFT, SOFT, SOFT, SILENCE, SILENCE, SILENCE, SILENCE,
  ];
  let after_quiet = SEGMENT.saturating_mul(5);
  let after_silence = SEGMENT.saturating_mul(8);
  let burst_frames = RATE.wrapping_div(40);
  let transients = [
    after_quiet..after_quiet.saturating_add(1),
    after_silence..after_silence.saturating_add(burst_frames),
  ];

  for (mode, playback) in [
    (
      "live",
      Playback {
        transients: &transients,
        ..Playback::DEFAULT
      },
    ),
    (
      "fixed-unity",
      Playback {
        properties: "{ console.balance.indexed-gain = 1.0 }",
        transients: &transients,
        ..Playback::DEFAULT
      },
    ),
    (
      "disabled",
      Playback {
        transients: &transients,
        leveling: Leveling::Disabled,
        ..Playback::DEFAULT
      },
    ),
  ] {
    let samples = monitored(mode, &levels, playback)?;
    assert_surprises_are_bounded(&samples, mode, &transients)?;
  }

  Ok(())
}

#[test]
#[ignore = "starts a private WirePlumber and PipeWire session on a Linux host"]
fn output_bounds_inter_sample_peaks_with_leveling_disabled() -> io::Result<()> {
  let start = SEGMENT.saturating_mul(2);
  let transients = [start..start.saturating_add(128)];
  let samples = monitored(
    "inter-sample",
    &[NORMAL, SILENCE, SILENCE, SILENCE],
    Playback {
      transients: &transients,
      transient: Transient::InterSample,
      leveling: Leveling::Disabled,
      ..Playback::DEFAULT
    },
  )?;
  let slack = RATE.wrapping_div(100);
  let burst = frame_range(
    &samples,
    start.saturating_sub(slack)..start.saturating_add(slack),
  )?;
  let Ok(peak) = reconstructed_peak(burst);
  assert!(peak > 0.025, "the high-frequency burst was missing: {peak}");
  assert!(
    peak <= MIX_CEILING + 0.001,
    "inter-sample peak reached {peak}"
  );

  Ok(())
}

fn reconstructed_peak(samples: &[StereoFrame]) -> Result<f64, Never> {
  let mut peak = 0.0_f64;

  for phase in 0..8 {
    let mut coefficients = Vec::new();

    for tap in 0..24 {
      let age = f64::from(tap);
      let x = age - 11.0 - f64::from(phase) / 8.0;
      let sinc = match (phase, tap) {
        (0, 11) => 1.0,
        _ => (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x),
      };
      let taper = 0.5 - 0.5 * (std::f64::consts::TAU * age / 23.0).cos();
      coefficients.push(sinc * taper);
    }

    let sum: f64 = coefficients.iter().sum();

    for window in samples.windows(24) {
      let mix = window
        .iter()
        .zip(&coefficients)
        .map(|((left, _), weight)| left * weight)
        .sum::<f64>();
      peak = peak.max((mix / sum).abs());
    }
  }

  Ok(peak)
}

fn frame_range(samples: &[StereoFrame], frames: Range<u32>) -> io::Result<&[StereoFrame]> {
  let Ok(start) = index(frames.start);
  let Ok(end) = index(frames.end);

  samples
    .get(start..end)
    .ok_or_else(|| io::Error::other("missing part of monitor recording"))
}

fn assert_surprises_are_bounded(
  samples: &[StereoFrame],
  mode: &str,
  [impulse, burst]: &[Range<u32>; 2],
) -> io::Result<()> {
  let normal = settled_rms(samples)?;
  let peak = samples
    .iter()
    .map(|(left, right)| left.abs().max(right.abs()))
    .fold(0.0, f64::max);
  assert!(
    peak <= MIX_CEILING,
    "{mode} peak {peak} exceeded the mix ceiling"
  );
  let slack = RATE.wrapping_div(100);
  let impulse_frames = frame_range(
    samples,
    impulse.start.saturating_sub(slack)..impulse.start.saturating_add(slack),
  )?;
  assert!(
    impulse_frames.iter().any(|(left, _)| left.abs() > 0.025),
    "{mode} impulse did not reach the recording"
  );
  let attack = frame_range(
    samples,
    burst.start.saturating_sub(slack)..burst.end.saturating_add(slack),
  )?;
  const ONSET_FRAMES: u32 = RATE.wrapping_div(200);
  let Ok(window) = index(ONSET_FRAMES);
  let onset = attack
    .windows(window.max(1))
    .map(|frames| {
      (frames.iter().map(|(left, _)| left * left).sum::<f64>() / f64::from(ONSET_FRAMES)).sqrt()
    })
    .fold(0.0, f64::max);

  eprintln!("{mode}: normal RMS {normal:.5}, burst RMS {onset:.5}, peak {peak:.5}");
  assert!(onset > 0.002, "{mode} burst did not reach the recording");
  assert!(
    onset <= 2.0 * normal,
    "{mode} burst was over 6 dB above normal"
  );

  Ok(())
}

#[test]
#[ignore = "starts a private WirePlumber and PipeWire session on a Linux host"]
fn recent_loud_passages_temper_live_boost_after_a_pause() -> io::Result<()> {
  let levels = [
    SOFT, LOUD, LOUD, LOUD, LOUD, LOUD, LOUD, LOUD, LOUD, LOUD, SILENCE, SILENCE, SILENCE, SOFT,
  ];
  let measured = monitored("history", &levels, Playback::DEFAULT)?;
  let fresh = settled_rms(&measured)?;
  let Ok(segments) = fitted::<_, u32>(levels.len());
  let Ok(last_start) = index(SEGMENT.saturating_mul(segments.saturating_sub(1)));
  let last = measured
    .get(last_start..)
    .ok_or_else(|| io::Error::other("missing quiet passage after the pause"))?;
  let after = settled_rms(last)?;
  let ratio = after / fresh;
  assert!(
    (0.80..0.93).contains(&ratio),
    "live boost after a loud passage and a pause was {ratio:.3} of the fresh level"
  );
  Ok(())
}

fn settled_rms(samples: &[StereoFrame]) -> io::Result<f64> {
  let samples_2 = frame_range(samples, RATE.wrapping_div(3)..RATE.wrapping_div(2))?;
  let sum = samples_2.iter().map(|(left, _)| left * left).sum::<f64>();
  let Ok(count) = console_core_number_conversion::fitted::<_, u32>(samples_2.len());
  Ok((sum / f64::from(count)).sqrt())
}

#[test]
#[ignore = "starts a private WirePlumber and PipeWire session on a Linux host"]
fn balances_loud_normal_and_very_quiet_noise_in_isolation_and_in_sequence()
-> Result<(), Box<dyn std::error::Error>> {
  let measured = monitored("levels", &LEVELS, Playback::DEFAULT)?;

  for &(left, right) in &measured {
    assert!(
      left.abs() <= 0.08034,
      "sample peak exceeded the bell-based ceiling"
    );
    assert!(
      (right + 0.5 * left).abs() < 0.000_001,
      "stereo balance changed"
    );
  }

  let Ok(segment) = index(SEGMENT);
  let steady = measured
    .chunks_exact(segment.max(1))
    .map(settled_rms)
    .collect::<Result<Vec<_>, _>>()?;
  let normal =
    ::core::result::Result::<&_, _>::copied(steady.first().ok_or("missing normal reference"))?;
  assert!(normal > 0.01, "normal noise was too quiet: {normal}");

  for (index, (&level, output)) in LEVELS.iter().zip(steady).enumerate() {
    match level {
      SILENCE => assert!(
        output < 0.000_001,
        "silence at segment {index} became audible"
      ),
      FLOOR => assert!(
        output <= FLOOR * 1.05,
        "noise-floor signal at segment {index} was boosted"
      ),
      _ => {
        let difference = 20.0 * (output / normal).log10();
        eprintln!(
          "segment {}: {:.1} dBFS in, {:.1} dB from normal out",
          index.saturating_add(1),
          20.0 * level.log10(),
          difference
        );
        assert!(
          difference.abs() < 3.0,
          "segment {index} was {difference:.2} dB from normal"
        );
      }
    }
  }

  Ok(())
}

#[test]
#[ignore = "starts private WirePlumber and PipeWire sessions on a Linux host"]
fn a_lower_app_volume_stays_lower_after_leveling() -> io::Result<()> {
  let normal = monitored(
    "app-normal",
    &[LOUD],
    Playback {
      properties: "{ state.default-volume = 1.0 }",
      ..Playback::DEFAULT
    },
  )?;
  let reduced = monitored(
    "app-reduced",
    &[LOUD],
    Playback {
      properties: "{ state.default-volume = 0.125 }",
      ..Playback::DEFAULT
    },
  )?;
  let normal_level = settled_rms(&normal)?;
  let reduced_level = settled_rms(&reduced)?;
  let ratio = reduced_level / normal_level;

  assert!(
    (ratio - 0.125).abs() < 0.01,
    "app volume changed by a factor of {ratio}"
  );
  Ok(())
}

#[test]
#[ignore = "starts a private WirePlumber and PipeWire session on a Linux host"]
fn indexed_playback_keeps_one_gain_across_quiet_and_loud_passages() -> io::Result<()> {
  let loud = 0.05;
  let quiet = 0.005;
  let samples = monitored(
    "indexed",
    &[loud, quiet, loud],
    Playback {
      properties: "{ console.balance.indexed-gain = 0.25 }",
      ..Playback::DEFAULT
    },
  )?;
  let Ok(segment) = index(SEGMENT);
  let levels = samples
    .chunks_exact(segment.max(1))
    .map(settled_rms)
    .collect::<Result<Vec<_>, _>>()?;

  let (first, middle, last) = match levels.as_slice() {
    [first, middle, last] => (first, middle, last),
    _ => return Err(io::Error::other("missing indexed passage")),
  };

  assert!(
    (first / loud - 0.25).abs() < 0.01,
    "first passage gained {first}"
  );
  assert!(
    (middle / quiet - 0.25).abs() < 0.01,
    "quiet passage gained {middle}"
  );
  assert!(
    (last / loud - 0.25).abs() < 0.01,
    "last passage gained {last}"
  );
  Ok(())
}

#[test]
#[ignore = "starts a private WirePlumber and PipeWire session on a Linux host"]
fn live_gain_readout_tracks_attenuation_and_boost() -> io::Result<()> {
  let mut session = Session::new("gain-meter")?;
  exercise_gain_meter(&mut session, MeterPlayer::Native).map_err(|fault| {
    let Ok(report) = session.failure(fault);
    report
  })
}

#[test]
#[ignore = "starts a private WirePlumber, PipeWire and Pulse session on a Linux host"]
fn pulse_streams_report_gain_without_application_configuration() -> io::Result<()> {
  let mut session = Session::new("pulse-gain-meter")?;
  exercise_gain_meter(&mut session, MeterPlayer::Pulse).map_err(|fault| {
    let Ok(report) = session.failure(fault);
    report
  })
}

enum MeterPlayer {
  Native,
  Pulse,
}

fn pulse_meter(session: &mut Session) -> io::Result<Command> {
  let config = session
    .path
    .join("pipewire/pipewire-pulse.conf.d/60-console-playback-gain.conf");
  let installed = session
    .files
    .join("etc/pipewire/pipewire-pulse.conf.d/60-console-playback-gain.conf");
  let bytes = fs::read(installed)?;
  console_core_atomic_writes::whole_with_folders(&config, &bytes).map_err(io::Error::other)?;
  let Ok(mut pulse) = session.command("pipewire-pulse");
  let socket = session.path.join("pulse/native");
  pulse
    .env_remove("PIPEWIRE_CONFIG_DIR")
    .env("PULSE_RUNTIME_PATH", session.path.join("pulse"));
  session.start(Log::Pulse, pulse)?;
  wait_for("private Pulse socket", || {
    Ok(match socket.metadata() {
      Ok(_) => Ready::Yes,
      Err(_not_ready) => Ready::NotYet,
    })
  })?;
  let Ok(mut player) = session.command("paplay");
  player
    .env("PULSE_SERVER", format!("unix:{}", socket.display()))
    .args([
      "--raw",
      "--format=float32le",
      "--rate=48000",
      "--channels=2",
      "--device=test-output",
      "--stream-name=metered-pulse-player",
      "--volume=32768",
    ]);

  Ok(player)
}

fn exercise_gain_meter(session: &mut Session, client: MeterPlayer) -> io::Result<()> {
  let binary = env!("CARGO_BIN_EXE_console-sound-effect");
  prepare(session, Leveling::Enabled)?;

  let Ok(mut watcher) = session.command(binary);
  let log = session.path.join("gain-readout");
  let readout = fresh_stream(&log)?;
  watcher.arg("--watch-gain").stdout(readout);
  session.start(Log::Gain, watcher)?;

  let samples = session.path.join("gain-fixture.f32");
  let Ok(fixture) = fixture(&[LOUD, LOUD, LOUD, SOFT, SOFT, SOFT], Playback::DEFAULT);
  console_core_atomic_writes::whole(&samples, &fixture).map_err(io::Error::other)?;

  let mut player = match client {
    MeterPlayer::Native => {
      let Ok(mut player) = session.command("pw-cat");
      player.arg("--playback").args(AUDIO).args([
        "--target", "test-output", "--properties",
        "{ node.name = metered-player console.balance.meter-capable = true state.default-volume = 0.125 }",
        "-",
      ]);
      player
    }
    MeterPlayer::Pulse => pulse_meter(session)?,
  };
  let input = File::open(samples)?;
  player.stdin(input);
  session.start(Log::Player, player)?;
  wait_for("observed boost and attenuation", || {
    let report = match fs::read_to_string(&log) {
      Ok(text) => text,
      Err(_opening) => return Ok(Ready::NotYet),
    };

    let mut boost = false;
    let mut attenuation = false;
    let mut nodes = BTreeSet::new();

    for line in report.lines().filter(|line| line.starts_with("Node ")) {
      let mut columns = line.split_whitespace().skip(1);
      let _inserted = nodes.insert(columns.next());
      let reading = columns.next().map(str::parse::<f64>);

      match reading {
        Some(Ok(gain)) => {
          boost |= gain > 10.0;
          attenuation |= gain < -10.0;
        }
        Some(Err(_invalid_gain)) => {}
        None => {}
      }
    }

    Ok(
      match (boost && attenuation && nodes.len() == 2).then_some(()) {
        Some(()) => Ready::Yes,
        None => Ready::NotYet,
      },
    )
  })
}
