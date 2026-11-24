use console_compositor::setup::{ModeMatch, Output};
use console_core_internal_programs::InternalProgram;
use console_core_localization::text;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_panel::page::{Aside, Row, Showing};
use console_panel::tasks::Task;
use console_program_contract::{Command, ExitStatus, Notification};

use crate::rows::choices;
use crate::words::Word;

use super::{Side, mode_at_resolution, modes, outputs_for_layout, placed};

#[must_use]
fn label(output: &Output) -> core::result::Result<String, Never> {
  Ok(
    match bool::then_some(
      output.model.trim().is_empty() || output.model == output.connector,
      (),
    ) {
      Some(()) => output.connector.clone(),
      None => {
        format!("{} ({})", output.model, output.connector)
      }
    },
  )
}

pub(crate) fn selector(
  outputs: &[Output],
  selected: &Output,
  choose: impl Fn(&dyn Showing, String) + Send + Sync + 'static,
) -> Result<Row, Never> {
  match bool::then_some(outputs.len() < 2, ()) {
    Some(()) => {
      let model = selected.model.trim();
      let name = match bool::then_some(model.is_empty(), ()) {
        Some(()) => selected.connector.as_str(),
        None => model,
      };

      return Row::naming(name, Aside(""));
    }
    None => {}
  }

  let Ok(name) = text(&Word::ConnectedDisplay);
  let said = outputs
    .iter()
    .map(|output| {
      let Ok(label) = label(output);
      label
    })
    .collect();
  let connectors: Vec<_> = outputs
    .iter()
    .map(|output| output.connector.clone())
    .collect();
  let Ok(chosen) = fitted(
    match outputs.iter().position(|output| output.id == selected.id) {
      Some(position) => position,
      None => 0,
    },
  );
  let Ok(control) = choices(said, chosen, move |showing, at| {
    let Ok(at_2) = index(at);

    {
      match connectors.get(at_2) {
        Some(connector) => {
          choose(showing, connector.clone());
        }
        None => {}
      };
    }
  });
  Row::picking(&name, control)
}

struct Choice {
  label: String,
  command: Option<Task>,
}

#[derive(Clone, Copy)]
enum Selection {
  Current,
  Available,
}

impl From<bool> for Selection {
  fn from(chosen: bool) -> Self {
    match bool::then_some(chosen, ()) {
      Some(()) => Self::Current,
      None => Self::Available,
    }
  }
}

impl Choice {
  #[must_use]
  fn new(
    label: String,
    selected: Selection,
    display: &Output,
    action: &[&str],
  ) -> core::result::Result<Self, Never> {
    let command = match selected {
      Selection::Current => None,
      Selection::Available => {
        let arguments: Vec<_> = ["display", display.connector.as_str()]
          .into_iter()
          .chain(action.iter().copied())
          .collect();
        let Ok(runs) = Command::internal(InternalProgram::Scale, &arguments);
        let Ok(task) = display_task(runs);
        Some(task)
      }
    };
    Ok(Self { label, command })
  }
}

pub(crate) fn display_task(runs: Command) -> Result<Task, Never> {
  let Ok(task) = Task::of(runs);
  task.then(|answer| {
    let summary = match answer.status {
      ExitStatus::Success => match bool::then_some(answer.output.trim().is_empty(), ()) {
        Some(()) => return Ok(None),
        None => answer.output.trim(),
      },
      ExitStatus::Interrupt => return Ok(None),
      ExitStatus::Failure(_) => "Could not change display settings",
    };
    let details = answer.error_output.trim();
    Ok(Some(Notification {
      summary: summary.to_owned(),
      body: (!details.is_empty()).then(|| details.to_owned()),
    }))
  })
}

