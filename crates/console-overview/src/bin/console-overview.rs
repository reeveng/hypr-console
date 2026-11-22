use std::collections::BTreeMap;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_compositor::{ActiveWorkspace, Clients, DispatchResult, Monitors, Request};
use console_core_arguments::{Operands, Presence, read};
use console_core_color::palette::Wearing;
use console_core_fonts::TextStyle;
use console_core_geometry::{Point, Size};
use console_core_internal_programs::OVERVIEW_SHOW;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_shapes::Pixels;
use console_draw_painting::{self as painting, Frame, Run};
use console_draw_surface::{
  Activation, Anchor, Closed, Keyboard, KeyboardEvent, Keysym, Margin, PointerEvent, Room,
  Surface, SurfaceError, Under, Wanted,
};
use console_onscreen::{OVERVIEW, OVERVIEW_EDGE};
use console_overview::{
  Choosing, Chose, Dragged, EDGE, Hit, HorizontalPixels, Key, Measure, Overview, PlaceId, Scene,
  Stroke, Swipe, Wandered, activate, carried, chose, chosen_window, dropped, held_by_the_pad,
  hit, overview, places, render, resized,
};
use console_response_times::{Wait, Waiting};

struct TextMetrics;

impl Measure for TextMetrics {
  fn measure(&self, said: &str, style: TextStyle, width: u32) -> Result<Size<u32>, Never> {
    let Ok(font) = style.font();
    let Ok(weight) = style.weight();

    painting::measure_text(
      Run {
        said,
        weight,
        width,
      },
      &font,
    )
  }
}

enum Showing {
  Edge {
    from: Option<Point<f64>>,
  },
  Open {
    laid: Box<Overview>,
    front: Option<PlaceId>,
    touch: Touch,
    choosing: Box<Choosing>,
    pictures: BTreeMap<String, Pixels>,
  },
}

const PICTURE_WIDEST: u32 = 480;

enum Touch {
  Lifted,
  Pressed { stroke: Stroke, on: Hit },
  Dragging(Dragged, PlaceId),
  Resizing { left: String, last: Point<f64> },
}

enum Next {
  Stay,
  Open,
  PutAway,
  Redraw,
  Ask(String),
  AskAndPutAway(String),
}

const SHOW: &str = "show";

const DATAGRAMS_PER_TURN: u32 = 32;

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "console-overview",
  about: "every workspace and its windows at once, waiting at the screen's edge until it is opened",
  flags: &[OVERVIEW_SHOW],
  operands: Operands::None,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
  Show,
  None,
}

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  let line = match read(&COMMAND, &words) {
    Ok(line) => line,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };
  let Ok(showing) = line.presence(OVERVIEW_SHOW);

  match showing {
    Presence::Present => {
      let Ok(shown) = show();

      return shown;
    }
    Presence::Absent => {}
  }

  let wearing = match Wearing::worn() {
    Ok(wearing) => wearing,
    Err(why) => {
      eprintln!("console-overview: no palette: {why}");

      return ExitCode::FAILURE;
    }
  };

  let mut surface = match Surface::connect() {
    Ok(surface) => surface,
    Err(why) => {
      eprintln!("console-overview: {why}");

      return ExitCode::FAILURE;
    }
  };

  match run(&mut surface, &wearing) {
    Ok(()) => ExitCode::SUCCESS,
    Err(why) => {
      eprintln!("console-overview: {why}");

      ExitCode::FAILURE
    }
  }
}

#[must_use]
fn show() -> Result<ExitCode, Never> {
  let Ok(socket_path) = socket();

  let at = match socket_path {
    Some(path) => path,
    None => {
      eprintln!("console-overview: there is no session or no compositor to show the overview in");

      return Ok(ExitCode::FAILURE);
    }
  };

  let sent = UnixDatagram::unbound()
    .and_then(|sending| sending.send_to(SHOW.as_bytes(), &at))
    .map(drop);

  Ok(match sent {
    Ok(()) => ExitCode::SUCCESS,
    Err(why) => {
      eprintln!("console-overview: {}: {why}", at.display());

      ExitCode::FAILURE
    }
  })
}

