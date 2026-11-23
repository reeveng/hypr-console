//! The Legion Go's desktop, running on this machine, in a window.
//!
//! ```text
//! console-desktop verify     does the compositor config still parse
//! console-desktop run        the desktop, nested, at the device's size
//! console-desktop shot FILE  a picture of it, once the screen has stopped
//!                            changing, or --settle N seconds after
//! console-desktop shot FILE --bare   the same, without a ground to paint
//! console-desktop shot FILE --until AT   and not before AT has a line in it
//! console-desktop shot FILE --press S   once the screen is still, run S
//!                            inside, and do not go on until it has finished
//! console-desktop pressing AT --then C  once AT has been drawn to by a panel
//!                            holding the keyboard, press C, and not return
//!                            until it has been drawn to again
//! console-desktop shot FILE --clients AT what windows it had, written to AT
//! console-desktop shot FILE --monitors AT what screen it was, written to AT
//! console-desktop describe --open C --until AT [--press S]
//!                            C alone and headless, until it has described
//!                            itself to AT and S has been drawn; no picture
//! console-desktop probe      what the nested compositor thinks
//! console-desktop stage      the staged copy, and nothing else
//! console-desktop clean      forget what no one is using
//! ```
//!
//! The compositor is started in a control group of its own, named after this
//! process, because killing it reaches nothing it started. `session-start`
//! backgrounds a pool, a bar and a keyboard; when the compositor
//! goes they are reparented to the user manager and land in the control group
//! of whoever is logged in, where nothing can tell them from the desktop
//! someone is using. An afternoon of runs left eight hundred of them on this
//! laptop, sixty `pactl subscribe` among them -- four short of the number
//! pipewire-pulse serves before it starts refusing, which is the volume keys
//! going quiet for a reason no one would find. Stopping the scope takes all of
//! it without naming one process, which matters because the names are the
//! session's own.
//!
//! `just alone` wraps a whole run the same way and this is not that. A scope
//! made inside a scope is its sibling rather than its child, so the run's scope
//! never reaches this one -- and it is this one that has to hold, because the
//! panel tier is run as often by `cargo test -p console-panel` as by `just`.
//! What a scope cannot answer is a run killed outright, which leaves its own
//! standing; `session::swept` ends the scope of every stage whose pid is gone,
//! by the same arithmetic that deletes the stage.
//!
//! The wallpaper is started directly and held until it has exited, before
//! the compositor is stopped. Letting the session script start it meant a
//! short panel check could finish first and the daemon would arrive during
//! teardown. Panel descriptions run alone without the desktop services.

use console_core_external_programs::Program;
use console_core_finite_numbers::FiniteF64;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_operating_system_linux::systemd::{Wrapped, in_a_scope_of_its_own, nothing_left_in};
use console_waiting::{Ready, Schedule, until_handed};
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::process::{Child, ChildStderr, Command, ExitCode, Stdio};
use std::time::Duration;

use console_core_arguments::{
  Flag, Operands, Presence, Subcommand, Takes, ValidationError, read_with,
};
use console_core_words::Words;
use console_test_desktop::connection::{Inside, Instance, Outcome};
use console_test_desktop::nested::Wallpaper;
use console_test_desktop::staging::{Screen, Services, Verbosity, environment, stage_desktop};
use console_test_desktop::{Unnested, scope_of, screen, session, stage};
use console_test_stages::picture::{Picture, where_};

const KILLED_BY_A_SIGNAL: i32 = -1;

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "what someone typed, one field per word: a window and a file and a number of seconds are asked for together or not at all, and the combinations are the command lines rather than states"
  )
)]
struct Arguments {
  command: Verb,
  file: Option<PathBuf>,
  seconds: Option<f64>,
  settle: Option<f64>,
  until: Option<PathBuf>,
  clients: Option<PathBuf>,
  monitors: Option<PathBuf>,
  open: Vec<String>,
  press: Option<String>,
  then: Option<String>,
  press_count: NonZeroU64,
  sample: Vec<String>,
  window: bool,
  bare: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
enum Verb {
  #[words(word = "run", about = "the desktop, nested, at the device's size")]
  Run,
  #[words(
    word = "shot",
    about = "a picture of it at FILE, once the screen has stopped changing"
  )]
  Shot,
  #[words(
    word = "pressing",
    about = "once FILE has been drawn to by a panel holding the keyboard, press --then, and wait for it to be drawn to again"
  )]
  Pressing,
  #[words(
    word = "describe",
    about = "--open alone and headless, until it has described itself to --until; no picture"
  )]
  Describe,
  #[words(word = "verify", about = "does the compositor config still parse")]
  Verify,
  #[words(word = "probe", about = "what the nested compositor thinks")]
  Probe,
  #[words(word = "stage", about = "the staged copy, and nothing else")]
  Stage,
  #[words(word = "clean", about = "forget what no one is using")]
  Clean,
}

