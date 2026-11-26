use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use console_core_external_programs::Program;

use super::{
  cargo, checked, child_exited, command, install, parent, prefixed, require_file, status,
  target_dir, temporary_dir,
};

const FIRMWARE: &str = "x86_64-unknown-uefi";
const ESP: &str = "target/kernel/esp";
const EFI: &str = "EFI/BOOT/BOOTX64.EFI";
const OVMF: &str = "/usr/share/edk2/x64/OVMF.4m.fd";

pub(super) fn run(name: &str, arguments: &[String]) -> io::Result<()> {
  match (name, arguments) {
    ("kernel-image", []) => kernel_image(),
    ("kernel", [cpu, memory]) => kernel(cpu, memory),
    ("kernel-input", []) => kernel_usb(Path::new(ESP), &[], Lockfile::Default),
    ("kernel-input", [mode]) => kernel_usb(Path::new(ESP), &[mode], Lockfile::Default),
    ("kernel-browser", []) => kernel_usb(Path::new(ESP), &["browser"], Lockfile::Pinned),
    ("native-build", [generation]) => native_build(Path::new(generation)),
    ("native-boot", [generation, previous]) => native_boot(generation, previous),
    ("native-check", [generation, expected]) => kernel_usb(
      &Path::new(generation).join("esp"),
      &[expected],
      Lockfile::Pinned,
    ),
    ("compute-worker", []) => compute_worker(),
    ("compute-benchmark", [profile]) => compute_benchmark(profile),
    ("video-presentation-benchmark", [clip]) => video_benchmark(clip),
    _invalid => Err(io::Error::other(format!("invalid arguments for {name}"))),
  }
}

fn kernel_image() -> io::Result<()> {
  let target = target_dir();
  checked(
    cargo(&[
      "build",
      "--quiet",
      "--release",
      "-p",
      "console-kernel",
      "--target",
      FIRMWARE,
      "--target-dir",
    ])
    .arg(&target),
  )?;
  install(
    &target.join(FIRMWARE).join("release/console-kernel.efi"),
    &Path::new(ESP).join(EFI),
  )
}

fn qemu_arguments<'a>(cpu: &'a str, memory: &'a str) -> [&'a str; 8] {
  ["-machine", "q35", "-cpu", cpu, "-m", memory, "-bios", OVMF]
}

fn kernel(cpu: &str, memory: &str) -> io::Result<()> {
  let Ok(qemu) = Program::QemuX86_64.name();
  let mut running = command(Program::Timeout)
    .args(["60", qemu])
    .args(qemu_arguments(cpu, memory))
    .args([
      "-drive",
      "if=none,id=preview,format=raw,file=fat:target/kernel/esp,readonly=on",
      "-device",
      "qemu-xhci",
      "-device",
      "usb-storage,drive=preview,removable=on,bootindex=1",
      "-nic",
      "none",
      "-display",
      "none",
      "-serial",
      "stdio",
      "-device",
      "isa-debug-exit,iobase=0xf4,iosize=0x04",
      "-no-reboot",
    ])
    .stdout(Stdio::piped())
    .spawn()?;
  let serial = running
    .stdout
    .take()
    .ok_or_else(|| io::Error::other("QEMU has no serial pipe"))?;
  let milestones = read_serial(BufReader::new(serial));
  let exit = running.wait()?;
  let milestones = milestones?;

  match (exit.code(), milestones) {
    (
      Some(1),
      Milestones {
        finished: true,
        displayed: true,
      },
    ) => Ok(()),
    (status, _) => Err(io::Error::other(format!(
      "kernel: QEMU exited {status:?}, and a complete transcript is required"
    ))),
  }
}

#[derive(Default)]
struct Milestones {
  finished: bool,
  displayed: bool,
}

fn read_serial(serial: impl BufRead) -> io::Result<Milestones> {
  let mut milestones = Milestones::default();

  for line in serial.split(b'\n') {
    let line = line?;
    let plain = strip_controls(&line);
    match plain.starts_with(b"console-kernel").then_some(()) {
      Some(()) => {}
      None => continue,
    }

    let mut stdout = io::stdout().lock();
    stdout.write_all(&plain)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;

    match plain.as_slice() {
      b"console-kernel: every step held" => milestones.finished = true,
      b"console-kernel: the browser page is on the panel" => milestones.displayed = true,
      _other => {}
    }
  }

  Ok(milestones)
}

fn strip_controls(line: &[u8]) -> Vec<u8> {
  let mut plain = Vec::with_capacity(line.len());
  let mut skip_until = 0;
  for (index, byte) in line.iter().copied().enumerate() {
    match (index < skip_until, byte) {
      (true, _) | (_, b'\r') => continue,
      (false, 0x1b) => match line.get(index..).and_then(control_end) {
        Some(length) => skip_until = index.saturating_add(length),
        None => plain.push(byte),
      },
      (false, text) => plain.push(text),
    }
  }

  plain
}

fn control_end(text: &[u8]) -> Option<usize> {
  let after_escape = text.strip_prefix(b"\x1b[")?;
  let (end, letter) = after_escape
    .iter()
    .enumerate()
    .find(|(_, byte)| !byte.is_ascii_digit() && **byte != b';')?;

  letter
    .is_ascii_alphabetic()
    .then_some(end.saturating_add(3))
}

enum Lockfile {
  Default,
  Pinned,
}

