use std::os::fd::AsFd;
use std::os::unix::net::UnixDatagram;
use std::process::{ExitCode, Stdio};
use std::sync::mpsc::{SyncSender, TrySendError};

#[path = "console-control-center/capture.rs"]
mod capture;

use console_control_center::selection::{
  self, CANCEL, CORNER_CONTROLS, Corners, Drag, Prompt, Purpose,
};
use console_control_center::{
  CAPTURE_TEXT, CLOSE, EDGE, Labels, SCREENSHOT, SCREENSHOT_AREA, Sheet, Swipe, Tapped,
  classify_swipe, hit_test, render, sheet,
};
use console_core_color::palette::Wearing;
use console_core_fonts::TextStyle;
use console_core_geometry::{Point, Rectangle, Size};
use console_core_internal_programs::{InternalProgram, SELECT_SCREENSHOT_AREA};
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_shapes::{Covers, Edge, Shape, Weight};
use console_draw_painting::{self as painting, Run};
use console_draw_surface::{
  Activation, Anchor, Closed, Keyboard, KeyboardEvent, Keysym, Margin, PointerEvent, Room,
  Surface, SurfaceError, Under, Wanted,
};
use console_onscreen::{CONTROL_CENTER, CONTROL_CENTER_PANEL, focus};

enum Showing {
  Edge {
    from: Option<(f64, f64)>,
  },
  Sheet {
    sheet: Box<Sheet>,
    at: Option<(f64, f64)>,
    focused: Tapped,
  },
  Selecting {
    purpose: Purpose,
    prompt: Prompt,
    selection: Selection,
  },
}

#[derive(Clone, Copy)]
enum Selection {
  Ready,
  Dragging(Drag),
  Corners(Corners),
}

#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
enum Next {
  Stay,
  Open,
  PutAway,
  TakeScreenshot,
  Select(Purpose),
  Capture(Purpose, Rectangle<i32>),
  Close,
  Repaint,
}

fn main() -> ExitCode {
  if std::env::args()
    .skip(1)
    .eq([SELECT_SCREENSHOT_AREA.spelling])
  {
    return match focus::Layer::ScreenSelection.take() {
      Ok(()) => ExitCode::SUCCESS,
      Err(fault) => {
        console_core_logging::error!("opening screenshot area: {fault}");
        ExitCode::FAILURE
      }
    };
  }

  let wearing = match Wearing::worn() {
    Ok(wearing) => wearing,
    Err(why) => {
      eprintln!("console-control-center: no palette: {why}");

      return ExitCode::FAILURE;
    }
  };

  let mut surface = match Surface::connect() {
    Ok(surface) => surface,
    Err(why) => {
      eprintln!("console-control-center: {why}");

      return ExitCode::FAILURE;
    }
  };

  let capture = match capture::worker() {
    Ok(capture) => capture,
    Err(fault) => {
      console_core_logging::error!("couldn’t start screen capture: {fault}");
      return ExitCode::FAILURE;
    }
  };

  match run(&mut surface, &wearing, &capture) {
    Ok(()) => ExitCode::SUCCESS,
    Err(why) => {
      eprintln!("console-control-center: {why}");

      ExitCode::FAILURE
    }
  }
}

fn run(
  surface: &mut Surface,
  wearing: &Wearing,
  capture: &SyncSender<capture::Request>,
) -> Result<(), SurfaceError> {
  let requests = match focus::Layer::ScreenSelection.listen() {
    Ok(socket) => Some(socket),
    Err(fault) => {
      console_core_logging::warn!("couldn’t listen for screenshot shortcuts: {fault}");
      None
    }
  };
  let showing = edge(surface)?;

  let ran = iterate((surface, showing), |(surface_2, showing_2)| {
    Ok(
      match round(surface_2, wearing, showing_2, capture, requests.as_ref()) {
        Ok(showing_3) => Step::Again((surface_2, showing_3)),
        Err(fault) => Step::Halt(fault),
      },
    )
  });

  match ran {
    Ok(fault) => Err(fault),
    Err(Endless) => Ok(()),
  }
}

