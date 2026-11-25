use console_compositor::setup::{Mode, ModeMatch, Output, OutputRule, configure_outputs};
use console_core_never::Never;
use console_core_number_conversion::{Float, whole_u32};
use console_core_state_machine::SingleEffectMachine;
use console_notifications::delivery::Options;
use console_program_lifetime::Detached;
use console_waiting::Schedule;
use serde::{Deserialize, Serialize};

use super::{
  Agreement, DISPLAY_POLL, DISPLAY_WAIT, Display, Live, OutputScale, Profile, Screen, Topology,
  apply,
};
use crate::Unresumed;
use crate::session::Really;

mod change;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Specifications {
  millimeters: Option<(i64, i64)>,
  maximum_mode: Option<((i64, i64), u32)>,
  built_in: Panel,
}

impl Specifications {
  pub(super) fn of(output: &Output) -> Result<Self, Never> {
    let maximum = output.available_modes.iter().max_by(|first, second| {
      let pixels = |mode: &Mode| mode.size.0.saturating_mul(mode.size.1);
      pixels(first)
        .cmp(&pixels(second))
        .then_with(|| first.size.cmp(&second.size))
        .then_with(|| first.refresh.0.total_cmp(&second.refresh.0))
    });
    let Ok(panel) = built_in(&output.connector);
    Ok(Self {
      millimeters: output.physical_size_mm,
      maximum_mode: maximum.map(|mode| {
        let Ok(refresh) = whole_u32(mode.refresh.0);
        (mode.size, refresh)
      }),
      built_in: panel,
    })
  }

  fn similar(self, other: Self) -> Result<Agreement, Never> {
    let (wide, tall, other_wide, other_tall) = {
      let outcome_wide = (self.millimeters, other.millimeters);

      match outcome_wide {
        (Some((matched_wide, matched_tall)), Some((matched_other_wide, matched_other_tall))) => (
          matched_wide,
          matched_tall,
          matched_other_wide,
          matched_other_tall,
        ),
        (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {
          return Ok(Agreement::Different);
        }
      }
    };

    let near = |first: i64, second: i64| {
      let least = first.min(second);
      least >= 0 && first.abs_diff(second).saturating_mul(10) <= least.unsigned_abs()
    };

    let (resolution, refresh, other_resolution, other_refresh) = {
      let outcome_resolution = (self.maximum_mode, other.maximum_mode);

      match outcome_resolution {
        (
          Some((matched_resolution, matched_refresh)),
          Some((matched_other_resolution, matched_other_refresh)),
        ) => (
          matched_resolution,
          matched_refresh,
          matched_other_resolution,
          matched_other_refresh,
        ),
        (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {
          return Ok(Agreement::Different);
        }
      }
    };

    let resembles = resolution == other_resolution
      && refresh.abs_diff(other_refresh).saturating_mul(5) <= refresh.min(other_refresh)
      && near(wide, other_wide)
      && near(tall, other_tall);
    Ok(Agreement::from(resembles))
  }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "bool", into = "bool")]
enum Panel {
  BuiltIn,
  #[default]
  External,
}

impl From<bool> for Panel {
  fn from(built_in: bool) -> Self {
    match built_in.then_some(()) {
      Some(()) => Self::BuiltIn,
      None => Self::External,
    }
  }
}

impl From<Panel> for bool {
  fn from(panel: Panel) -> Self {
    matches!(panel, Panel::BuiltIn)
  }
}

fn built_in(connector: &str) -> Result<Panel, Never> {
  let internal = ["eDP-", "LVDS-", "DSI-"]
    .into_iter()
    .any(|prefix| connector.starts_with(prefix));
  Ok(match bool::then_some(internal, ()) {
    Some(()) => Panel::BuiltIn,
    None => Panel::External,
  })
}

fn preference<'a>(
  profiles: &'a [Profile],
  display: &Display,
  output: &Output,
) -> Result<Option<&'a Screen>, Never> {
  let Ok(specifications) = Specifications::of(output);
  let mut screens = profiles
    .iter()
    .rev()
    .flat_map(|profile| profile.screens.iter())
    .filter(|screen| {
      let existing_builtin = screen
        .display
        .connector
        .as_deref()
        .is_some_and(|connector| matches!(built_in(connector), Ok(Panel::BuiltIn)));
      screen.specifications.built_in == Panel::External && !existing_builtin
    });
  let same_model = |screen: &&Screen| {
    !display.model.trim().is_empty()
      && display.make == screen.display.make
      && display.model == screen.display.model
  };
  let known = screens.clone().find(|screen| screen.display == *display);
  let by_model = known.or_else(|| screens.clone().find(same_model));
  let compatible = match by_model {
    Some(saved) => Some(saved),
    None => screens.find(|screen| {
      matches!(
        specifications.similar(screen.specifications),
        Ok(Agreement::Same)
      )
    }),
  };
  Ok(compatible)
}