impl Subcommand for Verb {
  fn variants() -> Result<impl Iterator<Item = Self>, Never> {
    Ok(Verb::VARIANTS.iter().copied())
  }

  fn spelling(self) -> Result<&'static str, Never> {
    self.word()
  }

  fn about(self) -> Result<&'static str, Never> {
    Verb::about(self)
  }
}

const SECONDS: Flag = Flag {
  spelling: "--seconds",
  takes: Takes::Value("SECONDS"),
  about: "stop the desktop after that long",
};

const SETTLE: Flag = Flag {
  spelling: "--settle",
  takes: Takes::Value("SECONDS"),
  about: "take the picture that long after, rather than once the screen is still",
};

const UNTIL: Flag = Flag {
  spelling: "--until",
  takes: Takes::Value("AT"),
  about: "not before AT has a line in it",
};

const CLIENTS: Flag = Flag {
  spelling: "--clients",
  takes: Takes::Value("AT"),
  about: "write what windows it had to AT",
};

const MONITORS: Flag = Flag {
  spelling: "--monitors",
  takes: Takes::Value("AT"),
  about: "write what screen it was to AT",
};

const OPEN: Flag = Flag {
  spelling: "--open",
  takes: Takes::Value("COMMAND"),
  about: "run COMMAND inside, once for every time it is said",
};

const PRESS: Flag = Flag {
  spelling: "--press",
  takes: Takes::Value("SCRIPT"),
  about: "once the screen is still, run SCRIPT inside and wait for it to finish",
};

const THEN: Flag = Flag {
  spelling: "--then",
  takes: Takes::Value("COMMAND"),
  about: "what pressing presses",
};

const PRESS_COUNT: Flag = Flag {
  spelling: "--press-count",
  takes: Takes::Value("COUNT"),
  about: "how many inputs pressing waits for (default: 1)",
};

const SAMPLE: Flag = Flag {
  spelling: "--sample",
  takes: Takes::Value("PLACE"),
  about: "say the colour at PLACE in the picture, once for every time it is said",
};

const WINDOW: Flag = Flag {
  spelling: "--window",
  takes: Takes::None,
  about: "in a window here rather than headless",
};

const BARE: Flag = Flag {
  spelling: "--bare",
  takes: Takes::None,
  about: "without a ground to paint",
};

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "console-desktop",
  about: "the device's desktop, nested on this machine",
  flags: &[
    SECONDS, SETTLE, UNTIL, CLIENTS, MONITORS, OPEN, PRESS, THEN, PRESS_COUNT, SAMPLE, WINDOW, BARE,
  ],
  operands: Operands::Optional("FILE"),
};