fn round(
  surface: &mut Surface,
  wearing: &Wearing,
  mut showing: Showing,
  capture: &SyncSender<capture::Request>,
  requests: Option<&UnixDatagram>,
) -> Result<Showing, SurfaceError> {
  let waiting = requests.map(AsFd::as_fd);
  surface.wait(waiting.as_slice(), None)?;

  match surface.closed() {
    Ok(Closed::Yes) => return Err(SurfaceError::Hung),
    Ok(Closed::No) => {}
  }

  let Ok(events) = surface.pointer_events();
  let Ok(keys) = surface.keyboard_events();
  if let Some(socket) = requests {
    match focus::heard(socket) {
      Ok(Some(focus::Request::Take))
        if !matches!(
          showing,
          Showing::Selecting {
            purpose: Purpose::Screenshot,
            ..
          }
        ) =>
      {
        return select(surface, wearing, Purpose::Screenshot, true);
      }
      Err(fault) => console_core_logging::warn!("reading screenshot shortcut: {fault}"),
      Ok(_) => {}
    }
  }

  for input in events
    .into_iter()
    .map(Input::Pointer)
    .chain(keys.into_iter().map(Input::Key))
  {
    let Ok(room) = surface.logical();
    let Ok(next) = match input {
      Input::Pointer(event) => step(&mut showing, event, room),
      Input::Key(KeyboardEvent::Down { key }) => keyed(&mut showing, key, room),
    };

    showing = match next {
      Next::Stay => showing,
      Next::Open => open(surface, wearing)?,
      Next::PutAway => edge(surface)?,
      Next::TakeScreenshot => {
        let put_away = edge(surface)?;
        let Ok(()) = send_capture(capture, capture::Request::Screenshot(None));

        put_away
      }
      Next::Select(purpose) => select(surface, wearing, purpose, false)?,
      Next::Capture(purpose, region) => submit_capture(surface, capture, purpose, region)?,
      Next::Close => {
        let put_away = edge(surface)?;
        let Ok(()) = start(InternalProgram::PutAway);

        put_away
      }
      Next::Repaint => {
        match &showing {
          Showing::Sheet { sheet, focused, .. } => {
            paint_sheet(surface, wearing, sheet, *focused)?
          }
          Showing::Selecting {
            purpose,
            prompt,
            selection,
          } => paint_selection(surface, wearing, *purpose, *prompt, *selection)?,
          Showing::Edge { .. } => {}
        }

        showing
      }
    };
  }

  Ok(showing)
}

enum Input {
  Pointer(PointerEvent),
  Key(KeyboardEvent),
}

fn keyed(showing: &mut Showing, key: Keysym, room: Option<Size<u32>>) -> Result<Next, Never> {
  let (sheet, focused) = match showing {
    Showing::Sheet { sheet, focused, .. } => (sheet, focused),
    Showing::Selecting {
      purpose, selection, ..
    } => {
      return selection_key(*purpose, selection, key, room);
    }
    Showing::Edge { .. } => return Ok(Next::Stay),
  };

  let target = match key {
    Keysym::Left | Keysym::ISO_Left_Tab => match *focused {
      Tapped::Screenshot | Tapped::Outside => Tapped::Close,
      Tapped::ScreenshotArea => Tapped::Screenshot,
      Tapped::CaptureText => Tapped::ScreenshotArea,
      Tapped::Close => Tapped::CaptureText,
    },
    Keysym::Right | Keysym::Tab => match *focused {
      Tapped::Screenshot => Tapped::ScreenshotArea,
      Tapped::ScreenshotArea => Tapped::CaptureText,
      Tapped::CaptureText => Tapped::Close,
      Tapped::Outside | Tapped::Close => Tapped::Screenshot,
    },
    Keysym::Up | Keysym::Down => match *focused {
      Tapped::Screenshot | Tapped::Outside => Tapped::CaptureText,
      Tapped::ScreenshotArea => Tapped::Close,
      Tapped::CaptureText => Tapped::Screenshot,
      Tapped::Close => Tapped::ScreenshotArea,
    },
    Keysym::Return | Keysym::KP_Enter | Keysym::space => {
      return activated(sheet, Activation::Focused, *focused);
    }
    Keysym::Escape => return Ok(Next::PutAway),
    _ => return Ok(Next::Stay),
  };

  *focused = target;
  Ok(Next::Repaint)
}

