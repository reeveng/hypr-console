use std::collections::BTreeMap;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::process::ExitCode;

use console_compositor::{ActiveWorkspace, Clients, DispatchResult, Monitors, Request};
use console_core_color::palette::Wearing;
use console_core_fonts::TextStyle;
use console_core_geometry::{Point, Size};
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_shapes::Pixels;
use console_draw_painting::{self as painting, Frame, Run};
use console_draw_surface::{
    Anchor, Closed, Keyboard, KeyboardEvent, Keysym, Margin, PointerEvent, Room, Surface, SurfaceError, Under, Wanted,
};
use console_onscreen::{OVERVIEW, OVERVIEW_EDGE};
use console_overview::{
    Chose, Choosing, Scene, EDGE, Dragged, Hit, Key, Measure, Overview, PlaceId, Stroke, Swipe, Wandered, carried, chose,
    chosen_window, dropped, gone_to, held_by_the_pad, hit, overview, places, render, resized,
};
use console_response_times::{Wait, Waiting};

struct Pango;

impl Measure for Pango {
    fn measure(&self, said: &str, style: TextStyle, width: u32) -> Result<Size<u32>, Never> {
        let Ok(font) = style.font();
        let Ok(weight) = style.weight();

        painting::measure_text(Run { said, weight, width }, &font)
    }
}

enum Showing {
    Edge { from: Option<Point<f64>> },
    Open { laid: Box<Overview>, front: Option<PlaceId>, touch: Touch, choosing: Box<Choosing>, pictures: BTreeMap<String, Pixels> },
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

const ASKED_TO_SHOW: &str = "--show";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Show,
    None,
}

fn main() -> ExitCode {
    let showing_only = std::env::args().skip(1).any(|word| word == ASKED_TO_SHOW);

    match showing_only {
        true => {
            let Ok(shown) = show();

            return shown;
        }
        false => {},
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
    let Ok(at) = socket();

    let at = match at {
        Some(at) => at,
        None => {
            eprintln!("console-overview: there is no session or no compositor to show the overview in");

            return Ok(ExitCode::FAILURE);
        }
    };

    let sent = UnixDatagram::unbound().and_then(|sending| sending.send_to(SHOW.as_bytes(), &at)).map(drop);

    Ok(match sent {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("console-overview: {}: {why}", at.display());

            ExitCode::FAILURE
        }
    })
}

fn socket() -> Result<Option<PathBuf>, Never> {
    let Ok(ours) = console_core_places::application_runtime();
    let Ok(instance) = console_compositor::instance();

    Ok(ours.zip(instance).map(|(ours, instance)| ours.join(format!("{OVERVIEW}-{instance}.sock"))))
}

fn listen() -> Result<Option<UnixDatagram>, Never> {
    let Ok(at) = socket();

    let at = match at {
        Some(at) => at,
        None => return Ok(None),
    };

    match std::fs::remove_file(&at) {
        Ok(()) => {},
        Err(_no_overview_was_listening_here) => {},
    }

    let bound = UnixDatagram::bind(&at).and_then(|told| {
        told.set_nonblocking(true)?;

        Ok(told)
    });

    Ok(match bound {
        Ok(told) => Some(told),
        Err(why) => {
            eprintln!("console-overview: {}: {why}; the pad cannot open the overview", at.display());

            None
        }
    })
}