fn asked(words: &[String]) -> Result<Arguments, ValidationError> {
  let line = read_with::<Verb, String>(&COMMAND, words)?;
  let Ok(doing) = line.subcommand();
  let Ok(operands) = line.operands();
  let seconds = line.parsed::<f64>(SECONDS)?;
  let settle = line.parsed::<f64>(SETTLE)?;
  let path = |flag: Flag| {
    let Ok(said) = line.value(flag);

    said.map(PathBuf::from)
  };
  let word = |flag: Flag| {
    let Ok(said) = line.value(flag);

    said.map(str::to_string)
  };
  let Ok(open) = line.values(OPEN);
  let Ok(sample) = line.values(SAMPLE);
  let Ok(window) = line.presence(WINDOW);
  let Ok(bare) = line.presence(BARE);
  let command = match doing {
    Some(doing_2) => doing_2,
    None => Verb::Run,
  };

  Ok(Arguments {
    command,
    file: operands.first().map(PathBuf::from),
    seconds,
    settle,
    until: path(UNTIL),
    clients: path(CLIENTS),
    monitors: path(MONITORS),
    open: open.to_vec(),
    press: word(PRESS),
    then: word(THEN),
    press_count: line.parsed::<NonZeroU64>(PRESS_COUNT)?.unwrap_or(NonZeroU64::MIN),
    sample: sample.to_vec(),
    window: window == Presence::Present,
    bare: bare == Presence::Present,
  })
}

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();
  let asked = match asked(&words) {
    Ok(asked) => asked,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };

  let done = match asked.command {
    Verb::Clean => clean(),
    Verb::Stage => stage_desktop(
      Verbosity::Aloud,
      Screen::InAWindow,
      Wallpaper::Started,
      Services::Desktop,
    )
    .map(|_| 0),
    Verb::Verify => verify(),
    Verb::Probe => run(&asked, None, Action::Probing),
    Verb::Describe => run(&asked, None, Action::Describing),
    Verb::Pressing => press_file(&asked),
    Verb::Shot => match asked.file.clone() {
      Some(file) => run(&asked, Some(file), Action::Running),
      None => Err(Unnested::NowhereToWrite),
    },
    Verb::Run => run(&asked, None, Action::Running),
  };

  let finished = match asked.command {
    Verb::Stage | Verb::Pressing => done,
    Verb::Run | Verb::Shot | Verb::Describe | Verb::Verify | Verb::Probe | Verb::Clean => {
      match stage() {
        Ok(here) => match session::remove_tree(&here) {
          Ok(()) => done,
          Err(fault) => {
            eprintln!("console-desktop: clearing {}: {fault}", here.display());

            done.and(Ok(1))
          }
        },
        Err(fault) => Err(fault),
      }
    }
  };

  match finished {
    Ok(code) => ExitCode::from(code),
    Err(why) => {
      eprintln!("{why}");
      ExitCode::from(1)
    }
  }
}

fn press_file(asked: &Arguments) -> Result<u8, Unnested> {
  let at = match &asked.file {
    Some(at) => at,
    None => return Err(Unnested::NothingToPress),
  };

  let command = match &asked.then {
    Some(command) => command,
    None => return Err(Unnested::NothingToPress),
  };

  let Ok(drew) = session::wait_for_written(at, session::A_LINE);

  match drew {
    session::Wrote::Some => {}
    session::Wrote::None => eprintln!(
      "console-desktop: nothing had drawn to {} in {}s, and {command} is pressed anyway",
      at.display(),
      session::A_LINE.as_secs()
    ),
  }

  let Ok(listening) = session::wait_for_the_keyboard(at, session::A_LINE);

  match listening {
    session::Wrote::Some => {}
    session::Wrote::None => eprintln!(
      "console-desktop: whoever writes {} never held the keyboard in {}s, and {command} is pressed anyway",
      at.display(),
      session::A_LINE.as_secs()
    ),
  }

  let Ok(before) = session::presses(at);

  let Ok(mut asking) = Program::Sh.command();

  #[cfg_attr(
    dylint_lib = "explicit082_no_built_shell_line",
    expect(
      explicit082_no_built_shell_line,
      reason = "the test driver accepts a shell program explicitly from its command-line input and executes that program as supplied"
    )
  )]
  let pressed = asking
    .args(["-c", command])
    .status()
    .map_err(Unnested::Machine)?;

  match ::core::primitive::bool::then_some(pressed.success(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => eprintln!("console-desktop: {command}: {pressed}"),
  }

  let last = before.saturating_add(asked.press_count.get().saturating_sub(1));
  let Ok(again) = session::wait_for_presses_past(at, last, session::A_LINE);

  Ok(match again {
    session::Wrote::Some => 0,
    session::Wrote::None => {
      eprintln!(
        "console-desktop: {command} was not heard by whoever writes {} in {}s",
        at.display(),
        session::A_LINE.as_secs()
      );

      1
    }
  })
}

fn out_of_the_way() -> Result<(), Never> {
  let Ok(instance) = console_compositor::instance();

  match instance {
    None => return Ok(()),
    Some(_a_compositor_to_stand_out_of_the_way_of) => {}
  }

  let Ok(_said) = console_compositor::request(
    console_compositor::Request::Script,
    r#"hl.window_rule({ name = "the nested desktop stays out of the way", match = { class = "aquamarine" }, workspace = "special:console-desktop silent" })"#,
  );

  Ok(())
}

fn clean() -> Result<u8, Unnested> {
  let here = stage()?;
  session::remove_tree(&here).map_err(|fault| Unnested::Staging("clearing the stage", fault))?;
  let Ok(()) = session::swept();
  let Ok(dead) = session::dead_instances();

  for path in dead {
    session::remove_tree(&path)
      .map_err(|fault| Unnested::Staging("clearing a dead instance", fault))?;
  }

  Ok(0)
}