fn selection_key(
  purpose: Purpose,
  selection: &mut Selection,
  key: Keysym,
  room: Option<Size<u32>>,
) -> Result<Next, Never> {
  if key == Keysym::Escape {
    return Ok(Next::PutAway);
  }
  let (Purpose::Screenshot, Some(room)) = (purpose, room) else {
    return Ok(Next::Stay);
  };
  let mut corners = match selection {
    Selection::Corners(corners) => *corners,
    Selection::Ready => Corners::centered(room),
    Selection::Dragging(_) => return Ok(Next::Stay),
  };

  match key {
    Keysym::Left => corners.move_by(-1, 0, room),
    Keysym::Right => corners.move_by(1, 0, room),
    Keysym::Up => corners.move_by(0, -1, room),
    Keysym::Down => corners.move_by(0, 1, room),
    Keysym::Tab | Keysym::ISO_Left_Tab | Keysym::F18 => corners.toggle_fine(),
    Keysym::Return | Keysym::KP_Enter | Keysym::space => {
      let Ok(region) = corners.pick(room);
      if let Some(region) = region {
        return Ok(Next::Capture(purpose, region));
      }
    }
    _ => return Ok(Next::Stay),
  }

  *selection = Selection::Corners(corners);
  Ok(Next::Repaint)
}

fn activated(
  sheet: &Sheet,
  activation: Activation<(f64, f64)>,
  focused: Tapped,
) -> Result<Next, Never> {
  let Ok(target) = activation.resolve(
    || Some(focused),
    |point| match hit_test(sheet, point) {
      ::core::result::Result::Ok(_result_value) => ::core::option::Option::Some(_result_value),
      ::core::result::Result::Err(_impossible_variant) => match _impossible_variant {},
    },
  );

  Ok(match target {
    Some(Tapped::Screenshot) => Next::TakeScreenshot,
    Some(Tapped::ScreenshotArea) => Next::Select(Purpose::Screenshot),
    Some(Tapped::CaptureText) => Next::Select(Purpose::Text),
    Some(Tapped::Close) => Next::Close,
    Some(Tapped::Outside) => Next::PutAway,
    None => Next::Stay,
  })
}

fn step(
  showing: &mut Showing,
  event: PointerEvent,
  room: Option<Size<u32>>,
) -> Result<Next, Never> {
  Ok(match (showing, event) {
    (
      Showing::Selecting {
        purpose,
        prompt,
        selection,
      },
      event_2,
    ) => {
      return selection_step(*purpose, *prompt, selection, event_2, room);
    }
    (Showing::Edge { from }, PointerEvent::Down { at }) => {
      *from = Some(at);

      Next::Stay
    }
    (Showing::Edge { from: Some(from) }, PointerEvent::Moved { at }) => {
      let Ok(swipe) = classify_swipe(*from, at);

      match swipe {
        Swipe::Down => Next::Open,
        Swipe::Up | Swipe::Neither => Next::Stay,
      }
    }
    (Showing::Edge { from }, PointerEvent::Up) => match from.take() {
      Some(_) => Next::Open,
      None => Next::Stay,
    },
    (Showing::Edge { from }, PointerEvent::Left) => {
      *from = None;

      Next::Stay
    }
    (Showing::Edge { from: None }, PointerEvent::Moved { .. })
    | (Showing::Edge { .. }, PointerEvent::Scrolled { .. } | PointerEvent::Pinched { .. }) => {
      Next::Stay
    }
    (Showing::Sheet { at, .. }, PointerEvent::Down { at: down }) => {
      *at = Some(down);

      Next::Stay
    }
    (
      Showing::Sheet {
        sheet,
        at: Some(at),
        focused,
      },
      PointerEvent::Up,
    ) => return activated(sheet, Activation::At(*at), *focused),
    (Showing::Sheet { at: Some(from), .. }, PointerEvent::Moved { at }) => {
      let Ok(swipe) = classify_swipe(*from, at);

      match swipe {
        Swipe::Up => Next::PutAway,
        Swipe::Down | Swipe::Neither => Next::Stay,
      }
    }
    (Showing::Sheet { at: None, .. }, PointerEvent::Up | PointerEvent::Moved { .. })
    | (
      Showing::Sheet { .. },
      PointerEvent::Scrolled { .. } | PointerEvent::Pinched { .. } | PointerEvent::Left,
    ) => Next::Stay,
  })
}