struct SocketName(String);

impl SocketName {
  fn for_instance(instance: &str) -> Result<Option<Self>, Never> {
    Ok(
      match ::core::primitive::bool::then_some(instance.is_empty() || instance.contains(['/', '\0']), ()) {
        ::core::option::Option::Some(()) => None,
        ::core::option::Option::None => Some(Self(format!("{OVERVIEW}-{instance}.sock"))),
      },
    )
  }
}

impl AsRef<Path> for SocketName {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

fn socket() -> Result<Option<PathBuf>, Never> {
  let Ok(runtime) = console_core_places::application_runtime();
  let Ok(signature) = console_compositor::instance();

  Ok(runtime.zip(signature).and_then(|(folder, instance)| {
    let Ok(name) = SocketName::for_instance(&instance);

    name.map(|name_2| folder.join(name_2))
  }))
}

fn listen() -> Result<Option<UnixDatagram>, Never> {
  let Ok(socket_path) = socket();

  let at = match socket_path {
    Some(path) => path,
    None => return Ok(None),
  };

  match std::fs::remove_file(&at) {
    Ok(()) => {}
    Err(fault) => match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        eprintln!(
          "console-overview: could not prepare {}: {fault}",
          at.display()
        );

        return Ok(None);
      }
    },
  }

  let bound = UnixDatagram::bind(&at).and_then(|told| {
    told.set_nonblocking(true)?;

    Ok(told)
  });

  Ok(match bound {
    Ok(told) => Some(told),
    Err(why) => {
      eprintln!(
        "console-overview: {}: {why}; the pad cannot open the overview",
        at.display()
      );

      None
    }
  })
}

#[cfg_attr(
  dylint_lib = "explicit016_no_wildcard_arm",
  allow(
    explicit016_no_wildcard_arm,
    reason = "`std::io::ErrorKind` is the standard library's and cannot be listed exhaustively, so the fault this desktop has no answer for is the arm that remains"
  )
)]
fn heard(told: Option<&UnixDatagram>) -> Result<Command, Never> {
  let told_3 = match told {
    Some(told_2) => told_2,
    None => return Ok(Command::None),
  };

  let mut buffer = [0_u8; 16];
  let mut asked = Command::None;

  for _message in 0..DATAGRAMS_PER_TURN {
    match told_3.recv(&mut buffer) {
      Ok(got) => match ::core::primitive::bool::then_some(buffer.get(..got) == Some(SHOW.as_bytes()), ()) {
        ::core::option::Option::Some(()) => asked = Command::Show,
        ::core::option::Option::None => {}
      },
      Err(fault) => match fault.kind() {
        std::io::ErrorKind::WouldBlock => break,
        std::io::ErrorKind::Interrupted => continue,
        _ => {
          eprintln!("console-overview: the show socket could not be read: {fault}");

          break;
        }
      },
    }
  }

  Ok(asked)
}

fn key_of(key: Keysym) -> Result<Option<Key>, Never> {
  Ok(match key {
    Keysym::Left => Some(Key::Left),
    Keysym::Right => Some(Key::Right),
    Keysym::Up => Some(Key::Up),
    Keysym::Down => Some(Key::Down),
    Keysym::Return | Keysym::KP_Enter => Some(Key::Choose),
    Keysym::F18 => Some(Key::More),
    Keysym::Escape => Some(Key::Back),
    _any_other_key => None,
  })
}

fn run(surface: &mut Surface, wearing: &Wearing) -> Result<(), SurfaceError> {
  let Ok(told) = listen();
  let showing = edge(surface)?;

  let ran = console_core_iteration::iterate((surface, showing), |(surface_2, showing_2)| {
    turned(surface_2, wearing, told.as_ref(), showing_2)
  });

  match ran {
    Ok(ended) => ended,
    Err(_endless) => Err(SurfaceError::Hung),
  }
}

type Turned<'a> = Step<(&'a mut Surface, Showing), Result<(), SurfaceError>>;