fn verify() -> Result<u8, Unnested> {
  let nested = stage_desktop(
    Verbosity::Quietly,
    Screen::InAWindow,
    Wallpaper::Started,
    Services::Desktop,
  )?;
  let Ok(mut asking) = Program::Hyprland.command();

  asking.args(["--verify-config", "-c"]).arg(&nested);

  let environment = environment()?;

  for (name, value) in environment {
    asking.env(name, value);
  }

  let done = asking.output().map_err(Unnested::Machine)?;
  let Ok(said) = process_text(&done.stdout, "Hyprland stdout");
  let Ok(complained) = process_text(&done.stderr, "Hyprland stderr");
  println!(
    "{}",
    match said.as_str() {
      "" => complained.as_str(),
      stdout => stdout,
    }
  );
  Ok(u8::from(!done.status.success()))
}

fn process_text(bytes: &[u8], stream: &str) -> Result<String, Never> {
  Ok(match std::str::from_utf8(bytes) {
    Ok(text) => text.trim().to_string(),
    Err(fault) => format!("{stream} is not UTF-8: {fault}"),
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
  Probing,
  Running,
  Describing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
  ByThis,
  ByTheCompositor,
}

fn start_nested(
  config: &std::path::Path,
  environment: &[(String, String)],
  scope: Option<&str>,
) -> Result<(Child, session::Came), Unnested> {
  #[cfg_attr(
    dylint_lib = "explicit082_no_built_shell_line",
    expect(
      explicit082_no_built_shell_line,
      reason = "Hyprland uses -c for a configuration file name"
    )
  )]
  let Ok(arguments) =
    Program::Hyprland.words(vec!["-c".to_string(), config.display().to_string()]);
  let Ok((wrapped, scoped_arguments)) = in_a_scope_of_its_own(scope, &arguments);

  match wrapped {
    Wrapped::InAScope => {}
    Wrapped::AsItWasHandedIn => eprintln!(
      "console-desktop: no user manager here, so what this session starts \
             outlives it"
    ),
  }

  let (program, rest) = match scoped_arguments.split_first() {
    Some(said) => said,
    None => return Err(Unnested::NoCompositor),
  };

  let (mut compositor, instance) = {
    let Ok(_held) = session::Starting::now();
    let Ok(running) = session::instances();
    let mut asking = Command::new(program);

    asking.args(rest).env_remove("HYPRLAND_INSTANCE_SIGNATURE");

    for (name, value) in environment {
      asking.env(name, value);
    }

    let started = asking.spawn().map_err(Unnested::Machine)?;
    let Ok(came) = session::wait_for_one(&running);

    (started, came)
  };

  match instance {
    Some(came) => Ok((compositor, came)),
    None => {
      let Ok(()) = kill_and_reap(&mut compositor, "the compositor");
      let Ok(()) = nothing_left(scope);

      Err(Unnested::NoCompositor)
    }
  }
}