fn picker(word: Word, offered: Vec<Choice>) -> Result<Row, Never> {
  let Ok(name) = text(&word);

  match offered.as_slice() {
    [only] => return Row::text(&name, Aside(&only.label)),
    [] | [_, _, ..] => {}
  }

  let Ok(chosen) = fitted(
    match offered.iter().position(|choice| choice.command.is_none()) {
      Some(position) => position,
      None => 0,
    },
  );
  let (labels, tasks): (Vec<_>, Vec<_>) = offered
    .into_iter()
    .map(|choice| (choice.label, choice.command))
    .unzip();
  let Ok(control) = choices(labels, chosen, move |showing, at| {
    let Ok(at_2) = index(at);

    {
      match tasks.get(at_2) {
        Some(Some(task)) => {
          showing.run(task.clone());
        }
        None | Some(None) => {}
      };
    }
  });

  Row::picking(&name, control)
}

fn resolutions(display: &Output) -> Result<Row, Never> {
  let Ok(modes) = modes(display);
  let mut sizes: Vec<_> = modes.iter().map(|mode| mode.size).collect();
  sizes.dedup();
  let offered = sizes
    .into_iter()
    .filter_map(|size| {
      let (wide, tall) = size;
      let mode = ({
        let Ok(explicit_value) = mode_at_resolution(display, size, &modes);
        explicit_value
      })?;
      Some({
        let Ok(explicit_value) = Choice::new(
          format!("{wide} × {tall}"),
          Selection::from(size == display.mode),
          display,
          &["mode", &mode.to_string()],
        );
        explicit_value
      })
    })
    .collect();

  picker(Word::Resolution, offered)
}

fn refresh_rates(display: &Output) -> Result<Row, Never> {
  let Ok(current) = display.current_mode();
  let Ok(modes) = modes(display);
  let offered = modes
    .into_iter()
    .filter(|mode| mode.size == current.size)
    .map(|mode| {
      let Ok(agreement) = current.matches(mode);
      let value = match mode.refresh.0.fract().classify() {
        std::num::FpCategory::Zero => format!("{:.0}", mode.refresh),
        std::num::FpCategory::Nan
        | std::num::FpCategory::Infinite
        | std::num::FpCategory::Subnormal
        | std::num::FpCategory::Normal => format!("{:.2}", mode.refresh),
      };

      {
        let Ok(explicit_value) = Choice::new(
          format!("{value} Hz"),
          Selection::from(agreement == ModeMatch::Same),
          display,
          &["mode", &mode.to_string()],
        );
        explicit_value
      }
    })
    .collect();

  picker(Word::RefreshRate, offered)
}

fn position(display: &Output, outputs: &[Output]) -> Result<Row, Never> {
  let mut offered = Vec::new();
  let Ok(candidates) = outputs_for_layout(display, outputs);

  for anchor in candidates {
    'over_side_2: for side in Side::ALL {
      let Ok(at) = placed(display, anchor, side);
      let Ok(placement) = super::placement(display, at, outputs);

      match placement {
        super::Placement::Overlapping => {
          continue 'over_side_2;
        }
        super::Placement::Available => {}
      }

      offered.push({
        let Ok(direction) = side.word();
        let Ok(explicit_value) = Choice::new(
          format!("{direction} of {}", anchor.connector),
          Selection::from(display.position == at && display.mirror.is_none()),
          display,
          &["place", &anchor.connector, direction],
        );
        explicit_value
      });
    }
  }

  match bool::then_some(!offered.iter().any(|choice| choice.command.is_none()), ()) {
    Some(()) => {
      let label = match display.mirror {
        Some(_source) => "Mirrored",
        None => "Current position",
      };
      offered.insert(
        0,
        Choice {
          label: label.to_owned(),
          command: None,
        },
      );
    }
    None => {}
  }

  picker(Word::Arrangement, offered)
}

fn mirrors(display: &Output, outputs: &[Output]) -> Result<Option<Row>, Never> {
  let mut offered = vec![
    ({
      let Ok(explicit_value) = Choice::new(
        "Extend desktop".to_owned(),
        Selection::from(display.mirror.is_none()),
        display,
        &["extend"],
      );
      explicit_value
    }),
  ];
  let Ok(sources) = super::mirror_sources(display, outputs);
  offered.extend(sources.map(|source| {
    let Ok(explicit_value) = Choice::new(
      {
        let Ok(explicit_value) = label(source);
        explicit_value
      },
      Selection::from(display.mirror == Some(source.id)),
      display,
      &["mirror", &source.connector],
    );
    explicit_value
  }));

  match bool::then_some(offered.len() == 1, ()) {
    Some(()) => {
      return Ok(None);
    }
    None => {}
  }

  let Ok(row) = picker(Word::MirrorDisplay, offered);

  Ok(Some(row))
}