fn turned<'a>(
  surface: &'a mut Surface,
  wearing: &Wearing,
  told: Option<&UnixDatagram>,
  mut showing: Showing,
) -> Result<Turned<'a>, Never> {
  let waiting: Vec<BorrowedFd<'_>> = told.iter().map(|told_2| told_2.as_fd()).collect();

  match surface.wait(&waiting, None) {
    Ok(()) => {}
    Err(fault) => return Ok(Step::Halt(Err(fault))),
  }

  match surface.closed() {
    Ok(Closed::Yes) => return Ok(Step::Halt(Err(SurfaceError::Hung))),
    Ok(Closed::No) => {}
  }

  let Ok(asked) = heard(told);
  let Ok(pointer) = surface.pointer_events();
  let Ok(keys) = surface.keyboard_events();
  let shown = match (asked, &showing) {
    (Command::Show, Showing::Edge { .. }) => Some(Next::Open),
    (Command::Show, Showing::Open { .. }) | (Command::None, _) => None,
  };
  let pressed = keys.into_iter().filter_map(|KeyboardEvent::Down { key }| {
    let Ok(pressed) = key_of(key);

    pressed
  });

  let Ok(nexts) = stepped(&mut showing, shown, pointer, pressed);

  for next in nexts {
    showing = match carry_out(surface, wearing, showing, next) {
      Ok(after) => after,
      Err(fault) => return Ok(Step::Halt(Err(fault))),
    };
  }

  Ok(Step::Again((surface, showing)))
}

