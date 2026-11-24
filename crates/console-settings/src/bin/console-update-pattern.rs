//! A separate desktop dialog for approving a Console release by pattern.

use std::error::Error;
use std::io::Write;
use std::path::Path;
use std::process::{ExitCode, Output, Stdio};

use console_core_color::{Oklch, palette::Wearing};
use console_core_external_programs::Program;
use console_core_geometry::{Containment, Point, Rectangle, Size};
use console_core_internal_programs::InternalProgram;
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{index, toward_zero_i32, whole_u32};
use console_core_shapes::{Edge, Font, Line, Panel, Round, Shape, Text, Weight};
use console_draw_painting::{Rendered, Run, measure_text, painter};
use console_draw_surface::{
  Anchor, Closed, Keyboard, KeyboardEvent, Margin, PointerEvent, Room, Surface, Under, Wanted,
};
use console_input_event_devices::presses::ButtonPress;
use console_login_greeter::session::press;
use console_login_pattern::{
  Clicked, DOTS, Direction, LOGIN, LOGIN_SIZE, Pattern, Secret, Target, Touch, dot,
};

const WHO: &str = "console-update-pattern";
const WIDTH: u32 = 520;
const HEIGHT: u32 = 552;
const CARD: Rectangle<i32> = Rectangle {
  origin: Point { x: 0, y: 0 },
  size: Size {
    width: 520,
    height: 552,
  },
};
const CLOSE: Rectangle<i32> = Rectangle {
  origin: Point { x: 452, y: 12 },
  size: Size {
    width: 52,
    height: 52,
  },
};
const RING: Point<i32> = Point { x: 40, y: 88 };

type DialogError = Box<dyn Error>;

enum Choice {
  Continue,
  Close,
  Install(Secret),
}

struct Dialog {
  pattern: Pattern,
  focus: DialogFocus,
  feedback: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DialogFocus {
  Pattern,
  Close,
}

#[derive(Clone, Copy)]
enum Highlight {
  Focused,
  Muted,
}

impl From<bool> for Highlight {
  fn from(focused: bool) -> Self {
    match focused {
      true => Self::Focused,
      false => Self::Muted,
    }
  }
}

#[derive(Clone, Copy)]
struct Layout {
  origin: Point<f64>,
  scale: DialogScale,
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this scale is a ratio of target pixels to dialog pixels"
  )
)]
struct DialogScale(f64);

struct Notice<'a> {
  title: &'a str,
  message: &'a str,
}

#[derive(Clone, Copy)]
struct Approval<'a> {
  request: &'a str,
  release: &'a str,
  title: &'static str,
  verb: &'static str,
  completed: &'static str,
}

struct PromptText<'a> {
  approval: Approval<'a>,
  feedback: &'static str,
}

enum Attempt {
  Finished,
  Retry(&'static str),
}

fn main() -> ExitCode {
  let arguments: Vec<_> = std::env::args().skip(1).collect();
  let Ok(request) = selected(&arguments);
  let approval = match request {
    Some(approval) => approval,
    None => {
      eprintln!("Usage: {WHO} RELEASE_TAG | --undo TARGET RELEASE");
      return ExitCode::FAILURE;
    }
  };

  match approve(approval) {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      let message = fault.to_string();
      eprintln!("{WHO}: {message}");
      let ::core::result::Result::Ok(()) = notice(Notice {
        title: "Console update failed",
        message: &message,
      });
      ExitCode::FAILURE
    }
  }
}

fn selected(arguments: &[String]) -> Result<Option<Approval<'_>>, Never> {
  Ok(match arguments {
    [tag] => Some(Approval {
      request: tag,
      release: tag,
      title: "Install Console update",
      verb: "Install",
      completed: "Console update installed",
    }),
    [flag, target, label] => match flag.as_str() {
      "--undo" => Some(Approval {
        request: target,
        release: label,
        title: "Undo last update",
        verb: "Restore",
        completed: "Previous Console release restored",
      }),
      _ => None,
    },
    _ => None,
  })
}

fn approve(approval: Approval<'_>) -> Result<(), DialogError> {
  let wearing = Wearing::worn()?;
  let outcome = iterate("", |feedback| {
    let prompt = PromptText { approval, feedback };
    Ok(match attempt(prompt, &wearing) {
      Ok(Attempt::Finished) => Step::Halt(Ok(())),
      Ok(Attempt::Retry(message)) => Step::Again(message),
      Err(fault) => Step::Halt(Err(fault)),
    })
  })?;
  outcome
}

