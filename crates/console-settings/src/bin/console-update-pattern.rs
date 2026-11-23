//! A separate desktop dialog for approving a Console release by pattern.

use std::error::Error;
use std::io::Write;
use std::process::{Command, ExitCode, Output, Stdio};

use console_core_color::{Oklch, palette::Wearing};
use console_core_geometry::{Containment, Point, Rectangle, Size};
use console_core_number_conversion::{toward_zero_i32, whole_u32};
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
  close_focused: bool,
  feedback: &'static str,
}

#[derive(Clone, Copy)]
struct Layout {
  origin: Point<f64>,
  scale: f64,
}

fn main() -> ExitCode {
  let arguments: Vec<_> = std::env::args().skip(1).collect();
  let [tag] = arguments.as_slice() else {
    eprintln!("Usage: {WHO} RELEASE_TAG");
    return ExitCode::FAILURE;
  };
  match approve(tag) {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      let message = fault.to_string();
      eprintln!("{WHO}: {message}");
      notice("Console update failed", &message);
      ExitCode::FAILURE
    }
  }
}

fn approve(tag: &str) -> Result<(), DialogError> {
  let wearing = Wearing::worn()?;
  let mut feedback = "";
  loop {
    let Some(secret) = prompt(tag, &wearing, feedback)? else {
      return Ok(());
    };
    let result = submit(tag, &secret)?;
    if result.status.success() {
      notice("Console update installed", tag);
      return Ok(());
    }
    let stderr = String::from_utf8_lossy(&result.stderr);
    let message = stderr
      .trim()
      .lines()
      .last()
      .unwrap_or("The installer stopped.");
    let message = message.strip_prefix("console-update: ").unwrap_or(message);
    match message {
      "the update pattern did not match" => feedback = "Pattern did not match. Try again.",
      "wait a few seconds before trying the update pattern again" => {
        feedback = "Wait a few seconds, then try again.";
      }
      _ => return Err(message.to_owned().into()),
    }
  }
}

