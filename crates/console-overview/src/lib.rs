//! The overview: every place at once, and a finger's way of arranging them.
//!
//! A window is not moved from where it is used. A touch on a window is the
//! app's, and nothing below the compositor can take it back once the app has
//! it -- only the compositor can say `wl_touch.cancel`, which is how an iPad
//! claims a finger late. So windows are arranged where every touch is already
//! ours, which is the answer iPad's App Switcher, Android's Recents and GNOME's
//! overview all give: swipe up from the bottom edge, and each place is a card
//! with its windows standing in it the way they stand on the screen.
//!
//! A window dragged onto another card joins that place beside what is there,
//! onto the last card it gets a place of its own, and anywhere along the top,
//! where a box saying so comes up, it closes -- a thumb aims at a band, not a
//! button. The line between two windows in a card is
//! dragged to resize the split. A tap goes to what it landed on, and a tap on
//! nothing, or a swipe back down, puts the overview away.
//!
//! This file is the arithmetic: what fits, what a point lands on, what a drop
//! means and what the compositor is asked for. The binary draws what it says
//! and carries it out.

mod mapping;
pub mod pictures;

use std::collections::BTreeMap;

use console_compositor::{Window, quote};
use console_core_color::palette::Wearing;
use console_core_fonts::TextStyle;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted, index, whole_i32, whole_u32};
use console_core_walking::{Ring, Step};
use console_core_shapes::{Edge, Panel, Picture, Pixels, Round, Shape, Text};

pub const EDGE: u32 = 10;

const PULL: f64 = 48.0;

const SLOP: f64 = 12.0;

const MARGIN: f64 = 32.0;

const GAP: f64 = 24.0;

const ACROSS: u32 = 3;

const NAME_TALL: f64 = 40.0;

const CLOSE_TALL: f64 = 72.0;

const CLOSE_WIDE: f64 = 280.0;

const REACH: f64 = 16.0;

const CARRIED: f64 = 0.6;

const TOUCHING: f64 = 4.0;

const CARD_ROUND: u32 = 16;

const TILE_ROUND: u32 = 8;

const CARD_EDGE: u32 = 2;

const EMPTY_PLACE: &str = "empty";

pub const CLOSE: &str = "Close";

pub const NEW: &str = "New Desktop";

const PLUS: &str = "+";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlaceId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub id: PlaceId,
    pub windows: Vec<Window>,
}