fn scale_fits(
  (width, height): (i64, i64),
  OutputScale(scale): OutputScale,
) -> Result<Agreement, Never> {
  let Ok(pixel_width) = width.float();
  let Ok(pixel_height) = height.float();
  let fits = scale.is_finite()
    && scale >= 0.25
    && [pixel_width, pixel_height].iter().all(|pixels| {
      let points = *pixels / scale;
      (points - points.round()).abs() < 0.0001
    });
  Ok(Agreement::from(fits))
}

fn preferred_rule<'a>(
  previous: OutputRule<'a>,
  output: &Output,
  saved: &Screen,
) -> Result<OutputRule<'a>, Never> {
  let requested = Mode {
    size: saved.geometry.mode,
    refresh: saved.geometry.refresh.0,
  };
  let saved_scale = saved.geometry.scale.0;
  let current_scale = previous.scale;
  let supported = output.available_modes.iter().find(|mode| {
    let Ok(agreement) = requested.matches(**mode);
    agreement == ModeMatch::Same
      && (matches!(scale_fits(mode.size, saved_scale), Ok(Agreement::Same))
        || matches!(scale_fits(mode.size, current_scale), Ok(Agreement::Same)))
  });
  let (mode, refresh) = supported.map_or((previous.mode, previous.refresh), |mode| {
    (mode.size, mode.refresh)
  });
  let Ok(agreement) = scale_fits(mode, saved_scale);
  let selected = match agreement {
    Agreement::Same => saved_scale,
    Agreement::Different => current_scale,
  };
  Ok(OutputRule {
    mode,
    refresh,
    scale: selected,
    ..previous
  })
}

fn settled(
  rule: OutputRule<'_>,
  topology: &Topology,
  id: i64,
) -> Result<Option<Live>, Unresumed> {
  let Ok(patience) = Schedule::asking_every(DISPLAY_WAIT, DISPLAY_POLL);
  let Ok(current) = console_waiting::until_some(patience, || {
    let live = match Live::read() {
      Ok(live) => {
        let Ok(observed) = live.topology();

        match bool::then_some(&observed == topology, ()) {
          Some(()) => live,
          None => return Ok(Some(Err(Unresumed::DisplaysChanged))),
        }
      }
      Err(fault) => return Ok(Some(Err(fault))),
    };
    let expected = Mode {
      size: rule.mode,
      refresh: rule.refresh,
    };
    let changed = live.outputs.iter().any(|(_, output)| {
      let Ok(mode) = output.current_mode();
      let Ok(agreement) = mode.matches(expected);
      output.id == id
        && agreement == ModeMatch::Same
        && (output.scale.0 - rule.scale.0).abs() < 0.0001
    });
    Ok(changed.then_some(Ok(live)))
  });

  current.transpose()
}