fn selection_step(
  purpose: Purpose,
  prompt: Prompt,
  selection: &mut Selection,
  event: PointerEvent,
  room: Option<Size<u32>>,
) -> Result<Next, Never> {
  Ok(match event {
    PointerEvent::Down { at: (x, y) } => {
      let point = Point { x, y };
      *selection = Selection::Dragging(Drag {
        from: point,
        at: point,
      });
      Next::Stay
    }
    PointerEvent::Moved { at: (x, y) } => match selection {
      Selection::Dragging(drag) => {
        drag.at = Point { x, y };
        Next::Repaint
      }
      Selection::Ready | Selection::Corners(_) => Next::Stay,
    },
    PointerEvent::Up => {
      let Selection::Dragging(drag) = *selection else {
        return Ok(Next::Stay);
      };
      *selection = Selection::Ready;
      let region = room.and_then(|size| {
        let Ok(region) = drag.region(size);
        region
      });
      let Ok(cancel) = prompt.cancels(drag.from);
      let Ok(ended_on_cancel) = prompt.cancels(drag.at);

      match (region, cancel, ended_on_cancel) {
        (Some(region_2), _, _) => Next::Capture(purpose, region_2),
        (None, Covers::Yes, Covers::Yes) => Next::PutAway,
        (None, _, _) => Next::Repaint,
      }
    }
    PointerEvent::Left | PointerEvent::Pinched { .. } => match selection {
      Selection::Dragging(_) => {
        *selection = Selection::Ready;
        Next::Repaint
      }
      Selection::Ready | Selection::Corners(_) => Next::Stay,
    },
    PointerEvent::Scrolled { .. } => Next::Stay,
  })
}

fn submit_capture(
  surface: &mut Surface,
  capture: &SyncSender<capture::Request>,
  purpose: Purpose,
  region: Rectangle<i32>,
) -> Result<Showing, SurfaceError> {
  let located = screen_region(region);
  let put_away = edge(surface)?;

  let Ok(()) = match located {
    Ok(at) => send_capture(
      capture,
      match purpose {
        Purpose::Text => capture::Request::Text(at),
        Purpose::Screenshot => capture::Request::Screenshot(Some(at)),
      },
    ),
    Err(fault) => {
      console_core_logging::warn!("screen selection: {fault}");
      capture::notify("Couldn’t capture the selection")
    }
  };

  Ok(put_away)
}

fn send_capture(
  capture: &SyncSender<capture::Request>,
  request: capture::Request,
) -> Result<(), Never> {
  match capture.try_send(request) {
    Ok(()) => Ok(()),
    Err(TrySendError::Full(_)) => capture::notify("Screen capture is busy"),
    Err(TrySendError::Disconnected(_)) => capture::notify("Screen capture is unavailable"),
  }
}

fn screen_region(region: Rectangle<i32>) -> Result<Rectangle<i64>, std::io::Error> {
  let layers =
    console_compositor::ask(console_compositor::Layers).map_err(std::io::Error::other)?;
  let Ok(namespace) = focus::Layer::ScreenSelection.namespace();
  let Ok(frame) = console_onscreen::layer_state(&layers, namespace);
  let frame_2 =
    frame.ok_or_else(|| std::io::Error::other("the selection surface disappeared"))?;
  let Ok(size) = region.size.map(i64::from);
  Ok(Rectangle {
    origin: Point {
      x: frame_2.x.saturating_add(i64::from(region.origin.x)),
      y: frame_2.y.saturating_add(i64::from(region.origin.y)),
    },
    size,
  })
}

fn edge(surface: &mut Surface) -> Result<Showing, SurfaceError> {
  let Ok(()) = surface.hide();

  surface.show(&Wanted {
    namespace: CONTROL_CENTER.to_string(),
    anchor: Anchor::Top,
    size: Size {
      width: 0,
      height: EDGE,
    },
    margin: Margin::default(),
    keyboard: Keyboard::Declines,
    room: Room::Over,
    under: Under::None,
  })?;

  paint(surface, &[])?;

  Ok(Showing::Edge { from: None })
}

fn open(surface: &mut Surface, wearing: &Wearing) -> Result<Showing, SurfaceError> {
  open_overlay(surface, CONTROL_CENTER_PANEL)?;
  let room = match surface.logical() {
    Ok(Some(room)) => room,
    Ok(None) => return edge(surface),
  };

  let Ok(laid) = sheet(room, wearing);
  let focused = Tapped::Screenshot;
  paint_sheet(surface, wearing, &laid, focused)?;

  Ok(Showing::Sheet {
    sheet: Box::new(laid),
    at: None,
    focused,
  })
}

fn select(
  surface: &mut Surface,
  wearing: &Wearing,
  purpose: Purpose,
  with_corners: bool,
) -> Result<Showing, SurfaceError> {
  let Ok(namespace) = focus::Layer::ScreenSelection.namespace();
  open_overlay(surface, namespace)?;
  let room = match surface.logical() {
    Ok(Some(room)) => room,
    Ok(None) => return edge(surface),
  };

  let Ok(prompt) = selection::prompt(room, wearing);
  let selection = if with_corners {
    Selection::Corners(Corners::centered(room))
  } else {
    Selection::Ready
  };
  paint_selection(surface, wearing, purpose, prompt, selection)?;
  Ok(Showing::Selecting {
    purpose,
    prompt,
    selection,
  })
}