fn submit(tag: &str, secret: &Secret) -> Result<Output, DialogError> {
  let mut child = Command::new("/usr/bin/sudo")
    .args([
      "-n",
      "--",
      "/usr/local/bin/console-update",
      "--install-with-pattern",
    ])
    .stdin(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
  let Ok(letters) = secret.as_str();
  let mut pipe = child
    .stdin
    .take()
    .ok_or("the installer did not open an input pipe")?;
  let sent = writeln!(pipe, "{tag}\n{letters}");
  drop(pipe);
  let result = child.wait_with_output()?;
  if result.status.success() {
    sent?;
  }
  Ok(result)
}

fn notice(title: &str, message: &str) {
  let _status = Command::new("notify-send").args([title, message]).status();
}

fn prompt(tag: &str, wearing: &Wearing, feedback: &'static str) -> Result<Option<Secret>, DialogError> {
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
  let mut dialog = Dialog::new(feedback);
  let mut rendered = Rendered::default();

  loop {
    let Ok(closed) = surface.closed();
    if closed == Closed::Yes {
      return Ok(None);
    }
    let Ok(logical) = surface.logical();
    if let Some(canvas) = logical {
      let layout = Layout::centred(canvas);
      let Ok(keys) = surface.keyboard_events();
      let Ok(pointers) = surface.pointer_events();
      let keys = keys.into_iter().filter_map(|KeyboardEvent::Down { key }| {
        let Ok(button) = press(key);
        button.map(Input::Pad)
      });
      for input in keys.chain(
        pointers
          .into_iter()
          .filter_map(|event| layout.pointer(event)),
      ) {
        match dialog.handle(input) {
          Choice::Continue => {}
          Choice::Close => return Ok(None),
          Choice::Install(secret) => return Ok(Some(secret)),
        }
      }
      let Ok(shapes) = rendered.wanted(layout.paint(&dialog, tag, wearing));
      if let Some(shapes) = shapes {
        let Ok(()) = surface.resize(canvas);
        let Ok(paint) = painter(canvas, shapes, WHO);
        surface.draw(paint)?;
      }
    }
    surface.wait(&[], None)?;
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Input {
  Pad(ButtonPress),
  Touch(Touch),
  Close,
}

impl Dialog {
  fn new(feedback: &'static str) -> Self {
    let Ok(pattern) = Pattern::new();
    Self {
      pattern,
      close_focused: false,
      feedback,
    }
  }

  fn handle(&mut self, input: Input) -> Choice {
    let clicked = match input {
      Input::Close => return Choice::Close,
      Input::Touch(touch) => {
        self.close_focused = false;
        let Ok(clicked) = self.pattern.touch(touch);
        clicked
      }
      Input::Pad(ButtonPress::Choose) if self.close_focused => return Choice::Close,
      Input::Pad(ButtonPress::Up) if self.pattern.at == Target::Dot(0) => {
        self.close_focused = true;
        return Choice::Continue;
      }
      Input::Pad(_) if self.close_focused => {
        self.close_focused = false;
        return Choice::Continue;
      }
      Input::Pad(ButtonPress::Choose) => {
        let Ok(clicked) = self.pattern.click();
        clicked
      }
      Input::Pad(ButtonPress::Back) => {
        let Ok(pattern) = self.pattern.undone();
        self.pattern = pattern;
        return Choice::Continue;
      }
      Input::Pad(button) => {
        let direction = match button {
          ButtonPress::Up => Direction::Up,
          ButtonPress::Down => Direction::Down,
          ButtonPress::Left => Direction::Left,
          ButtonPress::Right => Direction::Right,
          ButtonPress::Back | ButtonPress::Choose => return Choice::Continue,
        };
        let Ok(pattern) = self.pattern.moved(direction);
        self.pattern = pattern;
        return Choice::Continue;
      }
    };
    match clicked {
      Clicked::Traced(pattern) => self.pattern = pattern,
      Clicked::Submitted(secret) if self.pattern.path.len() >= 4 => {
        return Choice::Install(secret);
      }
      Clicked::Submitted(_) => {
        let Ok(pattern) = self.pattern.cleared();
        self.pattern = pattern;
        self.feedback = "Join at least four dots.";
      }
    }
    Choice::Continue
  }

  fn focused(&self, target: Target) -> bool {
    !self.close_focused && self.pattern.at == target
  }
}

impl Layout {
  fn centred(canvas: Size<u32>) -> Self {
    let (wide, tall) = (f64::from(canvas.width), f64::from(canvas.height));
    let scale = ((wide - 16.0) / f64::from(WIDTH))
      .min((tall - 16.0) / f64::from(HEIGHT))
      .clamp(0.01, 1.0);
    let origin = Point {
      x: (wide - f64::from(WIDTH) * scale) / 2.0,
      y: (tall - f64::from(HEIGHT) * scale) / 2.0,
    };
    Self { origin, scale }
  }

  fn pointer(&self, event: PointerEvent) -> Option<Input> {
    let on_card = |(x, y)| {
      let Ok(x) = toward_zero_i32((x - self.origin.x) / self.scale);
      let Ok(y) = toward_zero_i32((y - self.origin.y) / self.scale);
      Point { x, y }
    };
    let in_ring = |at: Point<i32>| Point {
      x: at.x.saturating_sub(RING.x),
      y: at.y.saturating_sub(RING.y),
    };
    match event {
      PointerEvent::Down { at } => {
        let at = on_card(at);
        if CLOSE.contains(at) == Ok(Containment::Inside) {
          return Some(Input::Close);
        }
        (CARD.contains(at) == Ok(Containment::Inside))
          .then_some(Input::Touch(Touch::Down(in_ring(at))))
      }
      PointerEvent::Moved { at } => Some(Input::Touch(Touch::Moved(in_ring(on_card(at))))),
      PointerEvent::Up => Some(Input::Touch(Touch::Up)),
      PointerEvent::Left | PointerEvent::Pinched { .. } | PointerEvent::Scrolled { .. } => None,
    }
  }

  fn point(&self, at: Point<i32>) -> Point<i32> {
    let Ok(x) = toward_zero_i32(self.origin.x + f64::from(at.x) * self.scale);
    let Ok(y) = toward_zero_i32(self.origin.y + f64::from(at.y) * self.scale);
    Point { x, y }
  }

  fn length(&self, length: u32) -> u32 {
    let Ok(length) = whole_u32(f64::from(length) * self.scale);
    length.max(1)
  }

  fn panel(&self, mut panel: Panel) -> Shape {
    panel.at = self.point(panel.at);
    panel.size = Size {
      width: self.length(panel.size.width),
      height: self.length(panel.size.height),
    };
    panel.round = Round(self.length(panel.round.0));
    if let Edge::Of { wide, color } = panel.edge {
      panel.edge = Edge::Of {
        wide: self.length(wide),
        color,
      };
    }
    Shape::Panel(panel)
  }

  fn text(&self, mut text: Text) -> Shape {
    text.at = self.point(text.at);
    text.width = self.length(text.width);
    text.font.height = self.length(text.font.height);
    Shape::Text(text)
  }

  fn line(&self, from: Point<i32>, to: Point<i32>, color: Oklch) -> Shape {
    Shape::Line(Line {
      from: self.point(from),
      to: self.point(to),
      width: self.length(4),
      color,
    })
  }

  fn paint(&self, dialog: &Dialog, tag: &str, palette: &Wearing) -> Vec<Shape> {
    let mut shapes = vec![
      self.panel(Panel {
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
      }),
      self.text(Text {
        at: Point { x: 26, y: 19 },
        width: 398,
        said: "Install Console update".into(),
        weight: Weight::Bold,
        font: font(26),
        ink: palette.text,
      }),
      self.text(Text {
        at: Point { x: 26, y: 52 },
        width: 398,
        said: tag.to_owned(),
        weight: Weight::Plain,
        font: font(18),
        ink: palette.soft,
      }),
    ];

    let button = shifted(LOGIN, RING);
    shapes.extend(self.button(
      button,
      LOGIN_SIZE,
      "Install",
      palette,
      dialog.focused(Target::Login),
    ));
    let close = Point {
      x: CLOSE.origin.x + 26,
      y: CLOSE.origin.y + 26,
    };
    shapes.extend(self.button(
      close,
      Size {
        width: 52,
        height: 52,
      },
      "×",
      palette,
      dialog.close_focused,
    ));

    let mut dots = dialog.pattern.path.iter().filter_map(|number| {
      let Ok(dot) = dot(*number);
      dot.map(|dot| shifted(dot.centre, RING))
    });
    if let Some(mut from) = dots.next() {
      for to in dots {
        shapes.push(self.line(from, to, palette.coral));
        from = to;
      }
      if let Some(finger) = dialog.pattern.finger {
        shapes.push(self.line(from, shifted(finger, RING), palette.coral));
      }
    }
    for (number, dot) in (0..).zip(DOTS) {
      let centre = shifted(dot.centre, RING);
      let fill = if dialog.pattern.path.contains(&number) {
        palette.coral
      } else {
        palette.ground
      };
      shapes.push(self.panel(Panel {
        at: Point {
          x: centre.x - 22,
          y: centre.y - 22,
        },
        size: dot.size,
        round: Round(22),
        fill,
        edge: outline(palette, dialog.focused(Target::Dot(number))),
      }));
    }

    if !dialog.feedback.is_empty() {
      shapes.push(self.text(Text {
        at: Point { x: 26, y: 519 },
        width: 468,
        said: dialog.feedback.to_owned(),
        weight: Weight::Plain,
        font: font(17),
        ink: palette.text,
      }));
    }
    shapes
  }

  fn button(
    &self,
    centre: Point<i32>,
    size: Size<u32>,
    label: &str,
    palette: &Wearing,
    focused: bool,
  ) -> [Shape; 2] {
    use console_core_number_conversion::fitted;
    let Ok(half_wide) = fitted::<_, i32>(size.width / 2);
    let Ok(half_tall) = fitted::<_, i32>(size.height / 2);
    let font = font(23);
    let Ok(ink) = measure_text(
      Run {
        said: label,
        weight: Weight::Bold,
        width: size.width,
      },
      &font,
    );
    let Ok(ink_wide) = fitted::<_, i32>(ink.width / 2);
    let Ok(ink_tall) = fitted::<_, i32>(ink.height / 2);
    [
      self.panel(Panel {
        at: Point {
          x: centre.x - half_wide,
          y: centre.y - half_tall,
        },
        size,
        round: Round(14),
        fill: palette.fill,
        edge: outline(palette, focused),
      }),
      self.text(Text {
        at: Point {
          x: centre.x - ink_wide,
          y: centre.y - ink_tall,
        },
        width: size.width,
        said: label.to_owned(),
        weight: Weight::Bold,
        font,
        ink: palette.text,
      }),
    ]
  }
}

fn outline(palette: &Wearing, focused: bool) -> Edge {
  if focused {
    Edge::Of {
      wide: 4,
      color: palette.text,
    }
  } else {
    Edge::Of {
      wide: 1,
      color: palette.edge,
    }
  }
}

fn shifted(point: Point<i32>, by: Point<i32>) -> Point<i32> {
  Point {
    x: point.x.saturating_add(by.x),
    y: point.y.saturating_add(by.y),
  }
}

fn font(height: u32) -> Font {
  Font {
    family: "Noto Sans".to_owned(),
    height,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_scaled_dialog_closes_only_at_the_close_button() -> Result<(), DialogError> {
    let layout = Layout::centred(Size {
      width: 400,
      height: 420,
    });
    let tap = |point| {
      let at = layout.point(point);
      layout.pointer(PointerEvent::Down {
        at: (f64::from(at.x), f64::from(at.y)),
      })
    };
    let mut dialog = Dialog::new("");
    let blank = tap(Point { x: 260, y: 240 }).ok_or("blank press is inside the dialog")?;

    assert_eq!(tap(Point { x: -6, y: 78 }), None);
    assert!(matches!(dialog.handle(blank), Choice::Continue));
    assert!(matches!(
      dialog.handle(Input::Touch(Touch::Up)),
      Choice::Continue
    ));
    assert!(matches!(
      dialog.handle(Input::Pad(ButtonPress::Back)),
      Choice::Continue
    ));
    assert_eq!(tap(Point { x: 478, y: 38 }), Some(Input::Close));
    assert!(matches!(dialog.handle(Input::Close), Choice::Close));
    Ok(())
  }

  #[test]
  fn the_pad_can_focus_close_or_submit_the_pattern() {
    let mut dialog = Dialog::new("");
    assert!(matches!(
      dialog.handle(Input::Pad(ButtonPress::Up)),
      Choice::Continue
    ));
    assert!(matches!(
      dialog.handle(Input::Pad(ButtonPress::Choose)),
      Choice::Close
    ));

    let mut dialog = Dialog::new("");
    dialog.pattern.at = Target::Login;
    dialog.pattern.path = vec![0, 1, 2, 3];
    assert!(matches!(
      dialog.handle(Input::Pad(ButtonPress::Choose)),
      Choice::Install(_)
    ));
  }
}