pub(crate) fn settings(display: &Output, outputs: &[Output]) -> Result<Vec<Row>, Never> {
  let Ok(resolutions) = resolutions(display);
  let Ok(refresh_rates) = refresh_rates(display);
  let mut rows = vec![resolutions, refresh_rates];

  match bool::then_some(outputs.len() > 1, ()) {
    Some(()) => {
      let Ok(position) = position(display, outputs);
      let Ok(mirror) = mirrors(display, outputs);
      rows.push(position);
      rows.extend(mirror);
    }
    None => {}
  }

  Ok(rows)
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_compositor::setup::Outputs;
  use console_panel::page::Heading;

  #[test]
  fn a_single_display_has_a_name_and_only_its_own_controls()
  -> Result<(), Box<dyn std::error::Error>> {
    let outputs = console_compositor::read(
      Outputs,
      r#"[
      {"id":0,"name":"eDP-1","model":"Legion Go","width":1920,"height":1080,
       "refreshRate":60,"x":0,"y":0,"scale":1,"transform":0}
    ]"#,
    )?;
    let display = outputs.first().ok_or("connected display")?;
    let Ok(heading) = selector(&outputs, display, |_, _| {});

    assert_eq!(heading.says, "Legion Go");
    assert_eq!(heading.heading(), Ok(Heading::Yes));
    assert!(heading.aside.is_empty());
    assert!(heading.choices.is_none());
    assert!(heading.does.is_none());

    let Ok(rows) = settings(display, &outputs);
    let labels: Vec<_> = rows.into_iter().map(|row| row.says).collect();
    let Ok(resolution) = text(&Word::Resolution);
    let Ok(refresh_rate) = text(&Word::RefreshRate);
    assert_eq!(labels, [resolution, refresh_rate]);

    Ok(())
  }

  #[test]
  fn refresh_choices_follow_the_selected_displays_current_resolution()
  -> Result<(), Box<dyn std::error::Error>> {
    let outputs = console_compositor::read(
      Outputs,
      r#"[
      {"id":0,"name":"eDP-1","width":1920,"height":1080,"refreshRate":60,
       "x":0,"y":0,"scale":1,"transform":0,"availableModes":["1920x1080@60.00Hz"]},
      {"id":1,"name":"DP-2","width":2560,"height":1440,"refreshRate":59.943,
       "x":1920,"y":0,"scale":1,"transform":0,
       "availableModes":["1920x1080@240.00Hz","2560x1440@59.94Hz",
                         "2560x1440@144.00Hz","2560x1440@144.00Hz","invalid"]}
    ]"#,
    )?;
    let Ok(found) = super::super::selected(&outputs, Some("DP-2"));
    let display = found.ok_or("external display")?;
    let Ok(selector) = selector(&outputs, display, |_, _| {});
    let picker = selector.choices.ok_or("a choice of connected displays")?;

    assert_eq!(picker.said, ["eDP-1", "DP-2"]);
    assert_eq!(picker.chosen, 1);

    let Ok(refresh) = refresh_rates(display);
    let controls = refresh.choices.ok_or("two supported refresh rates")?;

    assert_eq!(controls.said, ["144 Hz", "59.94 Hz"]);
    assert_eq!(controls.chosen, 1);

    let Ok(size) = resolutions(display);
    let controls_2 = size.choices.ok_or("two supported resolutions")?;
    assert_eq!(controls_2.said, ["2560 × 1440", "1920 × 1080"]);
    assert_eq!(controls_2.chosen, 0);

    let Ok(position) = position(display, &outputs);
    let controls_3 = position.choices.ok_or("multiple relative positions")?;
    assert_eq!(
      controls_3.said.get(1).map(String::as_str),
      Some("Right of eDP-1")
    );
    assert_eq!(controls_3.chosen, 1);

    Ok(())
  }
}