fn run(asked: &Arguments, shot: Option<PathBuf>, probe: Action) -> Result<u8, Unnested> {
  let headless = !asked.window;
  let showing = match (headless, &shot, probe) {
    (true, Some(_), Action::Probing | Action::Running | Action::Describing)
    | (true, None, Action::Describing) => Screen::Headless,
    (true, None, Action::Probing | Action::Running)
    | (false, Some(_) | None, Action::Probing | Action::Running | Action::Describing) => {
      Screen::InAWindow
    }
  };
  let ended = match ::core::primitive::bool::then_some(
    shot.is_none() && asked.seconds.is_none() && probe == Action::Running,
    (),
  ) {
    ::core::option::Option::Some(()) => Ended::ByTheCompositor,
    ::core::option::Option::None => Ended::ByThis,
  };
  let Ok(wallpaper) = wallpaper_for(ended, probe);
  let services = match probe {
    Action::Describing => Services::Panel,
    Action::Running | Action::Probing => Services::Desktop,
  };
  let nested = stage_desktop(Verbosity::Quietly, showing, wallpaper, services)?;
  let where_ = environment()?;
  let Ok(()) = out_of_the_way();
  let here = stage()?;
  let Ok(named) = scope_of(&here);
  let go = screen()?;
  let (
    mut compositor,
    session::Came {
      display: socket,
      signature,
    },
  ) = start_nested(&nested, &where_, named.as_deref())?;

  eprintln!("the desktop is on {socket}");

  match ended {
    Ended::ByTheCompositor => {
      let done = compositor.wait().map_err(Unnested::Machine);
      let Ok(()) = session::left_behind(&signature);
      let Ok(()) = nothing_left(named.as_deref());

      return done.map(|exit| u8::from(!exit.success()));
    }
    Ended::ByThis => {}
  }

  let Ok(inside) = Inside::new(
    where_,
    Instance {
      socket: &socket,
      signature: &signature,
    },
  );

  let Ok(looked_at) = looked_at(asked);

  let Ok(made) = match ::core::primitive::bool::then_some(headless, ()) {
    ::core::option::Option::Some(()) => inside.make_the_screen(&go),
    ::core::option::Option::None => Ok(Outcome::Happened),
  };

  match made {
    Outcome::Happened => {}
    Outcome::RanOut => {
      let Ok(()) = stop(&mut compositor, &signature, &inside, named.as_deref());

      return Err(Unnested::NoScreen);
    }
  }

  let Ok(waited) = inside.wait_for_screen(looked_at);

  match waited {
    Outcome::RanOut => {
      let Ok(()) = stop(&mut compositor, &signature, &inside, named.as_deref());

      return Err(Unnested::NoScreen);
    }
    Outcome::Happened => {}
  }

  let daemon = match wallpaper {
    Wallpaper::Started => {
      let Ok(daemon) = inside.start_the_wallpaper(&here.join("usr/local/lib/console/palette.sh"));

      daemon
    }
    Wallpaper::LeftOut => None,
  };

  match (asked.bare, wallpaper) {
    (false, Wallpaper::Started) => {
      let Ok(()) = inside.paint_the_background(&here.join("usr/share/backgrounds/console.webp"));
    }
    (true, Wallpaper::Started | Wallpaper::LeftOut) | (false, Wallpaper::LeftOut) => {}
  }

  let Ok(opened) = open_requested(asked, &inside, probe);
  let Ok(probe_status) = run_probe(asked, &inside, probe);
  let Ok(monitors) = record_clients(asked, &inside);

  match &shot {
    Some(file) => match screenshot(asked, &inside, file, &monitors, &go) {
      Ok(()) => {}
      Err(fault) => {
        let Ok(()) = inside.stop_the_wallpaper(daemon);
        let Ok(()) = stop(&mut compositor, &signature, &inside, named.as_deref());
        let Ok(()) = nothing_left_running(opened);

        return Err(fault);
      }
    },
    None => {}
  }

  match asked.seconds {
    Some(seconds) => {
      #[cfg_attr(
        dylint_lib = "explicit021_no_sleeping",
        allow(
          explicit021_no_sleeping,
          reason = "`--seconds N` is how long someone asked to be left looking at the desktop; nothing else ends this wait, because nothing else is meant to"
        )
      )]
      std::thread::sleep(Duration::from_secs_f64(seconds));
    }
    None => {}
  }

  let Ok(()) = inside.stop_the_wallpaper(daemon);
  let Ok(()) = stop(&mut compositor, &signature, &inside, named.as_deref());
  let Ok(()) = nothing_left_running(opened);

  Ok(probe_status)
}

fn wallpaper_for(ended: Ended, action: Action) -> Result<Wallpaper, Never> {
  Ok(match (ended, action) {
    (Ended::ByThis, Action::Running | Action::Probing) => Wallpaper::Started,
    (Ended::ByThis, Action::Describing)
    | (Ended::ByTheCompositor, Action::Running | Action::Probing | Action::Describing) => {
      Wallpaper::LeftOut
    }
  })
}

fn open_requested(
  asked: &Arguments,
  inside: &Inside,
  action: Action,
) -> Result<Vec<(String, Child)>, Never> {
  let Ok(was) = inside.surfaces();

  #[cfg_attr(
    dylint_lib = "explicit029_no_asking_per_item",
    allow(
      explicit029_no_asking_per_item,
      reason = "each is a different program the check asked to be opened, so there was never one process that could have opened all of them"
    )
  )]
  let mut opened: Vec<(String, Child)> = asked
    .open
    .iter()
    .filter_map(|command| {
      let Ok(mut asking) = inside.command("sh");

      #[cfg_attr(
        dylint_lib = "explicit082_no_built_shell_line",
        expect(
          explicit082_no_built_shell_line,
          reason = "each --open argument explicitly contains a shell program for the test driver"
        )
      )]
      let started = match asking
        .args(["-c", command])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
      {
        Ok(started) => started,
        Err(fault) => {
          eprintln!("console-desktop: {command}: {fault}");

          return None;
        }
      };

      Some((command.clone(), started))
    })
    .collect();

  match (opened.is_empty(), action) {
    (true, _) | (false, Action::Describing) => {}
    (false, Action::Running | Action::Probing) => {
      let Ok(waited) = inside.wait_for_something(&was);

      match waited {
        Outcome::RanOut => eprintln!("nothing that was asked for reached the screen"),
        Outcome::Happened => {}
      }

      let Ok(()) = inside.show_a_window();
      let Ok(()) = say_what_died(&mut opened);
    }
  }

  Ok(opened)
}