fn open_overlay(surface: &mut Surface, namespace: &str) -> Result<(), SurfaceError> {
  let Ok(()) = surface.hide();

  surface.show(&Wanted {
    namespace: namespace.to_string(),
    anchor: Anchor::Whole,
    size: Size {
      width: 0,
      height: 0,
    },
    margin: Margin::default(),
    keyboard: Keyboard::Takes,
    room: Room::Over,
    under: Under::None,
  })
}

fn paint_sheet(
  surface: &mut Surface,
  wearing: &Wearing,
  sheet: &Sheet,
  focused: Tapped,
) -> Result<(), SurfaceError> {
  let mut laid = sheet.clone();
  let outline = Edge::Of {
    wide: 2,
    color: wearing.pink,
  };

  match focused {
    Tapped::Screenshot => laid.screenshot.edge = outline,
    Tapped::ScreenshotArea => laid.screenshot_area.edge = outline,
    Tapped::CaptureText => laid.capture_text.edge = outline,
    Tapped::Close => laid.close.edge = outline,
    Tapped::Outside => {}
  }

  let Ok(screenshot) = measure_label(SCREENSHOT, laid.screenshot.size.width);
  let Ok(screenshot_area) = measure_label(SCREENSHOT_AREA, laid.screenshot_area.size.width);
  let Ok(capture_text) = measure_label(CAPTURE_TEXT, laid.capture_text.size.width);
  let Ok(close) = measure_label(CLOSE, laid.close.size.width);
  let Ok(shapes) = render(
    &laid,
    Labels {
      screenshot,
      screenshot_area,
      capture_text,
      close,
    },
    wearing,
  );

  paint(surface, &shapes)
}

fn paint_selection(
  surface: &mut Surface,
  wearing: &Wearing,
  purpose: Purpose,
  prompt: Prompt,
  selection: Selection,
) -> Result<(), SurfaceError> {
  let Ok(Some(room)) = surface.logical() else {
    return Ok(());
  };
  let (said, guides) = match selection {
    Selection::Dragging(drag) => {
      let Ok(region) = drag.region(room);
      if let Some(region) = region {
        let Ok(shapes) = selection::outline(region, wearing);
        return paint(surface, &shapes);
      }
      (purpose.instruction(), Vec::new())
    }
    Selection::Corners(corners) => {
      let Ok(shapes) = corners.shapes(room, wearing);
      (corners.instruction(), shapes)
    }
    Selection::Ready => (purpose.instruction(), Vec::new()),
  };
  let Ok(instruction) = measure_label(said, prompt.instruction.size.width);
  let Ok(cancel) = measure_label(CANCEL, prompt.cancel.size.width);
  let Ok(header) = prompt.shapes(said, instruction, cancel, wearing);
  let mut shapes = Vec::from(header);
  if purpose == Purpose::Screenshot {
    let Ok(measured) = measure_label(CORNER_CONTROLS, prompt.controls.size.width);
    let Ok(controls) = prompt.control_shapes(measured, wearing);
    shapes.extend(controls);
  }
  shapes.extend(guides);
  paint(surface, &shapes)
}

fn measure_label(said: &str, width: u32) -> Result<Size<u32>, Never> {
  let Ok(font) = TextStyle::Headline.font();
  painting::measure_text(
    Run {
      said,
      weight: Weight::Bold,
      width,
    },
    &font,
  )
}

fn paint(surface: &mut Surface, shapes: &[Shape]) -> Result<(), SurfaceError> {
  let points = match surface.logical() {
    Ok(Some(points)) => points,
    Ok(None) => return Ok(()),
  };

  let Ok(painting) = painting::painter(points, shapes, "console-control-center");
  surface.draw(painting)
}