fn stepped(
  showing: &mut Showing,
  shown: Option<Next>,
  pointer: Vec<PointerEvent>,
  pressed: impl Iterator<Item = Key>,
) -> Result<Vec<Next>, Never> {
  let mut nexts: Vec<Next> = shown.into_iter().collect();

  for event in pointer {
    let Ok(next) = step(showing, event);

    nexts.push(next);
  }

  for key in pressed {
    let Ok(next) = step_key(showing, key);

    nexts.push(next);
  }

  let (redraws, mut once): (Vec<Next>, Vec<Next>) = nexts
    .into_iter()
    .partition(|next| matches!(next, Next::Redraw));

  match ::core::primitive::bool::then_some(redraws.is_empty(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => once.push(Next::Redraw),
  }

  Ok(once)
}

fn step_key(showing: &mut Showing, key: Key) -> Result<Next, Never> {
  let (laid, choosing) = match showing {
    Showing::Open { laid, choosing, .. } => (laid, choosing),
    Showing::Edge { .. } => return Ok(Next::Stay),
  };

  let Ok((after, did)) = chose(laid, choosing, key);

  **choosing = after;

  Ok(match did {
    Chose::Stay => Next::Redraw,
    Chose::Ask(lua) => Next::Ask(lua),
    Chose::AskAndPutAway(lua) => Next::AskAndPutAway(lua),
    Chose::PutAway => Next::PutAway,
  })
}

fn carry_out(
  surface: &mut Surface,
  wearing: &Wearing,
  showing: Showing,
  next: Next,
) -> Result<Showing, SurfaceError> {
  match next {
    Next::Stay => Ok(showing),
    Next::Open => open(surface, wearing),
    Next::PutAway => edge(surface),
    Next::Redraw => {
      draw(surface, wearing, &showing)?;

      Ok(showing)
    }
    Next::Ask(lua) => {
      let Ok(()) = ask(&lua);

      refreshed(surface, wearing, showing)
    }
    Next::AskAndPutAway(lua) => {
      let put_away = edge(surface)?;
      let Ok(()) = ask(&lua);

      Ok(put_away)
    }
  }
}

fn refreshed(
  surface: &mut Surface,
  wearing: &Wearing,
  showing: Showing,
) -> Result<Showing, SurfaceError> {
  let room = match surface.logical() {
    Ok(Some(room)) => room,
    Ok(None) => return Ok(showing),
  };

  let Ok(again) = looked(room);

  let showing_2 = match (showing, again) {
    (
      Showing::Open {
        touch,
        choosing,
        pictures,
        ..
      },
      Some((laid, front)),
    ) => Showing::Open {
      laid: Box::new(laid),
      front,
      touch,
      choosing,
      pictures,
    },
    (open @ Showing::Open { .. }, None) => open,
    (edge @ Showing::Edge { .. }, Some(_) | None) => edge,
  };

  draw(surface, wearing, &showing_2)?;

  Ok(showing_2)
}

fn step(showing: &mut Showing, event: PointerEvent) -> Result<Next, Never> {
  match showing {
    Showing::Edge { from } => step_edge(from, event),
    Showing::Open { laid, touch, .. } => step_open(laid, touch, event),
  }
}

fn step_edge(from: &mut Option<Point<f64>>, event: PointerEvent) -> Result<Next, Never> {
  Ok(match (&from, event) {
    (_, PointerEvent::Down { at }) => {
      *from = Some(Point { x: at.0, y: at.1 });

      Next::Stay
    }
    (Some(start), PointerEvent::Moved { at }) => {
      let Ok(swipe) = (Stroke {
        from: *start,
        at: Point { x: at.0, y: at.1 },
      })
      .swipe();

      match swipe {
        Swipe::Up => {
          *from = None;

          Next::Open
        }
        Swipe::Down | Swipe::Neither => Next::Stay,
      }
    }
    (_, PointerEvent::Up | PointerEvent::Left) => {
      *from = None;

      Next::Stay
    }
    (None, PointerEvent::Moved { .. })
    | (_, PointerEvent::Scrolled { .. } | PointerEvent::Pinched { .. }) => Next::Stay,
  })
}

fn step_open(laid: &Overview, touch: &mut Touch, event: PointerEvent) -> Result<Next, Never> {
  let lifted = std::mem::replace(touch, Touch::Lifted);

  let (after, next) = match (lifted, event) {
    (Touch::Lifted, PointerEvent::Down { at }) => {
      let at_2 = Point { x: at.0, y: at.1 };
      let Ok(on) = hit(laid, at_2);

      (
        Touch::Pressed {
          stroke: Stroke {
            from: at_2,
            at: at_2,
          },
          on,
        },
        Next::Stay,
      )
    }
    (Touch::Pressed { stroke, on }, PointerEvent::Moved { at }) => {
      let Ok(moved) = pressed_moved(laid, stroke, on, Point { x: at.0, y: at.1 });

      moved
    }
    (Touch::Pressed { on, .. }, PointerEvent::Up) => {
      let Ok(went) = tapped(&on);

      (Touch::Lifted, went)
    }
    (Touch::Dragging(mut held, from), PointerEvent::Moved { at }) => {
      held.stroke.at = Point { x: at.0, y: at.1 };

      (Touch::Dragging(held, from), Next::Redraw)
    }
    (Touch::Dragging(held, from), PointerEvent::Up | PointerEvent::Left) => {
      let Ok(landed) = dropped(laid, from, held.stroke.at);
      let Ok(asked) = carried(&held.address, landed);

      (Touch::Lifted, asked.map_or(Next::Redraw, Next::Ask))
    }
    (Touch::Resizing { left, last }, PointerEvent::Moved { at }) => {
      let at_3 = Point { x: at.0, y: at.1 };
      let Ok(lua) = resized(laid, &left, HorizontalPixels(at_3.x - last.x));

      (Touch::Resizing { left, last: at_3 }, Next::Ask(lua))
    }
    (
      Touch::Lifted | Touch::Pressed { .. } | Touch::Resizing { .. },
      PointerEvent::Up | PointerEvent::Left,
    ) => (Touch::Lifted, Next::Stay),
    (
      still,
      PointerEvent::Down { .. }
      | PointerEvent::Moved { .. }
      | PointerEvent::Scrolled { .. }
      | PointerEvent::Pinched { .. },
    ) => (still, Next::Stay),
  };

  *touch = after;

  Ok(next)
}

fn pressed_moved(
  laid: &Overview,
  stroke: Stroke,
  on: Hit,
  at: Point<f64>,
) -> Result<(Touch, Next), Never> {
  let stroke_2 = Stroke {
    from: stroke.from,
    at,
  };
  let Ok(wandered) = stroke_2.wandered();
  let Ok(swipe) = stroke_2.swipe();

  Ok(match (wandered, on) {
    (Wandered::No, on_2) => (
      Touch::Pressed {
        stroke: stroke_2,
        on: on_2,
      },
      Next::Stay,
    ),
    (
      Wandered::Yes,
      Hit::Tile {
        address,
        from,
        frame,
      },
    ) => (
      Touch::Dragging(
        Dragged {
          address,
          frame,
          stroke: stroke_2,
        },
        from,
      ),
      Next::Redraw,
    ),
    (Wandered::Yes, Hit::Divider { left }) => {
      let Ok(lua) = resized(laid, &left, HorizontalPixels(at.x - stroke_2.from.x));

      (Touch::Resizing { left, last: at }, Next::Ask(lua))
    }
    (Wandered::Yes, Hit::Card(_) | Hit::Close | Hit::None) => match swipe {
      Swipe::Down => (Touch::Lifted, Next::PutAway),
      Swipe::Up | Swipe::Neither => (Touch::Lifted, Next::Stay),
    },
  })
}

fn tapped(on: &Hit) -> Result<Next, Never> {
  let Ok(went) = activate(Activation::At(on), None);

  Ok(match went {
    Chose::Stay => Next::Stay,
    Chose::Ask(lua) => Next::Ask(lua),
    Chose::AskAndPutAway(lua) => Next::AskAndPutAway(lua),
    Chose::PutAway => Next::PutAway,
  })
}

fn looked(room: Size<u32>) -> Result<Option<(Overview, Option<PlaceId>)>, Never> {
  let asked = console_compositor::ask(Clients).and_then(|open| {
    let screens = console_compositor::ask(Monitors)?;
    let front = console_compositor::ask(ActiveWorkspace)?;

    Ok((open, screens, front))
  });

  let (open, screens, front) = match asked {
    Ok(all) => all,
    Err(why) => {
      eprintln!("console-overview: {why}");

      return Ok(None);
    }
  };

  let screen = screens.iter().find_map(|screen| {
    let Ok(logical) = screen.logical();

    logical
  });

  let Ok(grouped) = places(open);
  let Ok(laid) = overview(
    &grouped,
    match screen {
      Some(screen_2) => screen_2,
      None => room,
    },
  );

  Ok(Some((laid, front.map(|workspace| PlaceId(workspace.id)))))
}

fn ask(lua: &str) -> Result<(), Never> {
  let Ok(answered) = console_compositor::request(Request::Script, lua);

  match answered {
    DispatchResult::Success => {}
    DispatchResult::Failure(why) => eprintln!("console-overview: {lua}: {why}"),
  }

  Ok(())
}

fn edge(surface: &mut Surface) -> Result<Showing, SurfaceError> {
  let Ok(()) = surface.hide();

  surface.show(&Wanted {
    namespace: OVERVIEW_EDGE.to_string(),
    anchor: Anchor::Bottom,
    size: Size {
      width: 0,
      height: EDGE,
    },
    margin: Margin::default(),
    keyboard: Keyboard::Declines,
    room: Room::Over,
    under: Under::Anything,
  })?;

  paint(surface, &[])?;

  Ok(Showing::Edge { from: None })
}

fn open(surface: &mut Surface, wearing: &Wearing) -> Result<Showing, SurfaceError> {
  let Ok(mut waiting) = Waiting::here(Wait {
    who: OVERVIEW,
    what: "opening",
  });
  let Ok(()) = surface.hide();

  surface.show(&Wanted {
    namespace: OVERVIEW.to_string(),
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

  let Ok(()) = waiting.mark("surface");

  let room = match surface.logical() {
    Ok(Some(room)) => room,
    Ok(None) => return edge(surface),
  };

  let Ok(seen) = looked(room);

  let (laid, front) = match seen {
    Some(seen_2) => seen_2,
    None => return edge(surface),
  };
  let Ok(()) = waiting.mark("compositor");
  let windows = laid
    .cards
    .iter()
    .flat_map(|card| card.tiles.iter())
    .fold(0_u64, |many, _| many.saturating_add(1));
  let Ok(()) = waiting.counted("windows", windows);
  let mut showing = Showing::Open {
    laid: Box::new(laid),
    front,
    touch: Touch::Lifted,
    choosing: Box::new(Choosing::Untouched),
    pictures: BTreeMap::new(),
  };

  draw(surface, wearing, &showing)?;

  let Ok(()) = waiting.mark("drawn");

  let taken = match console_overview::pictures::take(PICTURE_WIDEST) {
    Ok(taken) => taken,
    Err(why) => {
      eprintln!("console-overview: {why}");

      let Ok(()) = waiting.finish();

      return Ok(showing);
    }
  };

  let Ok(()) = waiting.mark("pictures");

  match &mut showing {
    Showing::Open { pictures, .. } => {
      pictures.extend(taken.into_iter().map(|one| (one.address, one.pixels)))
    }
    Showing::Edge { .. } => {}
  }

  draw(surface, wearing, &showing)?;

  let Ok(()) = waiting.mark("pictured");
  let Ok(()) = waiting.finish();

  Ok(showing)
}

fn draw(surface: &mut Surface, wearing: &Wearing, showing: &Showing) -> Result<(), SurfaceError> {
  let shapes = match showing {
    Showing::Edge { .. } => Vec::new(),
    Showing::Open {
      laid,
      front,
      touch,
      choosing,
      pictures,
    } => {
      let Ok(by_the_pad) = held_by_the_pad(laid, choosing);
      let Ok(chosen) = chosen_window(laid, choosing);
      let (held, landing) = match touch {
        Touch::Dragging(held, _) => {
          let Ok(landing) = console_overview::landing(laid, held);

          (Some(held), landing)
        }
        Touch::Lifted | Touch::Pressed { .. } | Touch::Resizing { .. } => {
          (by_the_pad.as_ref(), None)
        }
      };
      let drawn = Scene {
        overview: laid,
        front: *front,
        held,
        landing,
        chosen: chosen.as_deref(),
        pictures,
        wearing,
      };
      let Ok(shapes) = render(&drawn, &TextMetrics);

      shapes
    }
  };

  paint(surface, &shapes)
}

fn paint(
  surface: &mut Surface,
  shapes: &[console_core_shapes::Shape],
) -> Result<(), SurfaceError> {
  let points = match surface.logical() {
    Ok(Some(points)) => points,
    Ok(None) => return Ok(()),
  };

  surface.draw(|pixels, device, _scale| {
    match painting::onto(pixels, Frame { device, points }, shapes) {
      Ok(()) => {}
      Err(why) => eprintln!("console-overview: {why}"),
    }

    Ok(())
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn one_swipe_up_opens_the_overview_once_however_many_moves_it_arrives_in() {
    let mut showing = Showing::Edge { from: None };
    let pointer = vec![
      PointerEvent::Down { at: (500.0, 790.0) },
      PointerEvent::Moved { at: (500.0, 600.0) },
      PointerEvent::Moved { at: (500.0, 500.0) },
      PointerEvent::Moved { at: (500.0, 400.0) },
    ];
    let Ok(nexts) = stepped(&mut showing, None, pointer, std::iter::empty());
    let opened: Vec<&str> = nexts
      .iter()
      .filter(|next| matches!(next, Next::Open))
      .map(|_| "open")
      .collect();

    assert_eq!(
      opened,
      ["open"],
      "every open hides the overview and shows it again, so a second one is a flicker"
    );
  }

  #[test]
  fn moves_that_arrive_together_are_drawn_once() {
    let Ok(laid) = console_overview::overview(
      &[],
      Size {
        width: 1000,
        height: 1600,
      },
    );
    let mut showing = Showing::Open {
      laid: Box::new(laid),
      front: None,
      touch: Touch::Lifted,
      choosing: Box::new(Choosing::Untouched),
      pictures: BTreeMap::new(),
    };
    let keys = [Key::Right, Key::Left, Key::Right, Key::Left];
    let Ok(after) = stepped(&mut showing, None, Vec::new(), keys.into_iter());
    let drawn: Vec<&str> = after
      .iter()
      .filter(|next| matches!(next, Next::Redraw))
      .map(|_| "drawn")
      .collect();

    assert_eq!(
      drawn,
      ["drawn"],
      "every redraw paints the same last state, and a paint slower than the moves arriving falls further behind with each one"
    );
  }
}