fn run_probe(asked: &Arguments, inside: &Inside, probe: Action) -> Result<u8, Never> {
  match probe {
    Action::Probing => {
      for question in ["monitors", "workspaces", "clients"] {
        println!("== {question}");
        let Ok(said) = inside.hyprctl(&[question, "-j"]);

        println!("{}", said.chars().take(1200).collect::<String>());
      }
    }
    Action::Running | Action::Describing => {}
  }

  match &asked.until {
    Some(at) => {
      let Ok(wrote) = session::wait_for_written(at, session::A_LINE);

      match wrote {
        session::Wrote::Some => {}
        session::Wrote::None => {
          eprintln!(
            "console-desktop: nothing wrote a line to {} in {}s",
            at.display(),
            session::A_LINE.as_secs()
          );
        }
      }
    }
    None => {}
  }

  let mut status = 0;
  match &asked.press {
    Some(script) => {
      match probe {
        Action::Running | Action::Probing => {
          let Ok(_settled) = inside.wait_for_a_still_screen();
        }
        Action::Describing => {
          let Ok(()) = inside.focus_a_window();
        }
      }

      let Ok(mut asking) = inside.command("sh");

      #[cfg_attr(
        dylint_lib = "explicit082_no_built_shell_line",
        expect(
          explicit082_no_built_shell_line,
          reason = "the --press argument explicitly contains a shell program for the test driver"
        )
      )]
      match asking.args(["-c", script]).status() {
        Ok(done) => match ::core::primitive::bool::then_some(done.success(), ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => {
            eprintln!("console-desktop: the presses ended {done}");
            status = 1;
          }
        },
        Err(fault) => {
          eprintln!("console-desktop: {script}: {fault}");
          status = 1;
        }
      }
    }
    None => {}
  }

  match (asked.settle, probe) {
    (Some(seconds), Action::Probing | Action::Running | Action::Describing) => {
      #[cfg_attr(
        dylint_lib = "explicit021_no_sleeping",
        allow(
          explicit021_no_sleeping,
          reason = "`--settle N` is a person asking for N seconds; the elapsing is the whole of what was asked for, and the default below is the poll it overrides"
        )
      )]
      std::thread::sleep(Duration::from_secs_f64(seconds));
    }
    (None, Action::Describing) => {}
    (None, Action::Probing | Action::Running) => {
      let Ok(_still) = inside.wait_for_a_still_screen();
    }
  }

  Ok(match probe {
    Action::Describing => status,
    Action::Probing | Action::Running => 0,
  })
}

fn record_clients(asked: &Arguments, inside: &Inside) -> Result<String, Never> {
  match &asked.clients {
    Some(at) => {
      let Ok(said) = inside.hyprctl(&["clients", "-j"]);

      match console_core_atomic_writes::whole(at, said.as_bytes()) {
        Ok(()) => {}
        Err(fault) => eprintln!("console-desktop: {}: {fault}", at.display()),
      }
    }
    None => {}
  }

  let Ok(monitors) = inside.hyprctl(&["monitors", "-j"]);

  match &asked.monitors {
    Some(at) => match console_core_atomic_writes::whole(at, monitors.as_bytes()) {
      Ok(()) => {}
      Err(fault) => eprintln!("console-desktop: {}: {fault}", at.display()),
    },
    None => {}
  }

  Ok(monitors)
}

fn looked_at(asked: &Arguments) -> Result<&'static str, Never> {
  Ok(match ::core::primitive::bool::then_some(asked.window, ()) {
    ::core::option::Option::Some(()) => "WAYLAND-1",
    ::core::option::Option::None => "HEADLESS-1",
  })
}

