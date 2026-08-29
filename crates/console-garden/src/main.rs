//! Draw the garden, from the command line.
//!
//!     console-garden          draw files/usr/share/backgrounds/console.webp
//!     console-garden --check  say whether it has fallen out of step, draw nothing
//!     console-garden --still  write a PNG of the resting picture somewhere to look

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_garden::garden::{self, Garden};
use console_garden::probe::{self, PROBES, Pixels};
use console_garden::{SEED, palette, scene, stamp, webp};
use console_screen::{CONFIG, Screen};

/// What the run was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Doing {
    /// Draw the wallpaper.
    Draw { still: Option<PathBuf> },
    /// Say whether it has fallen out of step, draw nothing.
    Check,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(fault) => {
            eprintln!("{fault}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let doing = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => Doing::Draw { still: None },
        [flag] if flag == "--check" => Doing::Check,
        [flag] if flag == "--help" || flag == "-h" => {
            println!("{HELP}");
            return Ok(ExitCode::SUCCESS);
        }
        [flag, path] if flag == "--still" => Doing::Draw {
            still: Some(path.into()),
        },
        other => {
            return Err(format!(
                "console-garden takes --check or --still PATH, not {other:?}"
            ));
        }
    };

    let root = repository()?;
    let read = |at: &str| {
        std::fs::read_to_string(root.join(at))
            .map_err(|fault| format!("{at} could not be read: {fault}"))
    };

    let spec: garden::Spec = toml::from_str(&read("theme/palette.toml")?)
        .map_err(|fault| format!("theme/palette.toml does not parse: {fault}"))?;
    let colours = palette::read(&read("theme/report.md")?)?;
    let screen = Screen::read(&read(CONFIG)?)?;
    let size = screen.pixels();
    let wanted = stamp::wanted(&colours, &spec.garden, size);

    let canvas = root.join("files/usr/share/backgrounds/console.webp");
    let stamped = root.join("theme/garden.stamp");
    if doing == Doing::Check {
        let held = std::fs::read_to_string(&stamped)
            .ok()
            .as_deref()
            .and_then(stamp::drawn_from);
        return match canvas.is_file() && held.as_deref() == Some(wanted.as_str()) {
            true => {
                println!("the garden is drawn from the palette as it stands.");
                Ok(ExitCode::SUCCESS)
            }
            false => {
                Err("the garden is out of step with the palette; run `make garden`".to_string())
            }
        };
    }

    let garden = Garden {
        width: f64::from(size.0),
        height: f64::from(size.1),
        paint: spec.garden.paints(&|name| colours.get(name).cloned())?,
        rest_seconds: spec.garden.rest_seconds,
        gust_seconds: spec.garden.gust_seconds,
        frames_per_second: spec.garden.frames_per_second,
    };

    let mut drawn = scene::draw(&garden, SEED, &webp::encode);
    let picture = webp::animation(size.0 as i32, size.1 as i32, &drawn.frames)?;
    let pixels = Pixels::of(&mut drawn.still);

    let probes: Vec<((f64, f64), String)> = PROBES
        .iter()
        .map(|where_| (*where_, probe::probe(&pixels, where_.0, where_.1, 0.02)))
        .collect();
    let nothing = colours.get("night").ok_or("the palette names no night")?;
    let dark = probe::blind(&probes, nothing);
    if !dark.is_empty() {
        dark.iter().for_each(|found| {
            println!(
                "  the probe at {}, {} reads #{}, which is {} from what an unpainted screen reads",
                found.across, found.down, found.colour, found.apart
            )
        });
        return Err(
            "a probe cannot tell the picture from a bare screen; move it somewhere the picture has a colour"
                .to_string(),
        );
    }

    if let Some(holding) = canvas.parent() {
        std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{} could not be made: {fault}", holding.display()))?;
    }
    std::fs::write(&canvas, &picture)
        .map_err(|fault| format!("{} could not be written: {fault}", canvas.display()))?;
    std::fs::write(
        &stamped,
        stamp::written(&wanted, &probe::commonest(&pixels), size, &probes),
    )
    .map_err(|fault| format!("{} could not be written: {fault}", stamped.display()))?;
    if let Doing::Draw { still: Some(path) } = &doing {
        let mut out = std::fs::File::create(path)
            .map_err(|fault| format!("{} could not be written: {fault}", path.display()))?;
        drawn
            .still
            .write_to_png(&mut out)
            .map_err(|fault| format!("the picture would not write: {fault}"))?;
    }

    let rest = spec.garden.rest_seconds as u32;
    println!(
        "the garden: {}x{}, still for {}m{:02}s, then {} frames of wind.",
        size.0,
        size.1,
        rest / 60,
        rest % 60,
        drawn.count
    );
    println!(
        "  the wind crosses a band {}x{} at {}, and only that is redrawn.",
        size.0, drawn.tall, drawn.top
    );
    println!(
        "  {} is {:.0} KiB.",
        canvas.strip_prefix(&root).unwrap_or(&canvas).display(),
        picture.len() as f64 / 1024.0
    );
    Ok(ExitCode::SUCCESS)
}

const HELP: &str = "\
console-garden               draw files/usr/share/backgrounds/console.webp
console-garden --check       say whether it has fallen out of step, draw nothing
console-garden --still PATH  also write the resting picture as a PNG, to look at";

/// The repository this is being run inside.
///
/// Found by walking up from wherever it was started rather than from the
/// binary's own path, because a compiled program can be installed anywhere and
/// the tree it writes into is the one somebody is standing in.
fn repository() -> Result<PathBuf, String> {
    let here = std::env::current_dir().map_err(|fault| format!("no working directory: {fault}"))?;
    here.ancestors()
        .find(|at| at.join("theme/palette.toml").is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "no theme/palette.toml above {}; run this inside the repository",
                here.display()
            )
        })
}