pub fn places(open: Vec<Window>) -> Result<Vec<Place>, Never> {
    let mut by_place: BTreeMap<PlaceId, Vec<Window>> = BTreeMap::new();

    for window in open {
        let placed = match window.workspace > 0 {
            true => Some(PlaceId(window.workspace)),
            false => None,
        };

        match placed {
            Some(id) => by_place.entry(id).or_default().push(window),
            None => {},
        }
    }

    Ok(by_place
        .into_iter()
        .map(|(id, mut windows)| {
            windows.sort_by_key(|window| window.at);

            Place { id, windows }
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub at: Point<f64>,
    pub size: Size<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holds {
    Yes,
    No,
}

impl Frame {
    pub fn holds(&self, at: Point<f64>) -> Result<Holds, Never> {
        let inside = at.x >= self.at.x
            && at.y >= self.at.y
            && at.x < self.at.x + self.size.width
            && at.y < self.at.y + self.size.height;

        Ok(match inside {
            true => Holds::Yes,
            false => Holds::No,
        })
    }

    pub fn moved(&self, by: Point<f64>) -> Result<Frame, Never> {
        Ok(Frame { at: Point { x: self.at.x + by.x, y: self.at.y + by.y }, size: self.size })
    }

    fn panel(&self, round: u32, fill: console_core_color::Oklch, edge: Edge) -> Result<Panel, Never> {
        let Ok(across) = whole_i32(self.at.x);
        let Ok(down) = whole_i32(self.at.y);
        let Ok(wide) = whole_u32(self.size.width);
        let Ok(tall) = whole_u32(self.size.height);

        Ok(Panel { at: Point { x: across, y: down }, size: Size { width: wide, height: tall }, round: Round(round), fill, edge })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Place(PlaceId),
    New,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tile {
    pub address: String,
    pub class: String,
    pub frame: Frame,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Divider {
    pub left: String,
    pub x: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub target: Target,
    pub frame: Frame,
    pub tiles: Vec<Tile>,
    pub dividers: Vec<Divider>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Overview {
    pub cards: Vec<Card>,
    pub close: Frame,
    pub shrink: f64,
    pub room: Size<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Grid {
    columns: u32,
    card: Size<f64>,
    left: f64,
    top: f64,
}

fn grid(cards: u32, room: Size<f64>) -> Result<Grid, Never> {
    let aspect = room.height / room.width.max(1.0);
    let across = cards.clamp(1, ACROSS);
    let rows = cards.div_ceil(across).max(1);
    let wide = f64::from(across);
    let tall = f64::from(rows);
    let above = MARGIN + CLOSE_TALL + GAP;
    let width_room = room.width - MARGIN * 2.0 - GAP * (wide - 1.0);
    let height_room = room.height - above - MARGIN;
    let by_width = width_room / wide * aspect;
    let by_height = (height_room - GAP * (tall - 1.0)) / tall - NAME_TALL;
    let card_tall = by_width.min(by_height).max(0.0);
    let card_wide = card_tall / aspect.max(f64::EPSILON);
    let grid_wide = card_wide * wide + GAP * (wide - 1.0);
    let grid_tall = (card_tall + NAME_TALL) * tall + GAP * (tall - 1.0);

    Ok(Grid {
        columns: across,
        card: Size { width: card_wide, height: card_tall },
        left: (room.width - grid_wide) / 2.0,
        top: above + (height_room - grid_tall) / 2.0,
    })
}

pub fn points(size: Size<u32>) -> Result<Size<f64>, Never> {
    Ok(Size { width: f64::from(size.width), height: f64::from(size.height) })
}

pub fn overview(places: &[Place], screen: Size<u32>) -> Result<Overview, Never> {
    let Ok(room) = points(screen);
    let Ok(count) = fitted::<_, u32>(places.len());
    let Ok(laid) = grid(count.saturating_add(1), room);
    let shrink = laid.card.width / room.width.max(1.0);

    let targets = places.iter().map(Some).chain(std::iter::once(None));

    let cards = (0u32..)
        .zip(targets)
        .map(|(position, place)| {
            let Ok(frame) = cell(laid, position);
            let Ok(card) = card_for(place, frame, shrink);

            card
        })
        .collect();

    let close = Frame {
        at: Point { x: (room.width - CLOSE_WIDE) / 2.0, y: MARGIN },
        size: Size { width: CLOSE_WIDE, height: CLOSE_TALL },
    };

    Ok(Overview { cards, close, shrink, room })
}

fn cell(laid: Grid, position: u32) -> Result<Frame, Never> {
    let (column, row) = match (position.checked_rem(laid.columns), position.checked_div(laid.columns)) {
        (Some(column), Some(row)) => (column, row),
        (None, _) | (_, None) => (position, 0),
    };

    Ok(Frame {
        at: Point {
            x: laid.left + f64::from(column) * (laid.card.width + GAP),
            y: laid.top + f64::from(row) * (laid.card.height + NAME_TALL + GAP),
        },
        size: laid.card,
    })
}

fn card_for(place: Option<&Place>, frame: Frame, shrink: f64) -> Result<Card, Never> {
    Ok(match place {
        Some(place) => {
            let tiles: Vec<Tile> = place
                .windows
                .iter()
                .map(|window| {
                    let Ok(tile) = tile_for(window, frame, shrink);

                    tile
                })
                .collect();
            let Ok(dividers) = dividers_of(&tiles);

            Card { target: Target::Place(place.id), frame, tiles, dividers }
        }
        None => Card { target: Target::New, frame, tiles: Vec::new(), dividers: Vec::new() },
    })
}

fn tile_for(window: &Window, card: Frame, shrink: f64) -> Result<Tile, Never> {
    let Ok(across) = window.at.0.float();
    let Ok(down) = window.at.1.float();
    let Ok(wide) = window.size.0.float();
    let Ok(tall) = window.size.1.float();

    Ok(Tile {
        address: window.address.clone(),
        class: window.first_class.clone(),
        frame: Frame {
            at: Point { x: card.at.x + across * shrink, y: card.at.y + down * shrink },
            size: Size { width: wide * shrink, height: tall * shrink },
        },
    })
}

fn dividers_of(tiles: &[Tile]) -> Result<Vec<Divider>, Never> {
    Ok(tiles
        .iter()
        .zip(tiles.iter().skip(1))
        .filter_map(|(left, right)| {
            let edge = left.frame.at.x + left.frame.size.width;

            match (right.frame.at.x - edge).abs() <= TOUCHING {
                true => Some(Divider { left: left.address.clone(), x: right.frame.at.x }),
                false => None,
            }
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq)]
pub enum Hit {
    Divider { left: String },
    Tile { address: String, from: PlaceId, frame: Frame },
    Card(Target),
    Close,
    None,
}

pub fn hit(overview: &Overview, at: Point<f64>) -> Result<Hit, Never> {
    let on_card = overview.cards.iter().find_map(|card| {
        let Ok(found) = hit_card(card, at);

        found
    });

    let on_close = match at.y < overview.close.at.y + overview.close.size.height + GAP {
        true => Holds::Yes,
        false => Holds::No,
    };

    Ok(match (on_card, on_close) {
        (Some(found), Holds::Yes | Holds::No) => found,
        (None, Holds::Yes) => Hit::Close,
        (None, Holds::No) => Hit::None,
    })
}

fn hit_card(card: &Card, at: Point<f64>) -> Result<Option<Hit>, Never> {
    let Ok(inside) = card.frame.holds(at);

    Ok(match inside {
        Holds::No => None,
        Holds::Yes => {
            let Ok(within) = hit_within(card, at);

            Some(within)
        }
    })
}

fn hit_within(card: &Card, at: Point<f64>) -> Result<Hit, Never> {
    let divider = card.dividers.iter().find(|divider| (divider.x - at.x).abs() <= REACH);
    let tile = card.tiles.iter().find(|tile| {
        let Ok(inside) = tile.frame.holds(at);

        inside == Holds::Yes
    });

    Ok(match (divider, tile, card.target) {
        (Some(divider), _, Target::Place(_) | Target::New) => Hit::Divider { left: divider.left.clone() },
        (None, Some(tile), Target::Place(from)) => {
            Hit::Tile { address: tile.address.clone(), from, frame: tile.frame }
        }
        (None, Some(_) | None, target) => Hit::Card(target),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drop {
    Onto(PlaceId),
    New,
    Close,
    Back,
}

pub fn dropped(overview: &Overview, from: PlaceId, at: Point<f64>) -> Result<Drop, Never> {
    let Ok(landed) = hit(overview, at);

    let target = match landed {
        Hit::Close => return Ok(Drop::Close),
        Hit::None => return Ok(Drop::Back),
        Hit::Card(target) => target,
        Hit::Tile { from: under, .. } => Target::Place(under),
        Hit::Divider { .. } => match overview.cards.iter().find(|card| card.frame.holds(at) == Ok(Holds::Yes)) {
            Some(card) => card.target,
            None => return Ok(Drop::Back),
        },
    };

    Ok(match target {
        Target::New => Drop::New,
        Target::Place(id) => match id == from {
            true => Drop::Back,
            false => Drop::Onto(id),
        },
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stroke {
    pub from: Point<f64>,
    pub at: Point<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Swipe {
    Up,
    Down,
    Neither,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wandered {
    Yes,
    No,
}

impl Stroke {
    pub fn swipe(&self) -> Result<Swipe, Never> {
        let down = self.at.y - self.from.y;
        let across = (self.at.x - self.from.x).abs();

        Ok(match (down.abs() >= PULL && down.abs() > across, down > 0.0) {
            (true, true) => Swipe::Down,
            (true, false) => Swipe::Up,
            (false, _) => Swipe::Neither,
        })
    }

    pub fn wandered(&self) -> Result<Wandered, Never> {
        let across = self.at.x - self.from.x;
        let down = self.at.y - self.from.y;

        Ok(match across.hypot(down) > SLOP {
            true => Wandered::Yes,
            false => Wandered::No,
        })
    }

    pub fn by(&self) -> Result<Point<f64>, Never> {
        Ok(Point { x: self.at.x - self.from.x, y: self.at.y - self.from.y })
    }
}

fn window_named(address: &str) -> Result<String, Never> {
    quote(&format!("address:{address}"))
}

pub fn carried(address: &str, drop: Drop) -> Result<Option<String>, Never> {
    let Ok(window) = window_named(address);

    Ok(match drop {
        Drop::Onto(PlaceId(id)) => {
            let Ok(place) = quote(&id.to_string());

            Some(format!("hl.dsp.window.move({{ window = {window}, workspace = {place}, follow = false }})"))
        }
        Drop::New => {
            let Ok(place) = quote(EMPTY_PLACE);

            Some(format!("hl.dsp.window.move({{ window = {window}, workspace = {place}, follow = false }})"))
        }
        Drop::Close => Some(format!("hl.dsp.window.close({{ window = {window} }})")),
        Drop::Back => None,
    })
}

pub fn resized(overview: &Overview, left: &str, by: f64) -> Result<String, Never> {
    let Ok(window) = window_named(left);
    let Ok(across) = whole_i32(by / overview.shrink.max(f64::EPSILON));

    Ok(format!("hl.dsp.window.resize({{ window = {window}, x = {across}, y = 0, relative = true }})"))
}

pub fn gone_to(hit: &Hit) -> Result<Option<String>, Never> {
    Ok(match hit {
        Hit::Tile { address, .. } => {
            let Ok(window) = window_named(address);

            Some(format!("hl.dsp.focus({{ window = {window} }})"))
        }
        Hit::Card(Target::Place(PlaceId(id))) => {
            let Ok(place) = quote(&id.to_string());

            Some(format!("hl.dsp.focus({{ workspace = {place} }})"))
        }
        Hit::Card(Target::New) => {
            let Ok(place) = quote(EMPTY_PLACE);

            Some(format!("hl.dsp.focus({{ workspace = {place} }})"))
        }
        Hit::Divider { .. } | Hit::Close | Hit::None => None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Choose,
    More,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aim {
    Card(u32),
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Choosing {
    Untouched,
    Window(u32),
    Carrying(Carried),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Carried {
    pub address: String,
    pub from: PlaceId,
    pub frame: Frame,
    pub aim: Aim,
    pub home: u32,
    pub was: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chose {
    Stay,
    Ask(String),
    AskAndPutAway(String),
    PutAway,
}

#[derive(Debug, Clone, PartialEq)]
struct Standing {
    place: PlaceId,
    card: u32,
    tile: Tile,
}

fn every_window(overview: &Overview) -> Result<Vec<Standing>, Never> {
    Ok((0u32..)
        .zip(overview.cards.iter())
        .flat_map(|(card, shown)| {
            let place = match shown.target {
                Target::Place(place) => Some(place),
                Target::New => None,
            };

            shown.tiles.iter().filter_map(move |tile| place.map(|place| Standing { place, card, tile: tile.clone() }))
        })
        .collect())
}

pub fn chose(overview: &Overview, choosing: &Choosing, key: Key) -> Result<(Choosing, Chose), Never> {
    match choosing {
        Choosing::Untouched => untouched(key),
        Choosing::Window(at) => browse(overview, *at, key),
        Choosing::Carrying(carried) => carry(overview, carried, key),
    }
}

fn untouched(key: Key) -> Result<(Choosing, Chose), Never> {
    Ok(match key {
        Key::Back => (Choosing::Untouched, Chose::PutAway),
        Key::Left | Key::Right | Key::Up | Key::Down | Key::Choose | Key::More => (Choosing::Window(0), Chose::Stay),
    })
}

fn browse(overview: &Overview, at: u32, key: Key) -> Result<(Choosing, Chose), Never> {
    let Ok(windows) = every_window(overview);
    let Ok(ring) = Ring::of(&windows);

    let (ring, here) = match ring {
        Some(ring) => {
            let Ok(here) = ring.at(at);

            (ring, here)
        }
        None => {
            return Ok(match key {
                Key::Back => (Choosing::Untouched, Chose::PutAway),
                Key::Left | Key::Right | Key::Up | Key::Down | Key::Choose | Key::More => (Choosing::Untouched, Chose::Stay),
            });
        }
    };

    let Ok(position) = index(here);
    let standing = windows.get(position);

    Ok(match (key, standing) {
        (Key::Left, _) => {
            let Ok(before) = ring.step(here, Step::Back);

            (Choosing::Window(before), Chose::Stay)
        }
        (Key::Right, _) => {
            let Ok(after) = ring.step(here, Step::Forward);

            (Choosing::Window(after), Chose::Stay)
        }
        (Key::Back, _) => (Choosing::Window(here), Chose::PutAway),
        (Key::Choose, Some(standing)) => {
            let Ok(went) = gone_to(&Hit::Tile { address: standing.tile.address.clone(), from: standing.place, frame: standing.tile.frame });

            match went {
                Some(lua) => (Choosing::Window(here), Chose::AskAndPutAway(lua)),
                None => (Choosing::Window(here), Chose::Stay),
            }
        }
        (Key::More, Some(standing)) => (
            Choosing::Carrying(Carried {
                address: standing.tile.address.clone(),
                from: standing.place,
                frame: standing.tile.frame,
                aim: Aim::Card(standing.card),
                home: standing.card,
                was: here,
            }),
            Chose::Stay,
        ),
        (Key::Up | Key::Down, _) | (Key::Choose | Key::More, None) => (Choosing::Window(here), Chose::Stay),
    })
}

fn carry(overview: &Overview, carried: &Carried, key: Key) -> Result<(Choosing, Chose), Never> {
    let Ok(cards) = Ring::of(&overview.cards);
    let aimed = |aim: Aim| Choosing::Carrying(Carried { aim, ..carried.clone() });

    Ok(match (key, carried.aim, cards) {
        (Key::Left, Aim::Card(on), Some(cards)) => {
            let Ok(before) = cards.step(on, Step::Back);

            (aimed(Aim::Card(before)), Chose::Stay)
        }
        (Key::Right, Aim::Card(on), Some(cards)) => {
            let Ok(after) = cards.step(on, Step::Forward);

            (aimed(Aim::Card(after)), Chose::Stay)
        }
        (Key::Up, _, _) => (aimed(Aim::Close), Chose::Stay),
        (Key::Down, Aim::Close, _) => (aimed(Aim::Card(carried.home)), Chose::Stay),
        (Key::Choose, aim, _) => {
            let Ok(drop) = aim_dropped(overview, carried, aim);
            let Ok(asked) = carried_to(&carried.address, drop);

            (Choosing::Window(carried.was), asked)
        }
        (Key::Back, _, _) => (Choosing::Window(carried.was), Chose::Stay),
        (Key::Left | Key::Right, Aim::Close, _)
        | (Key::Left | Key::Right, Aim::Card(_), None)
        | (Key::Down, Aim::Card(_), _)
        | (Key::More, _, _) => (Choosing::Carrying(carried.clone()), Chose::Stay),
    })
}

fn aim_dropped(overview: &Overview, carried: &Carried, aim: Aim) -> Result<Drop, Never> {
    Ok(match aim {
        Aim::Close => Drop::Close,
        Aim::Card(on) => {
            let Ok(position) = index(on);

            match overview.cards.get(position).map(|card| card.target) {
                Some(Target::New) => Drop::New,
                Some(Target::Place(id)) => match id == carried.from {
                    true => Drop::Back,
                    false => Drop::Onto(id),
                },
                None => Drop::Back,
            }
        }
    })
}

fn carried_to(address: &str, drop: Drop) -> Result<Chose, Never> {
    let Ok(asked) = carried(address, drop);

    Ok(match asked {
        Some(lua) => Chose::Ask(lua),
        None => Chose::Stay,
    })
}

fn centre(frame: Frame) -> Result<Point<f64>, Never> {
    Ok(Point { x: frame.at.x + frame.size.width / 2.0, y: frame.at.y + frame.size.height / 2.0 })
}

pub fn held_by_the_pad(overview: &Overview, choosing: &Choosing) -> Result<Option<Dragged>, Never> {
    let carried = match choosing {
        Choosing::Carrying(carried) => carried,
        Choosing::Untouched | Choosing::Window(_) => return Ok(None),
    };

    let over = match carried.aim {
        Aim::Close => Some(overview.close),
        Aim::Card(on) => {
            let Ok(position) = index(on);

            overview.cards.get(position).map(|card| card.frame)
        }
    };

    let small = Size { width: carried.frame.size.width * CARRIED, height: carried.frame.size.height * CARRIED };
    let Ok(middle) = centre(carried.frame);
    let frame = Frame { at: Point { x: middle.x - small.width / 2.0, y: middle.y - small.height / 2.0 }, size: small };

    Ok(over.map(|over| {
        let Ok(at) = centre(over);

        Dragged { address: carried.address.clone(), frame, stroke: Stroke { from: middle, at } }
    }))
}

pub fn chosen_window(overview: &Overview, choosing: &Choosing) -> Result<Option<String>, Never> {
    let at = match choosing {
        Choosing::Window(at) => *at,
        Choosing::Untouched | Choosing::Carrying(_) => return Ok(None),
    };

    let Ok(windows) = every_window(overview);
    let Ok(ring) = Ring::of(&windows);

    Ok(ring.and_then(|ring| {
        let Ok(here) = ring.at(at);
        let Ok(position) = index(here);

        windows.get(position).map(|standing| standing.tile.address.clone())
    }))
}

pub trait Measure {
    fn measure(&self, said: &str, style: TextStyle, width: u32) -> Result<Size<u32>, Never>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dragged {
    pub address: String,
    pub frame: Frame,
    pub stroke: Stroke,
}

pub struct Scene<'a> {
    pub overview: &'a Overview,
    pub front: Option<PlaceId>,
    pub held: Option<&'a Dragged>,
    pub chosen: Option<&'a str>,
    pub pictures: &'a BTreeMap<String, Pixels>,
    pub wearing: &'a Wearing,
}

pub fn render(drawn: &Scene<'_>, measure: &dyn Measure) -> Result<Vec<Shape>, Never> {
    let ground = Frame { at: Point { x: 0.0, y: 0.0 }, size: drawn.overview.room };
    let Ok(night) = ground.panel(0, drawn.wearing.night, Edge::None);
    let mut shapes = vec![Shape::Panel(night)];

    for card in &drawn.overview.cards {
        let Ok(more) = render_card(drawn, card, measure);

        shapes.extend(more);
    }

    let Ok(held) = render_held(drawn, measure);

    shapes.extend(held);

    Ok(shapes)
}

fn render_card(drawn: &Scene<'_>, card: &Card, measure: &dyn Measure) -> Result<Vec<Shape>, Never> {
    let wearing = drawn.wearing;
    let in_front = match (card.target, drawn.front) {
        (Target::Place(id), Some(front)) => id == front,
        (Target::Place(_) | Target::New, None) | (Target::New, Some(_)) => false,
    };
    let edge = match in_front {
        true => wearing.pink,
        false => wearing.edge,
    };
    let Ok(panel) = card.frame.panel(CARD_ROUND, wearing.ground, Edge::Of { wide: CARD_EDGE, color: edge });
    let mut shapes = vec![Shape::Panel(panel)];

    for tile in &card.tiles {
        let lifted = drawn.held.is_some_and(|held| held.address == tile.address);

        match lifted {
            true => {},
            false => {
                let edge = match drawn.chosen == Some(tile.address.as_str()) {
                    true => Edge::Of { wide: CARD_EDGE, color: wearing.pink },
                    false => Edge::Of { wide: 1, color: wearing.soft },
                };
                let Ok(drawn_tile) = render_tile(drawn, tile.frame, tile, edge, measure);

                shapes.extend(drawn_tile);
            }
        }
    }

    let Ok(named) = render_name(card, wearing, measure);

    shapes.extend(named);

    Ok(shapes)
}

fn render_tile(drawn: &Scene<'_>, frame: Frame, tile: &Tile, edge: Edge, measure: &dyn Measure) -> Result<Vec<Shape>, Never> {
    let wearing = drawn.wearing;
    let Ok(panel) = frame.panel(TILE_ROUND, wearing.panel, edge);
    let inside = match drawn.pictures.get(&tile.address) {
        Some(pixels) => {
            let Ok(picture) = pictured(frame, pixels);

            picture
        }
        None => {
            let Ok(label) = centred(frame, &tile.class, TextStyle::Caption, wearing.text, measure);

            label
        }
    };

    Ok(vec![Shape::Panel(panel), inside])
}

fn pictured(frame: Frame, pixels: &Pixels) -> Result<Shape, Never> {
    let room = Size { width: frame.size.width - 2.0 * f64::from(CARD_EDGE), height: frame.size.height - 2.0 * f64::from(CARD_EDGE) };
    let wide = f64::from(pixels.width.max(1));
    let tall = f64::from(pixels.height.max(1));
    let scale = (room.width / wide).min(room.height / tall);
    let size = Size { width: wide * scale, height: tall * scale };
    let Ok(x) = whole_i32(frame.at.x + (frame.size.width - size.width) / 2.0);
    let Ok(y) = whole_i32(frame.at.y + (frame.size.height - size.height) / 2.0);
    let Ok(width) = whole_u32(size.width);
    let Ok(height) = whole_u32(size.height);

    Ok(Shape::Picture(Picture { at: Point { x, y }, size: Size { width, height }, pixels: pixels.clone() }))
}

fn render_name(card: &Card, wearing: &Wearing, measure: &dyn Measure) -> Result<Vec<Shape>, Never> {
    let under = Frame {
        at: Point { x: card.frame.at.x, y: card.frame.at.y + card.frame.size.height },
        size: Size { width: card.frame.size.width, height: NAME_TALL },
    };

    Ok(match card.target {
        Target::Place(PlaceId(id)) => {
            let Ok(name) = centred(under, &format!("Desktop {id}"), TextStyle::Callout, wearing.text, measure);

            vec![name]
        }
        Target::New => {
            let Ok(plus) = centred(card.frame, PLUS, TextStyle::Title, wearing.soft, measure);
            let Ok(name) = centred(under, NEW, TextStyle::Callout, wearing.soft, measure);

            vec![plus, name]
        }
    })
}

fn render_held(drawn: &Scene<'_>, measure: &dyn Measure) -> Result<Vec<Shape>, Never> {
    let held = match drawn.held {
        Some(held) => held,
        None => return Ok(Vec::new()),
    };
    let wearing = drawn.wearing;
    let close = drawn.overview.close;
    let Ok(band) = close.panel(CARD_ROUND, wearing.coral, Edge::None);
    let Ok(said) = centred(close, CLOSE, TextStyle::Headline, wearing.night, measure);
    let Ok(by) = held.stroke.by();
    let Ok(following) = held.frame.moved(by);
    let mut shapes = vec![Shape::Panel(band), said];
    let tile = drawn.overview.cards.iter().flat_map(|card| card.tiles.iter()).find(|tile| tile.address == held.address);

    match tile {
        Some(tile) => {
            let Ok(carried) = render_tile(drawn, following, tile, Edge::Of { wide: CARD_EDGE, color: wearing.pink }, measure);

            shapes.extend(carried);
        }
        None => {},
    }

    Ok(shapes)
}

fn centred(
    frame: Frame,
    said: &str,
    style: TextStyle,
    ink: console_core_color::Oklch,
    measure: &dyn Measure,
) -> Result<Shape, Never> {
    let Ok(font) = style.font();
    let Ok(weight) = style.weight();
    let Ok(room) = whole_u32(frame.size.width);
    let Ok(label) = measure.measure(said, style, room);
    let across = frame.at.x + (frame.size.width - f64::from(label.width)) / 2.0;
    let down = frame.at.y + (frame.size.height - f64::from(label.height)) / 2.0;
    let Ok(x) = whole_i32(across);
    let Ok(y) = whole_i32(down);

    Ok(Shape::Text(Text { at: Point { x, y }, width: label.width, said: said.to_string(), weight, font, ink }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_compositor::{Filling, Floating, Pinned};

    const SCREEN: Size<u32> = Size { width: 1024, height: 640 };

    fn window(address: &str, workspace: i64, at: (i64, i64), size: (i64, i64)) -> Result<Window, Never> {
        Ok(Window {
            address: address.to_string(),
            title: String::new(),
            first_class: format!("app-{address}"),
            first_title: String::new(),
            workspace,
            workspace_named: workspace.to_string(),
            monitor: Some(0),
            floating: Floating::No,
            pinned: Pinned::No,
            filling: Filling::None,
            at,
            size,
            pid: 1,
        })
    }

    fn laid_out() -> Result<Overview, Never> {
        let open: Result<Vec<Window>, Never> = [
            ("b", 1, (512, 40), (512, 600)),
            ("a", 1, (0, 40), (512, 600)),
            ("c", 3, (0, 40), (1024, 600)),
            ("scratch", -98, (0, 40), (1024, 600)),
        ]
        .into_iter()
        .map(|(address, workspace, at, size)| window(address, workspace, at, size))
        .collect();
        let Ok(open) = open;
        let Ok(grouped) = places(open);

        overview(&grouped, SCREEN)
    }

    fn middle(frame: Frame) -> Result<Point<f64>, Never> {
        Ok(Point { x: frame.at.x + frame.size.width / 2.0, y: frame.at.y + frame.size.height / 2.0 })
    }

    fn card(laid: &Overview, target: Target) -> Result<Option<Card>, Never> {
        Ok(laid.cards.iter().find(|card| card.target == target).cloned())
    }

    #[test]
    fn a_card_for_each_place_with_a_window_and_one_more_for_a_new_place() {
        let Ok(laid) = laid_out();
        let targets: Vec<Target> = laid.cards.iter().map(|card| card.target).collect();

        assert_eq!(targets, [Target::Place(PlaceId(1)), Target::Place(PlaceId(3)), Target::New]);
    }

    #[test]
    fn windows_stand_in_a_card_the_way_they_stand_on_the_screen() -> Result<(), &'static str> {
        let Ok(laid) = laid_out();
        let Ok(first) = card(&laid, Target::Place(PlaceId(1)));
        let first = first.ok_or("no card for that target")?;
        let order: Vec<&str> = first.tiles.iter().map(|tile| tile.address.as_str()).collect();

        assert_eq!(order, ["a", "b"]);
        assert!(first.tiles.iter().all(|tile| tile.frame.at.x >= first.frame.at.x));
        assert_eq!(first.dividers.len(), 1);

        Ok(())
    }

    #[test]
    fn every_card_fits_on_the_screen_and_below_the_close_box() {
        let Ok(laid) = laid_out();

        for card in &laid.cards {
            for tile in &card.tiles {
                assert!(tile.frame.at.y + tile.frame.size.height <= card.frame.at.y + card.frame.size.height + 0.5);
            }

            assert!(card.frame.at.x >= 0.0);
            assert!(card.frame.at.y >= laid.close.at.y + laid.close.size.height);
            assert!(card.frame.at.x + card.frame.size.width <= 1024.0);
            assert!(card.frame.at.y + card.frame.size.height + NAME_TALL <= 640.0);
        }
    }

    #[test]
    fn many_places_wrap_into_rows_and_still_fit() {
        let open: Result<Vec<Window>, Never> = (1..=7).map(|id| window(&id.to_string(), id, (0, 40), (1024, 600))).collect();
        let Ok(open) = open;
        let Ok(grouped) = places(open);
        let Ok(laid) = overview(&grouped, SCREEN);

        assert_eq!(laid.cards.len(), 8);

        for card in &laid.cards {
            assert!(card.frame.size.width > 0.0);
            assert!(card.frame.at.y + card.frame.size.height + NAME_TALL <= 640.0);
        }
    }

    #[test]
    fn a_window_dropped_on_another_card_goes_to_that_place() -> Result<(), &'static str> {
        let Ok(laid) = laid_out();
        let Ok(other) = card(&laid, Target::Place(PlaceId(3)));
        let other = other.ok_or("no card for that target")?;

        let Ok(centre) = middle(other.frame);

        assert_eq!(dropped(&laid, PlaceId(1), centre), Ok(Drop::Onto(PlaceId(3))));

        Ok(())
    }

    #[test]
    fn a_window_dropped_on_its_own_card_or_on_nothing_goes_back() -> Result<(), &'static str> {
        let Ok(laid) = laid_out();
        let Ok(own) = card(&laid, Target::Place(PlaceId(1)));
        let own = own.ok_or("no card for that target")?;

        let Ok(centre) = middle(own.frame);

        assert_eq!(dropped(&laid, PlaceId(1), centre), Ok(Drop::Back));
        assert_eq!(dropped(&laid, PlaceId(1), Point { x: 2.0, y: 638.0 }), Ok(Drop::Back));

        Ok(())
    }

    #[test]
    fn a_window_dropped_on_the_last_card_gets_a_place_of_its_own() -> Result<(), &'static str> {
        let Ok(laid) = laid_out();
        let Ok(new) = card(&laid, Target::New);
        let new = new.ok_or("no card for that target")?;

        let Ok(centre) = middle(new.frame);

        assert_eq!(dropped(&laid, PlaceId(1), centre), Ok(Drop::New));

        Ok(())
    }

    #[test]
    fn a_window_dropped_on_the_close_box_closes() {
        let Ok(laid) = laid_out();
        let Ok(centre) = middle(laid.close);

        assert_eq!(dropped(&laid, PlaceId(1), centre), Ok(Drop::Close));
        assert_eq!(dropped(&laid, PlaceId(1), Point { x: 4.0, y: laid.close.at.y + laid.close.size.height + 4.0 }), Ok(Drop::Close));
        assert_eq!(
            carried("0x1", Drop::Close),
            Ok(Some(r#"hl.dsp.window.close({ window = "address:0x1" })"#.to_string()))
        );
    }

    #[test]
    fn a_press_on_the_line_between_two_windows_is_the_divider_not_either_window() -> Result<(), &'static str> {
        let Ok(laid) = laid_out();
        let Ok(first) = card(&laid, Target::Place(PlaceId(1)));
        let first = first.ok_or("no card for that target")?;
        let line = first.dividers.first().ok_or("two windows side by side have a line between them")?;
        let Ok(centre) = middle(first.frame);
        let on_line = Point { x: line.x + 2.0, y: centre.y };

        assert_eq!(hit(&laid, on_line), Ok(Hit::Divider { left: "a".to_string() }));

        Ok(())
    }

    #[test]
    fn a_press_on_a_window_is_that_window() -> Result<(), &'static str> {
        let Ok(laid) = laid_out();
        let Ok(first) = card(&laid, Target::Place(PlaceId(1)));
        let first = first.ok_or("no card for that target")?;
        let right = first.tiles.get(1).ok_or("two tiles")?;
        let Ok(centre) = middle(right.frame);

        let Ok(found) = hit(&laid, centre);

        assert!(matches!(found, Hit::Tile { ref address, from: PlaceId(1), .. } if address == "b"));

        Ok(())
    }

    #[test]
    fn moving_a_window_names_it_and_does_not_follow_it() {
        assert_eq!(
            carried("0x1", Drop::Onto(PlaceId(3))),
            Ok(Some(r#"hl.dsp.window.move({ window = "address:0x1", workspace = "3", follow = false })"#.to_string()))
        );
        assert_eq!(carried("0x1", Drop::Back), Ok(None));
    }

    #[test]
    fn a_divider_dragged_in_a_card_resizes_by_the_distance_on_the_screen() {
        let Ok(laid) = laid_out();
        let by = 10.0 * laid.shrink;

        assert_eq!(
            resized(&laid, "0x1", by),
            Ok(r#"hl.dsp.window.resize({ window = "address:0x1", x = 10, y = 0, relative = true })"#.to_string())
        );
    }

    #[test]
    fn a_finger_pulled_up_off_the_bottom_is_a_swipe_up_and_a_tap_is_not() {
        let up = Stroke { from: Point { x: 500.0, y: 638.0 }, at: Point { x: 505.0, y: 560.0 } };
        let tap = Stroke { from: Point { x: 500.0, y: 638.0 }, at: Point { x: 503.0, y: 634.0 } };
        let along = Stroke { from: Point { x: 100.0, y: 638.0 }, at: Point { x: 400.0, y: 560.0 } };

        assert_eq!(up.swipe(), Ok(Swipe::Up));
        assert_eq!(tap.swipe(), Ok(Swipe::Neither));
        assert_eq!(tap.wandered(), Ok(Wandered::No));
        assert_eq!(along.swipe(), Ok(Swipe::Neither));
    }

    #[test]
    fn the_pad_walks_the_windows_goes_to_one_and_puts_the_overview_away() {
        let Ok(laid) = laid_out();
        let Ok((first, _)) = chose(&laid, &Choosing::Untouched, Key::Right);
        let Ok((second, stay)) = chose(&laid, &first, Key::Right);
        let Ok((_, went)) = chose(&laid, &second, Key::Choose);

        assert_eq!(first, Choosing::Window(0));
        assert_eq!(stay, Chose::Stay);
        assert_eq!(chosen_window(&laid, &second), Ok(Some("b".to_string())));
        assert_eq!(went, Chose::AskAndPutAway(r#"hl.dsp.focus({ window = "address:b" })"#.to_string()));
        assert_eq!(chose(&laid, &Choosing::Window(2), Key::Right).map(|(to, _)| to), Ok(Choosing::Window(0)));
    }

    #[test]
    fn the_pad_carries_a_window_to_another_card_and_drops_it_there() {
        let Ok(laid) = laid_out();
        let Ok((held, _)) = chose(&laid, &Choosing::Window(0), Key::More);
        let Ok((aimed, _)) = chose(&laid, &held, Key::Right);
        let Ok((back, dropped_there)) = chose(&laid, &aimed, Key::Choose);

        assert_eq!(back, Choosing::Window(0));
        assert_eq!(
            dropped_there,
            Chose::Ask(r#"hl.dsp.window.move({ window = "address:a", workspace = "3", follow = false })"#.to_string())
        );
        assert!(matches!(held_by_the_pad(&laid, &aimed), Ok(Some(Dragged { ref address, .. })) if address == "a"));
    }

    #[test]
    fn the_pad_closes_a_window_by_carrying_it_up_and_b_puts_it_back() {
        let Ok(laid) = laid_out();
        let Ok((held, _)) = chose(&laid, &Choosing::Window(1), Key::More);
        let Ok((up, _)) = chose(&laid, &held, Key::Up);
        let Ok((_, closed)) = chose(&laid, &up, Key::Choose);
        let Ok((put_back, nothing)) = chose(&laid, &up, Key::Back);

        assert_eq!(closed, Chose::Ask(r#"hl.dsp.window.close({ window = "address:b" })"#.to_string()));
        assert_eq!((put_back, nothing), (Choosing::Window(1), Chose::Stay));
    }

    #[test]
    fn dropping_a_window_where_it_already_is_asks_nothing() {
        let Ok(laid) = laid_out();
        let Ok((held, _)) = chose(&laid, &Choosing::Window(0), Key::More);

        assert_eq!(chose(&laid, &held, Key::Choose), Ok((Choosing::Window(0), Chose::Stay)));
    }

    #[test]
    fn tapping_the_new_card_goes_to_an_empty_desktop() {
        assert_eq!(gone_to(&Hit::Card(Target::New)), Ok(Some(r#"hl.dsp.focus({ workspace = "empty" })"#.to_string())));
    }
}