fn screenshot(
  asked: &Arguments,
  inside: &Inside,
  file: &std::path::Path,
  monitors: &str,
  go: &console_screen::Screen,
) -> Result<(), Unnested> {
  let Ok(looked_at) = looked_at(asked);
  let Ok(mut asking) = inside.command("grim");

  let taken = asking
    .args(["-o", looked_at])
    .arg(file)
    .output()
    .map_err(Unnested::Machine)?;

  match ::core::primitive::bool::then_some(taken.status.success(), ()) {
    ::core::option::Option::Some(()) => {
      println!("{}", file.display());
      let Ok(()) = say_the_colors(file, &asked.sample, monitors, go);
    }
    ::core::option::Option::None => {
      let Ok(detail) = process_text(&taken.stderr, "grim stderr");

      eprintln!("no picture: {detail}");
    }
  }

  Ok(())
}

const GOING: Duration = Duration::from_secs(3);

fn asked_to_stop(process: &Child) -> Result<(), Never> {
  let Ok(which) = fitted(process.id());

  console_operating_system_linux::program_lifetime::signal(which, rustix::process::Signal::TERM)
}

fn kill_and_reap(process: &mut Child, what: &str) -> Result<(), Never> {
  match process.kill() {
    Ok(()) => match process.wait() {
      Ok(_) => {}
      Err(fault) => eprintln!("console-desktop: waiting for {what}: {fault}"),
    },
    Err(fault) => match process.try_wait() {
      Ok(Some(_exited)) => return Ok(()),
      Ok(None) => eprintln!("console-desktop: could not kill {what}: {fault}"),
      Err(wait_fault) => {
        eprintln!("console-desktop: could not kill {what}: {fault}. Checking exit: {wait_fault}");
      }
    },
  }

  Ok(())
}

fn nothing_left_running(opened: Vec<(String, Child)>) -> Result<(), Never> {
  let mut going: Vec<(String, Child)> = Vec::new();

  for (command, mut process) in opened {
    match process.try_wait() {
      Ok(Some(_)) => {}
      Ok(None) => {
        let Ok(()) = asked_to_stop(&process);

        going.push((command, process));
      }
      Err(_unasked) => {
        let Ok(()) = asked_to_stop(&process);

        going.push((command, process));
      }
    }
  }

  let Ok(patience) = Schedule::asking_every(GOING, Duration::from_millis(100));
  let Ok(all_gone) = until_handed(patience, &mut going, |going_2| {
    going_2.retain_mut(|(_, process)| !matches!(process.try_wait(), Ok(Some(_))));

    Ok(
      match ::core::primitive::bool::then_some(going_2.is_empty(), ()) {
        ::core::option::Option::Some(()) => Ready::Yes,
        ::core::option::Option::None => Ready::NotYet,
      },
    )
  });

  match all_gone {
    Outcome::Happened => return Ok(()),
    Outcome::RanOut => {}
  }

  for (command, mut process) in going {
    eprintln!("console-desktop: {command} did not stop after SIGTERM");
    let Ok(()) = kill_and_reap(&mut process, &command);
  }

  Ok(())
}

fn nothing_left(named: Option<&str>) -> Result<(), Never> {
  match named {
    Some(unit) => nothing_left_in(unit),
    None => Ok(()),
  }
}

fn stop(
  compositor: &mut Child,
  signature: &str,
  inside: &Inside,
  named: Option<&str>,
) -> Result<(), Never> {
  let Ok(()) = inside.stop_the_bar();
  let Ok(which) = fitted(compositor.id());
  let Ok(()) = console_operating_system_linux::program_lifetime::signal(
    which,
    rustix::process::Signal::TERM,
  );

  let Ok(patience) = Schedule::asking_every(Duration::from_secs(10), Duration::from_millis(100));
  let Ok(ended) = until_handed(patience, compositor, |compositor_2| {
    Ok(
      match ::core::primitive::bool::then_some(
        compositor_2.try_wait().is_ok_and(|ended| ended.is_some()),
        (),
      ) {
        ::core::option::Option::Some(()) => Ready::Yes,
        ::core::option::Option::None => Ready::NotYet,
      },
    )
  });

  match ended {
    Outcome::Happened => {
      let Ok(()) = session::left_behind(signature);
      let Ok(()) = nothing_left(named);

      return Ok(());
    }
    Outcome::RanOut => {}
  }

  let Ok(()) = kill_and_reap(compositor, "the compositor");
  let Ok(()) = session::left_behind(signature);
  let Ok(()) = nothing_left(named);

  Ok(())
}