fn heard(told: Option<&UnixDatagram>) -> Result<Command, Never> {
    let told = match told {
        Some(told) => told,
        None => return Ok(Command::None),
    };

    let said = std::iter::from_fn(|| {
        let mut said = [0_u8; 16];

        match told.recv(&mut said) {
            Ok(got) => Some(said.get(..got) == Some(SHOW.as_bytes())),
            Err(_nothing_more_was_said) => None,
        }
    });

    Ok(said.fold(Command::None, |asked, show| match show {
        true => Command::Show,
        false => asked,
    }))
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

    let ran = console_core_iteration::iterate((surface, showing), |(surface, showing)| turned(surface, wearing, told.as_ref(), showing));

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
    let waiting: Vec<BorrowedFd<'_>> = told.iter().map(|told| told.as_fd()).collect();

    match surface.wait(&waiting, None) {
        Ok(()) => {},
        Err(fault) => return Ok(Step::Halt(Err(fault))),
    }

    match surface.closed() {
        Ok(Closed::Yes) => return Ok(Step::Halt(Err(SurfaceError::Hung))),
        Ok(Closed::No) => {},
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

    let (redraws, mut once): (Vec<Next>, Vec<Next>) = nexts.into_iter().partition(|next| matches!(next, Next::Redraw));

    match redraws.is_empty() {
        true => {},
        false => once.push(Next::Redraw),
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

fn carry_out(surface: &mut Surface, wearing: &Wearing, showing: Showing, next: Next) -> Result<Showing, SurfaceError> {
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

fn refreshed(surface: &mut Surface, wearing: &Wearing, showing: Showing) -> Result<Showing, SurfaceError> {
    let room = match surface.logical() {
        Ok(Some(room)) => room,
        Ok(None) => return Ok(showing),
    };

    let Ok(again) = looked(room);

    let showing = match (showing, again) {
        (Showing::Open { touch, choosing, pictures, .. }, Some((laid, front))) => {
            Showing::Open { laid: Box::new(laid), front, touch, choosing, pictures }
        }
        (open @ Showing::Open { .. }, None) => open,
        (edge @ Showing::Edge { .. }, Some(_) | None) => edge,
    };

    draw(surface, wearing, &showing)?;

    Ok(showing)
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
            let Ok(swipe) = (Stroke { from: *start, at: Point { x: at.0, y: at.1 } }).swipe();

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
        (None, PointerEvent::Moved { .. }) | (_, PointerEvent::Scrolled { .. } | PointerEvent::Pinched { .. }) => {
            Next::Stay
        }
    })
}

fn step_open(laid: &Overview, touch: &mut Touch, event: PointerEvent) -> Result<Next, Never> {
    let lifted = std::mem::replace(touch, Touch::Lifted);

    let (after, next) = match (lifted, event) {
        (Touch::Lifted, PointerEvent::Down { at }) => {
            let at = Point { x: at.0, y: at.1 };
            let Ok(on) = hit(laid, at);

            (Touch::Pressed { stroke: Stroke { from: at, at }, on }, Next::Stay)
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
            let at = Point { x: at.0, y: at.1 };
            let Ok(lua) = resized(laid, &left, at.x - last.x);

            (Touch::Resizing { left, last: at }, Next::Ask(lua))
        }
        (
            Touch::Lifted | Touch::Pressed { .. } | Touch::Resizing { .. },
            PointerEvent::Up | PointerEvent::Left,
        ) => (Touch::Lifted, Next::Stay),
        (still, PointerEvent::Down { .. } | PointerEvent::Moved { .. } | PointerEvent::Scrolled { .. } | PointerEvent::Pinched { .. }) => {
            (still, Next::Stay)
        }
    };

    *touch = after;

    Ok(next)
}

fn pressed_moved(laid: &Overview, stroke: Stroke, on: Hit, at: Point<f64>) -> Result<(Touch, Next), Never> {
    let stroke = Stroke { from: stroke.from, at };
    let Ok(wandered) = stroke.wandered();
    let Ok(swipe) = stroke.swipe();

    Ok(match (wandered, on) {
        (Wandered::No, on) => (Touch::Pressed { stroke, on }, Next::Stay),
        (Wandered::Yes, Hit::Tile { address, from, frame }) => {
            (Touch::Dragging(Dragged { address, frame, stroke }, from), Next::Redraw)
        }
        (Wandered::Yes, Hit::Divider { left }) => {
            let Ok(lua) = resized(laid, &left, at.x - stroke.from.x);

            (Touch::Resizing { left, last: at }, Next::Ask(lua))
        }
        (Wandered::Yes, Hit::Card(_) | Hit::Close | Hit::None) => match swipe {
            Swipe::Down => (Touch::Lifted, Next::PutAway),
            Swipe::Up | Swipe::Neither => (Touch::Lifted, Next::Stay),
        },
    })
}

fn tapped(on: &Hit) -> Result<Next, Never> {
    let Ok(went) = gone_to(on);

    Ok(match (went, on) {
        (Some(lua), _) => Next::AskAndPutAway(lua),
        (None, Hit::Divider { .. } | Hit::Card(_)) => Next::Stay,
        (None, Hit::Tile { .. } | Hit::Close | Hit::None) => Next::PutAway,
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
    let Ok(laid) = overview(&grouped, match screen {
        Some(screen) => screen,
        None => room,
    });

    Ok(Some((laid, front.map(|workspace| PlaceId(workspace.id)))))
}

fn ask(lua: &str) -> Result<(), Never> {
    let Ok(answered) = console_compositor::request(Request::Script, lua);

    match answered {
        DispatchResult::Success => {},
        DispatchResult::Failure(why) => eprintln!("console-overview: {lua}: {why}"),
    }

    Ok(())
}

fn edge(surface: &mut Surface) -> Result<Showing, SurfaceError> {
    let Ok(()) = surface.hide();

    surface.show(&Wanted {
        namespace: OVERVIEW_EDGE.to_string(),
        anchor: Anchor::Bottom,
        size: Size { width: 0, height: EDGE },
        margin: Margin::default(),
        keyboard: Keyboard::Declines,
        room: Room::Over,
        under: Under::Anything,
    })?;

    paint(surface, &[])?;

    Ok(Showing::Edge { from: None })
}

fn open(surface: &mut Surface, wearing: &Wearing) -> Result<Showing, SurfaceError> {
    let Ok(mut waiting) = Waiting::here(Wait { who: OVERVIEW, what: "opening" });
    let Ok(()) = surface.hide();

    surface.show(&Wanted {
        namespace: OVERVIEW.to_string(),
        anchor: Anchor::Whole,
        size: Size { width: 0, height: 0 },
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
        Some(seen) => seen,
        None => return edge(surface),
    };
    let Ok(()) = waiting.mark("compositor");
    let windows = laid.cards.iter().flat_map(|card| card.tiles.iter()).fold(0_u64, |many, _| many.saturating_add(1));
    let Ok(()) = waiting.counted("windows", windows);
    let mut showing =
        Showing::Open { laid: Box::new(laid), front, touch: Touch::Lifted, choosing: Box::new(Choosing::Untouched), pictures: BTreeMap::new() };

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
        Showing::Open { pictures, .. } => pictures.extend(taken.into_iter().map(|one| (one.address, one.pixels))),
        Showing::Edge { .. } => {},
    }

    draw(surface, wearing, &showing)?;

    let Ok(()) = waiting.mark("pictured");
    let Ok(()) = waiting.finish();

    Ok(showing)
}

fn draw(surface: &mut Surface, wearing: &Wearing, showing: &Showing) -> Result<(), SurfaceError> {
    let shapes = match showing {
        Showing::Edge { .. } => Vec::new(),
        Showing::Open { laid, front, touch, choosing, pictures } => {
            let Ok(by_the_pad) = held_by_the_pad(laid, choosing);
            let Ok(chosen) = chosen_window(laid, choosing);
            let (held, landing) = match touch {
                Touch::Dragging(held, _) => {
                    let Ok(landing) = console_overview::landing(laid, held);

                    (Some(held), landing)
                }
                Touch::Lifted | Touch::Pressed { .. } | Touch::Resizing { .. } => (by_the_pad.as_ref(), None),
            };
            let drawn = Scene { overview: laid, front: *front, held, landing, chosen: chosen.as_deref(), pictures, wearing };
            let Ok(shapes) = render(&drawn, &Pango);

            shapes
        }
    };

    paint(surface, &shapes)
}

fn paint(surface: &mut Surface, shapes: &[console_core_shapes::Shape]) -> Result<(), SurfaceError> {
    let points = match surface.logical() {
        Ok(Some(points)) => points,
        Ok(None) => return Ok(()),
    };

    surface.draw(|pixels, device, _scale| {
        match painting::onto(pixels, Frame { device, points }, shapes) {
            Ok(()) => {},
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
        let opened: Vec<&str> = nexts.iter().filter(|next| matches!(next, Next::Open)).map(|_| "open").collect();

        assert_eq!(opened, ["open"], "every open hides the overview and shows it again, so a second one is a flicker");
    }

    #[test]
    fn moves_that_arrive_together_are_drawn_once() {
        let Ok(laid) = console_overview::overview(&[], Size { width: 1000, height: 1600 });
        let mut showing = Showing::Open {
            laid: Box::new(laid),
            front: None,
            touch: Touch::Lifted,
            choosing: Box::new(Choosing::Untouched),
            pictures: BTreeMap::new(),
        };
        let keys = [Key::Right, Key::Left, Key::Right, Key::Left];
        let Ok(after) = stepped(&mut showing, None, Vec::new(), keys.into_iter());
        let drawn: Vec<&str> = after.iter().filter(|next| matches!(next, Next::Redraw)).map(|_| "drawn").collect();

        assert_eq!(drawn, ["drawn"], "every redraw paints the same last state, and a paint slower than the moves arriving falls further behind with each one");
    }
}