fn start(program: InternalProgram) -> Result<(), Never> {
  let Ok(mut starting) = program.command();
  let Ok(named) = program.name();

  let _ = starting.stdout(Stdio::null());

  match console_program_lifetime::let_go(&mut starting) {
    Ok(_it_outlives_the_sheet) => {}
    Err(why) => eprintln!("console-control-center: {named}: {why}"),
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_color::palette;
  use console_draw_surface::fingers::{Fingers, Touch};

  #[test]
  fn clicking_the_top_edge_opens_the_sheet() {
    let mut showing = Showing::Edge { from: None };
    assert_eq!(
      step(&mut showing, PointerEvent::Down { at: (50.0, 3.0) }, None),
      Ok(Next::Stay)
    );
    assert_eq!(step(&mut showing, PointerEvent::Up, None), Ok(Next::Open));
  }

  #[test]
  fn lifting_a_finger_submits_its_dragged_region() -> Result<(), palette::PaletteError> {
    let Ok(colors) = palette::read(include_str!(
      "../../../../files/usr/local/lib/console/palette.sh"
    ));
    let wearing = Wearing::out_of(&colors)?;
    let room = Size {
      width: 800,
      height: 600,
    };
    let Ok(prompt) = selection::prompt(room, &wearing);
    let purpose = Purpose::Screenshot;
    let mut showing = Showing::Selecting {
      purpose,
      prompt,
      selection: Selection::Ready,
    };
    let mut fingers = Fingers::default();
    let touches = [
      Touch::Down {
        id: 1,
        at: (50.0, 70.0),
      },
      Touch::Moved {
        id: 1,
        at: (105.0, 90.0),
      },
      Touch::Up { id: 1 },
    ];
    let expected = [
      Next::Stay,
      Next::Repaint,
      Next::Capture(
        purpose,
        Rectangle {
          origin: Point { x: 50, y: 70 },
          size: Size {
            width: 55,
            height: 20,
          },
        },
      ),
    ];
    for (touch, next) in touches.into_iter().zip(expected) {
      let Ok(event) = fingers.handle_touch(touch);
      assert_eq!(
        event.map(|event| step(&mut showing, event, Some(room))),
        Some(Ok(next))
      );
    }

    Ok(())
  }

  #[test]
  fn the_shared_keyboard_and_pad_keys_pick_two_corners_and_allow_fine_movement() {
    let room = Some(Size {
      width: 800,
      height: 600,
    });
    let purpose = Purpose::Screenshot;
    let mut selecting = Selection::Ready;
    for key in [
      Keysym::Left,
      Keysym::Return,
      Keysym::Return,
      Keysym::Right,
      Keysym::Down,
      Keysym::F18,
      Keysym::Down,
    ] {
      assert_eq!(
        selection_key(purpose, &mut selecting, key, room),
        Ok(Next::Repaint)
      );
    }
    assert_eq!(
      selection_key(purpose, &mut selecting, Keysym::Return, room),
      Ok(Next::Capture(
        purpose,
        Rectangle {
          origin: Point { x: 368, y: 300 },
          size: Size {
            width: 32,
            height: 33
          }
        },
      ))
    );
    assert_eq!(
      selection_key(purpose, &mut selecting, Keysym::Escape, room),
      Ok(Next::PutAway)
    );
  }

  #[test]
  fn each_button_can_be_tapped_or_selected() -> Result<(), palette::PaletteError> {
    let Ok(colors) = palette::read(include_str!(
      "../../../../files/usr/local/lib/console/palette.sh"
    ));
    let wearing = Wearing::out_of(&colors)?;
    let Ok(laid) = sheet(
      Size {
        width: 800,
        height: 600,
      },
      &wearing,
    );
    let buttons = [
      (laid.screenshot, Next::TakeScreenshot),
      (laid.screenshot_area, Next::Select(Purpose::Screenshot)),
      (laid.capture_text, Next::Select(Purpose::Text)),
      (laid.close, Next::Close),
    ];
    let mut showing = Showing::Sheet {
      sheet: Box::new(laid),
      at: None,
      focused: Tapped::Close,
    };

    for (panel, expected) in buttons {
      let Ok(focus) = keyed(&mut showing, Keysym::Tab, None);
      assert_eq!(focus, Next::Repaint);
      let Ok(chosen) = keyed(&mut showing, Keysym::Return, None);

      assert_eq!(chosen, expected);
      let at = (
        f64::from(panel.at.x) + f64::from(panel.size.width) / 2.0,
        f64::from(panel.at.y) + f64::from(panel.size.height) / 2.0,
      );
      let Ok(down) = step(&mut showing, PointerEvent::Down { at }, None);
      let Ok(up) = step(&mut showing, PointerEvent::Up, None);
      assert_eq!(down, Next::Stay);
      assert_eq!(up, expected);
    }

    Ok(())
  }
}