fn apply_preference(
  previous: OutputRule<'_>,
  wanted: OutputRule<'_>,
  topology: &Topology,
  id: i64,
) -> Result<Option<Live>, Unresumed> {
  match {
    ::core::primitive::bool::then_some(
      previous.mode == wanted.mode
        && (previous.refresh.0 - wanted.refresh.0).abs() < 0.0051
        && (previous.scale.0 - wanted.scale.0).abs() < 0.0001,
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  use change::{Effect, ModeChange, Reply, State};

  let Ok(first) = ModeChange::initialize(&(), None);

  let outcome = ModeChange::run_effects(first, 4, &mut (), |(), effect| match effect {
    Effect::ApplyPreferred => Reply::Applied(apply_mode(wanted)),
    Effect::VerifyPreferred => Reply::Settled(settled(wanted, topology, id)),
    Effect::Restore(failure) => {
      match failure {
        Some(fault) => {
          console_core_logging::warn!("console-resume: trying display preferences: {fault}");
        }
        ::core::option::Option::None => {}
      }

      Reply::Applied(apply_mode(previous))
    }
    Effect::VerifyRestore => Reply::Settled(settled(previous, topology, id)),
  });

  match outcome {
    Ok(State::Finished(result)) => result,
    Ok(State::Applying | State::Verifying | State::Restoring | State::VerifyingRestore)
    | Err(_) => Err(Unresumed::ApplyingSetup(
      "display preferences did not settle".to_owned(),
    )),
  }
}

fn apply_mode(rule: OutputRule<'_>) -> Result<(), Unresumed> {
  let Ok(script) = configure_outputs(&[rule]);
  apply(&script, Really::Truly)
}

pub(super) struct Notice {
  name: String,
  copied: bool,
}

pub(super) fn prepare(
  profiles: &[Profile],
  mut live: Live,
) -> Result<(Live, Vec<Notice>), Unresumed> {
  let Ok(topology) = live.topology();
  let outputs: Vec<_> = live
    .outputs
    .iter()
    .map(|(_, output)| output.clone())
    .collect();
  let mut settled = None;
  let mut notices = Vec::new();

  for (display, output) in &live.outputs {
    let Ok(panel) = built_in(&output.connector);

    match panel {
      Panel::BuiltIn => continue,
      Panel::External => {}
    }

    let Ok(saved) = preference(profiles, display, output);
    let Ok(previous) = output.rule(&outputs);
    let copied = match saved {
      Some(saved_2) => {
        let Ok(wanted) = preferred_rule(previous, output, saved_2);
        let changed = apply_preference(previous, wanted, &topology, output.id)?;
        let copied = changed.is_some();
        settled = changed.or(settled);
        copied
      }
      ::core::option::Option::None => false,
    };

    match {
      ::core::primitive::bool::then_some(
        saved.is_none_or(|screen| screen.display != *display),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        let model = output.model.trim();
        let name = match { ::core::primitive::bool::then_some(model.is_empty(), ()) } {
          ::core::option::Option::Some(()) => &output.connector,
          ::core::option::Option::None => model,
        };
        notices.push(Notice {
          name: name.to_owned(),
          copied,
        });
      }
      ::core::option::Option::None => {}
    }
  }

  match settled {
    Some(current) => {
      live = current;
    }
    ::core::option::Option::None => {}
  }

  Ok((live, notices))
}

pub(super) fn notify(notice: Notice) -> Result<Option<Detached>, Never> {
  let summary = format!("Set up {}?", notice.name);
  let question =
    "Open Settings → Display to choose your preferred resolution, refresh rate, and zoom.";
  let body = match { ::core::primitive::bool::then_some(notice.copied, ()) } {
    ::core::option::Option::Some(()) => {
      format!("Started with compatible settings from a similar display. {question}")
    }
    ::core::option::Option::None => question.to_owned(),
  };
  let options = Options {
    app_name: Some("Console"),
    ..Options::default()
  };
  let Ok(mut command) = options.command(&summary, Some(&body));

  Ok(match console_program_lifetime::let_go(&mut command) {
    Ok(child) => Some(child),
    Err(fault) => {
      console_core_logging::warn!("console-resume: showing the display setup prompt: {fault}");
      None
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_compositor::setup::Outputs;

  const DISPLAYS: &str = r#"[
    {"id":1,"name":"DP-1","make":"North","model":"Studio","serial":"1",
     "physicalWidth":600,"physicalHeight":340,"width":2560,"height":1440,
     "refreshRate":144,"x":0,"y":0,"scale":1.25,"transform":0,
     "availableModes":["2560x1440@144.00Hz","2560x1440@60.00Hz"]},
    {"id":2,"name":"DP-2","make":"West","model":"Desk","serial":"2",
     "physicalWidth":610,"physicalHeight":350,"width":2560,"height":1440,
     "refreshRate":60,"x":100,"y":200,"scale":1,"transform":1,
     "availableModes":["2560x1440@165.00Hz","2560x1440@144.00Hz","2560x1440@60.00Hz"]},
    {"id":3,"name":"DP-3","make":"West","model":"Travel","serial":"3",
     "physicalWidth":400,"physicalHeight":230,"width":2560,"height":1440,
     "refreshRate":60,"x":0,"y":0,"scale":1,"transform":0,
     "availableModes":["2560x1440@144.00Hz","2560x1440@60.00Hz"]}
  ]"#;

  #[test]
  fn a_similar_display_reuses_only_settings_it_can_show() -> Result<(), Box<dyn std::error::Error>>
  {
    let outputs = console_compositor::read(Outputs, DISPLAYS)?;
    let Ok(live) = Live::from_outputs(outputs.clone());

    let [(identity, base), (similar, output), (different, small)] = match live.outputs.as_slice()
    {
      [a, b, c] => [a, b, c],
      _ => {
        return Err(Box::<dyn std::error::Error>::from(
          "expected three displays",
        ));
      }
    };
    let Ok(geometry) = live.geometry(base);
    let Ok(specifications) = Specifications::of(base);
    let saved = Screen {
      display: identity.clone(),
      geometry,
      specifications,
      active: None,
      focused: super::super::Focus::Unfocused,
    };
    let profiles = [Profile {
      screens: super::super::ScreenList(vec![saved]),
      workspaces: Default::default(),
    }];

    let Ok(matched) = preference(&profiles, similar, output);
    let source = matched.ok_or("similar monitor")?;
    assert_eq!(source.display, *identity);
    assert!(matches!(preference(&profiles, different, small), Ok(None)));
    assert!(
      matches!(preference(&profiles, identity, base), Ok(Some(screen)) if screen.display == *identity)
    );

    let Ok(previous) = output.rule(&outputs);
    let Ok(wanted) = preferred_rule(previous, output, source);
    assert_eq!(wanted.refresh.0, 144.0);
    assert_eq!(wanted.scale.0, 1.25);
    assert_eq!(wanted.position, (100, 200));
    assert_eq!(wanted.transform, 1);

    let mut limited = output.clone();
    limited.available_modes.retain(|mode| {
      ::core::primitive::f64::to_bits(mode.refresh.0) == ::core::primitive::f64::to_bits(60.0)
    });
    let Ok(wanted_2) = preferred_rule(previous, &limited, source);
    assert_eq!(wanted_2.refresh.0, 60.0);
    assert_eq!(wanted_2.scale.0, 1.25);

    Ok(())
  }
}
