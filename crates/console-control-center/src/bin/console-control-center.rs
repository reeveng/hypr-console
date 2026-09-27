use std::process::{ExitCode, Stdio};

use console_control_center::{CLOSE, EDGE, Labels, SCREENSHOT, Sheet, Swipe, Tapped, render, sheet, classify_swipe, hit_test};
use console_core_color::palette::Wearing;
use console_core_iteration::{Endless, Step, iterate};
use console_core_fonts::TextStyle;
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_internal_programs::InternalProgram;
use console_core_shapes::{Shape, Weight};
use console_draw_painting::{self as painting, Run};
use console_draw_surface::{
    Anchor, Closed, Keyboard, Margin, PointerEvent, Room, Surface, SurfaceError, Under, Wanted,
};
use console_onscreen::CONTROL_CENTER;

enum Showing {
    Edge { from: Option<(f64, f64)> },
    Sheet { sheet: Box<Sheet>, at: Option<(f64, f64)> },
}

enum Next {
    Stay,
    Open,
    PutAway,
    TakeScreenshot,
    Close,
}

fn main() -> ExitCode {
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

    match run(&mut surface, &wearing) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("console-control-center: {why}");

            ExitCode::FAILURE
        }
    }
}

fn run(surface: &mut Surface, wearing: &Wearing) -> Result<(), SurfaceError> {
    let showing = edge(surface)?;

    let ran = iterate((surface, showing), |(surface, showing)| {
        Ok(match round(surface, wearing, showing) {
            Ok(showing) => Step::Again((surface, showing)),
            Err(fault) => Step::Halt(fault),
        })
    });

    match ran {
        Ok(fault) => Err(fault),
        Err(Endless) => Ok(()),
    }
}

fn round(surface: &mut Surface, wearing: &Wearing, mut showing: Showing) -> Result<Showing, SurfaceError> {
    surface.wait(&[], None)?;

    match surface.closed() {
        Ok(Closed::Yes) => return Err(SurfaceError::Hung),
        Ok(Closed::No) => {},
    }

    let Ok(events) = surface.pointer_events();

    for event in events {
        let Ok(next) = step(&mut showing, event);

        showing = match next {
            Next::Stay => showing,
            Next::Open => open(surface, wearing)?,
            Next::PutAway => edge(surface)?,
            Next::TakeScreenshot => {
                let put_away = edge(surface)?;
                let Ok(()) = start(InternalProgram::Screenshot);

                put_away
            }
            Next::Close => {
                let put_away = edge(surface)?;
                let Ok(()) = start(InternalProgram::PutAway);

                put_away
            }
        };
    }

    Ok(showing)
}

fn step(showing: &mut Showing, event: PointerEvent) -> Result<Next, Never> {
    Ok(match (showing, event) {
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
        (Showing::Edge { from }, PointerEvent::Up | PointerEvent::Left) => {
            *from = None;

            Next::Stay
        }
        (Showing::Edge { from: None }, PointerEvent::Moved { .. })
        | (Showing::Edge { .. }, PointerEvent::Scrolled { .. } | PointerEvent::Pinched { .. }) => Next::Stay,
        (Showing::Sheet { at, .. }, PointerEvent::Down { at: down }) => {
            *at = Some(down);

            Next::Stay
        }
        (Showing::Sheet { sheet, at: Some(at) }, PointerEvent::Up) => {
            let Ok(tap) = hit_test(sheet, *at);

            match tap {
                Tapped::Screenshot => Next::TakeScreenshot,
                Tapped::Close => Next::Close,
                Tapped::Outside => Next::PutAway,
            }
        }
        (Showing::Sheet { at: Some(from), .. }, PointerEvent::Moved { at }) => {
            let Ok(swipe) = classify_swipe(*from, at);

            match swipe {
                Swipe::Up => Next::PutAway,
                Swipe::Down | Swipe::Neither => Next::Stay,
            }
        }
        (Showing::Sheet { at: None, .. }, PointerEvent::Up | PointerEvent::Moved { .. })
        | (Showing::Sheet { .. }, PointerEvent::Scrolled { .. } | PointerEvent::Pinched { .. } | PointerEvent::Left) => Next::Stay,
    })
}

fn edge(surface: &mut Surface) -> Result<Showing, SurfaceError> {
    let Ok(()) = surface.hide();

    surface.show(&Wanted {
        namespace: CONTROL_CENTER.to_string(),
        anchor: Anchor::Top,
        size: Size { width: 0, height: EDGE },
        margin: Margin::default(),
        keyboard: Keyboard::Declines,
        room: Room::Over,
        under: Under::None,
    })?;

    paint(surface, &[])?;

    Ok(Showing::Edge { from: None })
}

fn open(surface: &mut Surface, wearing: &Wearing) -> Result<Showing, SurfaceError> {
    let Ok(()) = surface.hide();

    surface.show(&Wanted {
        namespace: CONTROL_CENTER.to_string(),
        anchor: Anchor::Whole,
        size: Size { width: 0, height: 0 },
        margin: Margin::default(),
        keyboard: Keyboard::Declines,
        room: Room::Over,
        under: Under::None,
    })?;

    let room = match surface.logical() {
        Ok(Some(room)) => room,
        Ok(None) => return Ok(Showing::Edge { from: None }),
    };

    let Ok(laid) = sheet(room, wearing);
    let Ok(font) = TextStyle::Headline.font();
    let Ok(screenshot) = painting::measure_text(
        Run { said: SCREENSHOT, weight: Weight::Bold, width: laid.screenshot.size.width },
        &font,
    );
    let Ok(close) = painting::measure_text(
        Run { said: CLOSE, weight: Weight::Bold, width: laid.close.size.width },
        &font,
    );
    let Ok(shapes) = render(&laid, Labels { screenshot, close }, wearing);

    paint(surface, &shapes)?;

    Ok(Showing::Sheet { sheet: Box::new(laid), at: None })
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
        Ok(_it_outlives_the_sheet) => {},
        Err(why) => eprintln!("console-control-center: {named}: {why}"),
    }

    Ok(())
}