fn kernel_usb(esp: &Path, extras: &[&str], lockfile: Lockfile) -> io::Result<()> {
  let mut run = cargo(&["run"]);
  match lockfile {
    Lockfile::Default => {}
    Lockfile::Pinned => {
      run.arg("--locked");
    }
  }

  checked(
    run
      .args([
        "--quiet",
        "-p",
        "console-tasks",
        "--example",
        "kernel-usb",
        "--",
      ])
      .arg(esp)
      .args(extras)
      .env("TMPDIR", temporary_dir()),
  )
}

fn native_build(generation: &Path) -> io::Result<()> {
  fs::create_dir_all(parent(generation))?;
  checked(
    command(Program::Nix)
      .args([
        "build",
        "--impure",
        "--file",
        "nix/native-generation.nix",
        "--out-link",
      ])
      .arg(generation),
  )
}

fn native_boot(generation: &str, previous: &str) -> io::Result<()> {
  let mut run = command(Program::QemuX86_64);
  run.args(qemu_arguments("qemu64", "256M")).args([
    "-device",
    "qemu-xhci,id=xhci",
    "-device",
    "usb-kbd,bus=xhci.0",
  ]);
  add_drive(&mut run, generation, "current", 1)?;
  match previous {
    "" => {}
    previous => add_drive(&mut run, previous, "previous", 2)?,
  }
  run.args([
    "-boot",
    "menu=on",
    "-nic",
    "none",
    "-serial",
    "stdio",
    "-device",
    "isa-debug-exit,iobase=0xf4,iosize=0x04",
    "-no-reboot",
  ]);

  let exit = status(&mut run)?;
  match exit.code() {
    Some(0 | 65) => Ok(()),
    _failed => Err(child_exited("native-boot: QEMU".to_owned(), exit)),
  }
}

fn add_drive(run: &mut Command, generation: &str, id: &str, index: u8) -> io::Result<()> {
  let esp = fs::canonicalize(Path::new(generation).join("esp"))?;
  require_file(&esp.join(EFI))?;
  let mut escaped = Vec::new();
  for byte in esp.as_os_str().as_bytes() {
    escaped.push(*byte);
    match byte {
      b',' => escaped.push(b','),
      _other => {}
    }
  }

  let mut drive = prefixed(
    &format!("if=none,id={id},format=raw,file=fat:"),
    OsString::from_vec(escaped),
  );
  drive.push(",readonly=on");
  run.arg("-drive").arg(drive).arg("-device").arg(format!(
    "usb-storage,bus=xhci.0,drive={id},serial=console-{id},removable=on,bootindex={index}"
  ));

  Ok(())
}

fn compute_dir() -> PathBuf {
  target_dir().join("kernel-compute")
}

fn compute_worker() -> io::Result<()> {
  let build = compute_dir();
  fs::create_dir_all(&build)?;
  checked(
    command(Program::Cc)
      .args([
        "-std=c17",
        "-O2",
        "-Wall",
        "-Wextra",
        "-Werror",
        "crates/console-operating-system-linux/compute/vulkan.c",
        "-o",
      ])
      .arg(build.join("vulkan"))
      .arg("-lvulkan"),
  )
}

const COMPUTE_EXAMPLE: &[&str] = &[
  "-p",
  "console-kernel",
  "--features",
  "hosted-compute",
  "--example",
  "compute-benchmark",
];
const VIDEO_EXAMPLE: &[&str] = &[
  "-p",
  "console-application-media-viewer",
  "--example",
  "video-presentation",
];

fn compute_benchmark(profile: &str) -> io::Result<()> {
  let build = compute_dir();
  for shader in ["checksum-bench", "wrapping-sum", "mandelbrot"] {
    let mut emit = cargo(&["run", "--locked", "--quiet", "--release"]);
    emit
      .args(COMPUTE_EXAMPLE)
      .args(["--", "--emit-shader", shader]);
    compile_shader(&build, shader, &mut emit)?;
  }

  checked(
    cargo(&["run", "--locked", "--release"])
      .args(COMPUTE_EXAMPLE)
      .arg("--")
      .arg(build.join("vulkan"))
      .arg(build.join("checksum-bench.spv"))
      .arg(build.join("mandelbrot.spv"))
      .arg(profile),
  )
}

fn video_benchmark(clip: &str) -> io::Result<()> {
  let build = compute_dir();
  let mut emit = cargo(&["run", "--locked", "--quiet", "--release"]);
  emit.args(VIDEO_EXAMPLE).args(["--", "--emit-shader"]);
  compile_shader(&build, "video-colour", &mut emit)?;
  checked(
    cargo(&["run", "--locked", "--release"])
      .args(VIDEO_EXAMPLE)
      .arg("--")
      .arg(clip)
      .arg(build.join("vulkan"))
      .arg(build.join("video-colour.spv")),
  )
}

fn compile_shader(build: &Path, name: &str, emit: &mut Command) -> io::Result<()> {
  let source = build.join(format!("{name}.comp"));
  let compiled = build.join(format!("{name}.spv"));
  let shader = File::create(&source)?;
  checked(emit.stdout(shader))?;
  checked(
    command(Program::GlslangValidator)
      .args(["--target-env", "vulkan1.0", "-V"])
      .arg(source)
      .arg("-o")
      .arg(&compiled),
  )?;
  checked(
    command(Program::SpirvVal)
      .args(["--target-env", "vulkan1.0"])
      .arg(compiled),
  )
}