fn attempt(text: PromptText<'_>, wearing: &Wearing) -> Result<Attempt, DialogError> {
  let approval = text.approval;
  let entered = prompt(text, wearing)?;
  let secret = match entered {
    Some(secret) => secret,
    None => return Ok(Attempt::Finished),
  };
  let result = submit(approval.request, &secret)?;

  match result.status.success().then_some(()) {
    Some(()) => {
      let Ok(()) = notice(Notice {
        title: approval.completed,
        message: approval.release,
      });
      return Ok(Attempt::Finished);
    }
    None => {}
  }

  let stderr = std::str::from_utf8(&result.stderr)?;
  let message = match stderr.trim().lines().last() {
    Some(line) => line,
    None => "The installer stopped.",
  };
  let reason = match message.strip_prefix("console-update: ") {
    Some(reason) => reason,
    None => message,
  };

  match reason {
    "the update pattern did not match" => Ok(Attempt::Retry("Pattern did not match. Try again.")),
    "wait a few seconds before trying the update pattern again" => {
      Ok(Attempt::Retry("Wait a few seconds, then try again."))
    }
    _ => Err(DialogError::from(reason.to_owned())),
  }
}

fn submit(request: &str, secret: &Secret) -> Result<Output, DialogError> {
  let Ok(updater) = InternalProgram::Update.path();
  let Ok(mut sudo) = Program::Sudo.command_from(Path::new("/usr/bin"));
  let mut child = sudo
    .args(["-n", "--", updater, "--install-with-pattern"])
    .stdin(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
  let Ok(letters) = secret.as_str();
  let mut pipe = child
    .stdin
    .take()
    .ok_or("the installer did not open an input pipe")?;
  let sent = writeln!(pipe, "{request}\n{letters}");
  drop(pipe);
  let result = child.wait_with_output()?;

  match { ::core::primitive::bool::then_some(result.status.success(), ()) } {
    ::core::option::Option::Some(()) => {
      sent?;
    }
    ::core::option::Option::None => {}
  }

  Ok(result)
}

fn notice(
  Notice { title, message }: Notice<'_>,
) -> ::core::result::Result<(), ::console_core_never::Never> {
  let Ok(mut notify) = console_operating_system_linux::notification::Options::default()
    .command(title, Some(message));
  let _status = notify.status();
  ::core::result::Result::Ok(())
}

fn prompt(
  PromptText { approval, feedback }: PromptText<'_>,
  wearing: &Wearing,
) -> Result<Option<Secret>, DialogError> {
  let mut surface = Surface::connect()?;
  surface.show(&Wanted {
    namespace: WHO.to_owned(),
    anchor: Anchor::Whole,
    size: Size {
      width: 0,
      height: 0,
    },
    margin: Margin::default(),
    keyboard: Keyboard::Takes,
    room: Room::Over,
    under: Under::None,
  })?;
  let Ok(dialog) = Dialog::new(feedback);
  let initial = PromptWindow {
    surface,
    dialog,
    rendered: Rendered::default(),
  };
  let result = iterate(initial, |mut window| {
    Ok(match window.turn(approval, wearing) {
      Ok(Choice::Continue) => Step::Again(window),
      Ok(Choice::Close) => Step::Halt(Ok(None)),
      Ok(Choice::Install(secret)) => Step::Halt(Ok(Some(secret))),
      Err(fault) => Step::Halt(Err(fault)),
    })
  })?;
  result
}

struct PromptWindow {
  surface: Surface,
  dialog: Dialog,
  rendered: Rendered,
}

impl PromptWindow {
  fn turn(&mut self, approval: Approval<'_>, wearing: &Wearing) -> Result<Choice, DialogError> {
    let Ok(closed) = self.surface.closed();

    match closed {
      Closed::Yes => return Ok(Choice::Close),
      Closed::No => {}
    }

    let Ok(logical) = self.surface.logical();

    match logical {
      Some(canvas) => {
        let Ok(layout) = Layout::centred(canvas);
        let Ok(choice) = self.input(&layout);

        match choice {
          Choice::Close | Choice::Install(_) => return Ok(choice),
          Choice::Continue => self.paint(&layout, canvas, approval, wearing)?,
        }
      }
      None => {}
    }

    self.surface.wait(&[], None)?;
    Ok(Choice::Continue)
  }

  fn input(&mut self, layout: &Layout) -> Result<Choice, Never> {
    let Ok(keys) = self.surface.keyboard_events();
    let Ok(pointers) = self.surface.pointer_events();
    let buttons = keys.into_iter().filter_map(|KeyboardEvent::Down { key }| {
      let Ok(button) = press(key);
      button.map(Input::Pad)
    });
    let touches = pointers.into_iter().filter_map(|event| {
      let Ok(input) = layout.pointer(event);
      input
    });

    for input in buttons.chain(touches) {
      let Ok(choice) = self.dialog.handle(input);

      match choice {
        Choice::Continue => {}
        Choice::Close | Choice::Install(_) => return Ok(choice),
      }
    }

    Ok(Choice::Continue)
  }

  fn paint(
    &mut self,
    layout: &Layout,
    canvas: Size<u32>,
    approval: Approval<'_>,
    wearing: &Wearing,
  ) -> Result<(), DialogError> {
    let Ok(composed) = layout.paint(&self.dialog, approval, wearing);
    let Ok(redraw) = self.rendered.wanted(composed);

    match redraw {
      Some(drawing) => {
        let Ok(()) = self.surface.resize(canvas);
        let Ok(paint) = painter(canvas, drawing, WHO);
        self.surface.draw(paint)?;
      }
      None => {}
    }

    Ok(())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Input {
  Pad(ButtonPress),
  Touch(Touch),
  Close,
}

impl Dialog {
  #[must_use]
  fn new(feedback: &'static str) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(pattern) = Pattern::new();
    Ok(Self {
      pattern,
      focus: DialogFocus::Pattern,
      feedback,
    })
  }

  fn handle(&mut self, input: Input) -> core::result::Result<Choice, console_core_never::Never> {
    let clicked = match (input, self.focus, self.pattern.at) {
      (Input::Close, _, _) => return Ok(Choice::Close),
      (Input::Touch(touch), _, _) => {
        self.focus = DialogFocus::Pattern;
        let Ok(clicked) = self.pattern.touch(touch);
        clicked
      }
      (Input::Pad(ButtonPress::Choose), DialogFocus::Close, _) => return Ok(Choice::Close),
      (Input::Pad(ButtonPress::Up), _, Target::Dot(0)) => {
        self.focus = DialogFocus::Close;
        return Ok(Choice::Continue);
      }
      (Input::Pad(_), DialogFocus::Close, _) => {
        self.focus = DialogFocus::Pattern;
        return Ok(Choice::Continue);
      }
      (Input::Pad(ButtonPress::Choose), DialogFocus::Pattern, _) => {
        let Ok(clicked) = self.pattern.click();
        clicked
      }
      (Input::Pad(ButtonPress::Back), DialogFocus::Pattern, _) => {
        let Ok(pattern) = self.pattern.undone();
        self.pattern = pattern;
        return Ok(Choice::Continue);
      }
      (Input::Pad(button), DialogFocus::Pattern, _) => {
        let direction = match button {
          ButtonPress::Up => Direction::Up,
          ButtonPress::Down => Direction::Down,
          ButtonPress::Left => Direction::Left,
          ButtonPress::Right => Direction::Right,
          ButtonPress::Back | ButtonPress::Choose => return Ok(Choice::Continue),
        };
        let Ok(pattern) = self.pattern.moved(direction);
        self.pattern = pattern;
        return Ok(Choice::Continue);
      }
    };

    match clicked {
      Clicked::Traced(pattern) => self.pattern = pattern,
      Clicked::Submitted(secret) => match self.pattern.path.len() {
        0..=3 => {
          let Ok(pattern) = self.pattern.cleared();
          self.pattern = pattern;
          self.feedback = "Join at least four dots.";
        }
        _enough_dots => return Ok(Choice::Install(secret)),
      },
    }

    Ok(Choice::Continue)
  }

  #[must_use]
  fn highlight(&self, target: Target) -> Result<Highlight, Never> {
    Ok(Highlight::from(
      self.focus == DialogFocus::Pattern && self.pattern.at == target,
    ))
  }
}

impl Layout {
  #[must_use]
  fn centred(canvas: Size<u32>) -> core::result::Result<Self, console_core_never::Never> {
    let (wide, tall) = (f64::from(canvas.width), f64::from(canvas.height));
    let scale = ((wide - 16.0) / f64::from(WIDTH))
      .min((tall - 16.0) / f64::from(HEIGHT))
      .clamp(0.01, 1.0);
    let origin = Point {
      x: (wide - f64::from(WIDTH) * scale) / 2.0,
      y: (tall - f64::from(HEIGHT) * scale) / 2.0,
    };
    Ok(Self {
      origin,
      scale: DialogScale(scale),
    })
  }

  #[must_use]
  fn pointer(
    &self,
    event: PointerEvent,
  ) -> core::result::Result<Option<Input>, console_core_never::Never> {
    let on_card = |(x, y)| {
      let Ok(x_2) = toward_zero_i32((x - self.origin.x) / self.scale.0);
      let Ok(y_2) = toward_zero_i32((y - self.origin.y) / self.scale.0);
      Point { x: x_2, y: y_2 }
    };
    let in_ring = |at: Point<i32>| Point {
      x: at.x.saturating_sub(RING.x),
      y: at.y.saturating_sub(RING.y),
    };

    Ok(match event {
      PointerEvent::Down { at } => {
        let at_2 = on_card(at);

        match {
          ::core::primitive::bool::then_some(CLOSE.contains(at_2) == Ok(Containment::Inside), ())
        } {
          ::core::option::Option::Some(()) => {
            return Ok(Some(Input::Close));
          }
          ::core::option::Option::None => {}
        }

        (CARD.contains(at_2) == Ok(Containment::Inside))
          .then_some(Input::Touch(Touch::Down(in_ring(at_2))))
      }
      PointerEvent::Moved { at } => Some(Input::Touch(Touch::Moved(in_ring(on_card(at))))),
      PointerEvent::Up => Some(Input::Touch(Touch::Up)),
      PointerEvent::Left | PointerEvent::Pinched { .. } | PointerEvent::Scrolled { .. } => None,
    })
  }

  #[must_use]
  fn point(&self, at: Point<i32>) -> core::result::Result<Point<i32>, console_core_never::Never> {
    let Ok(x) = toward_zero_i32(self.origin.x + f64::from(at.x) * self.scale.0);
    let Ok(y) = toward_zero_i32(self.origin.y + f64::from(at.y) * self.scale.0);
    Ok(Point { x, y })
  }

  #[must_use]
  fn length(&self, length: u32) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(length_2) = whole_u32(f64::from(length) * self.scale.0);
    Ok(length_2.max(1))
  }

  #[must_use]
  fn panel(&self, mut panel: Panel) -> core::result::Result<Shape, console_core_never::Never> {
    panel.at = {
      let Ok(explicit_value) = self.point(panel.at);
      explicit_value
    };
    panel.size = Size {
      width: ({
        let Ok(explicit_value) = self.length(panel.size.width);
        explicit_value
      }),
      height: ({
        let Ok(explicit_value) = self.length(panel.size.height);
        explicit_value
      }),
    };
    panel.round = Round({
      let Ok(explicit_value) = self.length(panel.round.0);
      explicit_value
    });

    match panel.edge {
      Edge::Of { wide, color } => {
        let Ok(width) = self.length(wide);
        panel.edge = Edge::Of { wide: width, color };
      }
      Edge::None => {}
    }

    Ok(Shape::Panel(panel))
  }

  #[must_use]
  fn text(&self, mut text: Text) -> core::result::Result<Shape, console_core_never::Never> {
    text.at = {
      let Ok(explicit_value) = self.point(text.at);
      explicit_value
    };
    text.width = {
      let Ok(explicit_value) = self.length(text.width);
      explicit_value
    };
    text.font.height = {
      let Ok(explicit_value) = self.length(text.font.height);
      explicit_value
    };
    Ok(Shape::Text(text))
  }

  #[must_use]
  fn line(
    &self,
    from: Point<i32>,
    to: Point<i32>,
    color: Oklch,
  ) -> core::result::Result<Shape, console_core_never::Never> {
    Ok(Shape::Line(Line {
      from: ({
        let Ok(explicit_value) = self.point(from);
        explicit_value
      }),
      to: ({
        let Ok(explicit_value) = self.point(to);
        explicit_value
      }),
      width: ({
        let Ok(explicit_value) = self.length(4);
        explicit_value
      }),
      color,
    }))
  }

  #[must_use]
  fn paint(
    &self,
    dialog: &Dialog,
    approval: Approval<'_>,
    palette: &Wearing,
  ) -> Result<Vec<Shape>, Never> {
    let Ok(heading) = self.heading(approval, palette);
    let mut shapes = Vec::from(heading);
    let Ok(button) = shifted(LOGIN, RING);
    let Ok(install_focus) = dialog.highlight(Target::Login);
    let Ok(install) = self.button(button, LOGIN_SIZE, approval.verb, palette, install_focus);
    shapes.extend(install);

    let close = Point {
      x: CLOSE.origin.x.saturating_add(26),
      y: CLOSE.origin.y.saturating_add(26),
    };
    let close_focus = Highlight::from(dialog.focus == DialogFocus::Close);
    let Ok(close_button) = self.button(
      close,
      Size {
        width: 52,
        height: 52,
      },
      "×",
      palette,
      close_focus,
    );
    shapes.extend(close_button);
    let Ok(()) = self.trace(&mut shapes, &dialog.pattern, palette.coral);
    let Ok(()) = self.dots(&mut shapes, dialog, palette);

    match dialog.feedback {
      "" => {}
      feedback => {
        let Ok(typeface) = font(17);
        let Ok(line) = self.text(Text {
          at: Point { x: 26, y: 519 },
          width: 468,
          said: feedback.to_owned(),
          weight: Weight::Plain,
          font: typeface,
          ink: palette.text,
        });
        shapes.push(line);
      }
    }

    Ok(shapes)
  }

  fn heading(&self, approval: Approval<'_>, palette: &Wearing) -> Result<[Shape; 3], Never> {
    let Ok(backdrop) = self.panel(Panel {
      at: CARD.origin,
      size: Size {
        width: WIDTH,
        height: HEIGHT,
      },
      round: Round(24),
      fill: palette.panel,
      edge: Edge::Of {
        wide: 2,
        color: palette.edge,
      },
    });
    let Ok(title_font) = font(26);
    let Ok(title) = self.text(Text {
      at: Point { x: 26, y: 19 },
      width: 398,
      said: approval.title.to_owned(),
      weight: Weight::Bold,
      font: title_font,
      ink: palette.text,
    });
    let Ok(release_font) = font(18);
    let Ok(release) = self.text(Text {
      at: Point { x: 26, y: 52 },
      width: 398,
      said: approval.release.to_owned(),
      weight: Weight::Plain,
      font: release_font,
      ink: palette.soft,
    });
    Ok([backdrop, title, release])
  }

  fn trace(&self, shapes: &mut Vec<Shape>, pattern: &Pattern, ink: Oklch) -> Result<(), Never> {
    let mut dots = pattern.path.iter().filter_map(|number| {
      let Ok(dot) = dot(*number);
      dot.map(|key| {
        let Ok(point) = shifted(key.centre, RING);
        point
      })
    });
    let mut from = match dots.next() {
      Some(first) => first,
      None => return Ok(()),
    };

    for to in dots {
      let Ok(segment) = self.line(from, to, ink);
      shapes.push(segment);
      from = to;
    }

    match pattern.finger {
      Some(finger) => {
        let Ok(to) = shifted(finger, RING);
        let Ok(segment) = self.line(from, to, ink);
        shapes.push(segment);
      }
      None => {}
    }

    Ok(())
  }

  fn dots(
    &self,
    shapes: &mut Vec<Shape>,
    dialog: &Dialog,
    palette: &Wearing,
  ) -> Result<(), Never> {
    let mut fills = [palette.ground; DOTS.len()];

    for &number in &dialog.pattern.path {
      let Ok(position) = index(number);

      match fills.get_mut(position) {
        Some(fill) => *fill = palette.coral,
        None => {}
      }
    }

    for (number, (dot, fill)) in (0..).zip(DOTS.into_iter().zip(fills)) {
      let Ok(centre) = shifted(dot.centre, RING);
      let Ok(highlight) = dialog.highlight(Target::Dot(number));
      let Ok(edge) = outline(palette, highlight);
      let Ok(shape) = self.panel(Panel {
        at: Point {
          x: centre.x.saturating_sub(22),
          y: centre.y.saturating_sub(22),
        },
        size: dot.size,
        round: Round(22),
        fill,
        edge,
      });
      shapes.push(shape);
    }

    Ok(())
  }

  #[must_use]
  fn button(
    &self,
    centre: Point<i32>,
    size: Size<u32>,
    label: &str,
    palette: &Wearing,
    highlight: Highlight,
  ) -> core::result::Result<[Shape; 2], console_core_never::Never> {
    use console_core_number_conversion::fitted;
    let Ok(half_wide) = fitted::<_, i32>(::core::primitive::u32::wrapping_div(size.width, 2));
    let Ok(half_tall) = fitted::<_, i32>(::core::primitive::u32::wrapping_div(size.height, 2));
    let Ok(font) = font(23);
    let Ok(ink) = measure_text(
      Run {
        said: label,
        weight: Weight::Bold,
        width: size.width,
      },
      &font,
    );
    let Ok(ink_wide) = fitted::<_, i32>(::core::primitive::u32::wrapping_div(ink.width, 2));
    let Ok(ink_tall) = fitted::<_, i32>(::core::primitive::u32::wrapping_div(ink.height, 2));
    Ok([
      ({
        let Ok(explicit_value) = self.panel(Panel {
          at: Point {
            x: ::core::primitive::i32::wrapping_sub(centre.x, half_wide),
            y: ::core::primitive::i32::wrapping_sub(centre.y, half_tall),
          },
          size,
          round: Round(14),
          fill: palette.fill,
          edge: ({
            let Ok(explicit_value) = outline(palette, highlight);
            explicit_value
          }),
        });
        explicit_value
      }),
      ({
        let Ok(explicit_value) = self.text(Text {
          at: Point {
            x: ::core::primitive::i32::wrapping_sub(centre.x, ink_wide),
            y: ::core::primitive::i32::wrapping_sub(centre.y, ink_tall),
          },
          width: size.width,
          said: label.to_owned(),
          weight: Weight::Bold,
          font,
          ink: palette.text,
        });
        explicit_value
      }),
    ])
  }
}

#[must_use]
fn outline(
  palette: &Wearing,
  highlight: Highlight,
) -> core::result::Result<Edge, console_core_never::Never> {
  Ok(match highlight {
    Highlight::Focused => Edge::Of {
      wide: 4,
      color: palette.text,
    },
    Highlight::Muted => Edge::Of {
      wide: 1,
      color: palette.edge,
    },
  })
}

#[must_use]
fn shifted(
  point: Point<i32>,
  by: Point<i32>,
) -> core::result::Result<Point<i32>, console_core_never::Never> {
  Ok(Point {
    x: point.x.saturating_add(by.x),
    y: point.y.saturating_add(by.y),
  })
}

#[must_use]
fn font(height: u32) -> core::result::Result<Font, console_core_never::Never> {
  Ok(Font {
    family: "Noto Sans".to_owned(),
    height,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_scaled_dialog_closes_only_at_the_close_button() -> Result<(), DialogError> {
    let Ok(layout) = Layout::centred(Size {
      width: 400,
      height: 420,
    });
    let tap = |point| {
      let Ok(at) = layout.point(point);

      {
        let Ok(explicit_value) = layout.pointer(PointerEvent::Down {
          at: (f64::from(at.x), f64::from(at.y)),
        });
        explicit_value
      }
    };
    let Ok(mut dialog) = Dialog::new("");
    let blank = tap(Point { x: 260, y: 240 }).ok_or("blank press is inside the dialog")?;

    assert_eq!(tap(Point { x: -6, y: 78 }), None);
    let Ok(handle) = dialog.handle(blank);
    assert!(matches!(handle, Choice::Continue));
    let Ok(dialog_handle) = dialog.handle(Input::Touch(Touch::Up));
    assert!(matches!(dialog_handle, Choice::Continue));
    assert!(matches!(
      {
        let Ok(explicit_value) = dialog.handle(Input::Pad(ButtonPress::Back));
        explicit_value
      },
      Choice::Continue
    ));
    assert_eq!(tap(Point { x: 478, y: 38 }), Some(Input::Close));
    assert!(matches!(
      {
        let Ok(explicit_value) = dialog.handle(Input::Close);
        explicit_value
      },
      Choice::Close
    ));
    Ok(())
  }

  #[test]
  fn the_pad_can_focus_close_or_submit_the_pattern() {
    let Ok(mut dialog) = Dialog::new("");
    let Ok(handle) = dialog.handle(Input::Pad(ButtonPress::Up));
    assert!(matches!(handle, Choice::Continue));
    let Ok(dialog_handle) = dialog.handle(Input::Pad(ButtonPress::Choose));
    assert!(matches!(dialog_handle, Choice::Close));

    let Ok(mut submit_dialog) = Dialog::new("");
    submit_dialog.pattern.at = Target::Login;
    submit_dialog.pattern.path = vec![0, 1, 2, 3];
    let Ok(submit_dialog_handle) = submit_dialog.handle(Input::Pad(ButtonPress::Choose));
    assert!(matches!(submit_dialog_handle, Choice::Install(_)));
  }
}