fn say_what_died(opened: &mut [(String, Child)]) -> Result<(), Never> {
  for (command, process) in opened {
    let ended = match process.try_wait() {
      Ok(Some(ended)) => ended,
      Ok(None) => continue,
      Err(_unasked) => continue,
    };

    let Ok(said) = match process.stderr.take() {
      Some(voice) => stderr_excerpt(voice),
      None => Ok(String::new()),
    };

    eprintln!(
      "{command} ended with {}: {}",
      match ended.code() {
        Some(code) => code,
        None => KILLED_BY_A_SIGNAL,
      },
      match said.as_str() {
        "" => "nothing said",
        detail => detail,
      }
    );
  }

  Ok(())
}

fn stderr_excerpt(voice: ChildStderr) -> Result<String, Never> {
  use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
  use rustix::io::Errno;

  let nonblocking =
    fcntl_getfl(&voice).and_then(|flags| fcntl_setfl(&voice, flags | OFlags::NONBLOCK));

  match nonblocking {
    Ok(()) => {}
    Err(fault) => return Ok(format!("could not set stderr nonblocking: {fault}")),
  }

  let mut excerpt = [0u8; 65_536];
  let ::core::result::Result::Ok(length) =
    ::console_core_number_conversion::fixed_index(match rustix::io::read(&voice, &mut excerpt) {
      Ok(length) => length,
      Err(Errno::AGAIN | Errno::INTR) => return Ok(String::new()),
      Err(fault) => return Ok(format!("could not read stderr: {fault}")),
    });

  match excerpt.get(
    ..match ::console_core_number_conversion::index(length) {
      ::core::result::Result::Ok(native_position) => native_position,
    },
  ) {
    Some(bytes) => process_text(bytes, "child stderr"),
    None => Ok(String::from("invalid stderr length")),
  }
}

fn live(monitors: &str, go: &console_screen::Screen) -> Result<Size<u32>, Never> {
  let Ok(declared) = go.logical();

  let monitors_3 = match console_compositor::read(console_compositor::Monitors, monitors) {
    Ok(monitors_2) => monitors_2,
    Err(_nothing_to_read) => return Ok(declared),
  };

  let asked = monitors_3.first().and_then(|first| {
    let Ok(logical) = first.logical();

    logical
  });

  Ok(match asked {
    Some(asked_2) => asked_2,
    None => declared,
  })
}

fn say_the_colors(
  shot: &std::path::Path,
  sample: &[String],
  monitors: &str,
  go: &console_screen::Screen,
) -> Result<(), Never> {
  match ::core::primitive::bool::then_some(sample.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok(()),
    ::core::option::Option::None => {}
  }

  let Ok(logical) = live(monitors, go);

  let picture = match Picture::read(shot) {
    Ok(picture) => picture,
    Err(_unreadable) => return Ok(()),
  };

  for place in sample {
    match ::core::primitive::bool::then_some(place == "most", ()) {
      ::core::option::Option::Some(()) => {
        let Ok(most) = picture.most_common();

        println!("  most of it   #{most}");
        continue;
      }
      ::core::option::Option::None => {}
    }

    let (written_across, written_down) = match place.split_once(',') {
      Some(both) => both,
      None => continue,
    };

    let (across, down) = match (
      written_across.trim().parse::<FiniteF64>(),
      written_down.trim().parse::<FiniteF64>(),
    ) {
      (Ok(read_across), Ok(read_down)) => (f64::from(read_across), f64::from(read_down)),
      (Err(_not_a_finite_number), _) | (_, Err(_not_a_finite_number)) => continue,
    };

    match where_(&picture, Point { x: across, y: down }, logical) {
      Ok(color) => println!("  {:<12} #{color}", format!("{across},{down}")),
      Err(why) => eprintln!("  {place}: {why}"),
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn describing_a_panel_never_needs_a_wallpaper_process() {
    assert_eq!(
      wallpaper_for(Ended::ByThis, Action::Describing),
      Ok(Wallpaper::LeftOut)
    );
    assert_eq!(
      wallpaper_for(Ended::ByThis, Action::Running),
      Ok(Wallpaper::Started)
    );
    assert_eq!(
      wallpaper_for(Ended::ByThis, Action::Probing),
      Ok(Wallpaper::Started)
    );
    assert_eq!(
      wallpaper_for(Ended::ByTheCompositor, Action::Running),
      Ok(Wallpaper::LeftOut)
    );
  }
}
