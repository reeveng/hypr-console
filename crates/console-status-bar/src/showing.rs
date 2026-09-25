//! The bar, in numbers: how deep it is, what stands along it, and what a thumb
//! on it lands on.
//!
//! **Every measurement is a share of the em.** The stylesheet held them as
//! points -- 38 deep, 15 either side of what a slot says, 2 of margin -- and
//! they were right for one size of type and silently wrong at any other. So the
//! bar's depth is a share of the em every surface sets its words in, and
//! everything inside it is a share of the bar's depth, which is what those
//! numbers always were: 15 of padding was never fifteen of anything, it was a bit
//! under four tenths of the depth.
//!
//! It was a share of the screen's shorter side for a while, so that a denser
//! screen gave a bar the same size to a thumb. That kept the bar and the words
//! in it answering two different questions, and the words won every time
//! someone asked for bigger type: the text grew and the bar it stood in did not.
//! A bar is where things are read, so it is as deep as what is read in it
//! needs, and making the type bigger makes the bar bigger with it.
//!
//! At an em of 18 they come out at exactly the points the stylesheet held, which
//! is how the shares were chosen and is what
//! `the_shares_land_on_the_numbers_the_stylesheet_held` keeps true.
//!
//! What the shape buys is a second knob: the bar as a whole is `DEEP`, and
//! anything in it can be nudged against the bar without moving the rest.
//!
//! **A turned screen is a shorter bar, and something has to go.** The depth
//! holds because the type does not turn with the screen -- but neither does
//! anything else in here, and the edge the bar runs along is the panel's long
//! one one way up and its short one the other. So the same slots, at the same
//! size to a thumb, are asked to stand in about two thirds of the room, and
//! what they did instead was stand on top of each other: the clock painted
//! across the music, which is a bar that looks broken rather than a bar that is
//! full.
//!
//! [`shed`] is the order things go in, and it is an order rather than a
//! shrink, because everything here is already the size a thumb needs. The
//! workspaces no one is on go first -- the shoulders are what move between
//! windows on this device and the bar is where you read which one you are on,
//! so the one in front is the whole of what that row has to say when there is
//! no room for the rest. Then the readings drop what is written beside them
//! and keep their icon, which is the other half of a slot that was never the
//! half being read at a glance. If it still does not fit the middle is put
//! against the group on its right rather than in the center of the screen, so
//! what a short bar looks like is a clock off-center and not a clock over an
//! icon.
//!
//! **The strip is the last two rows of this surface rather than a bar of its
//! own.** waybar needed two windows because a module cannot be given a width
//! and a fill, so the strip was a second bar with a gradient standing in for a
//! number, one stylesheet rule per whole per cent, and a `min-width` written
//! into a file `console-scale` had to rewrite at every login because a gradient
//! measures in percentages of a box and the box was only as wide as its
//! contents. None of that survives having a surface: the room comes back from
//! the compositor, the fill is a panel that many points across, and the number
//! is the number.
//!
//! **Nothing here measures.** How wide a run of words comes out is Pango's to
//! say and it is the one thing that needs a font in the room, so it arrives
//! measured and this places against it. That is what lets the whole arrangement
//! -- where each slab lands, what a thumb hits, whether lighting one moves the
//! rest -- be asserted with no screen anywhere near it.

use console_core_color::Oklch;
use console_core_color::palette::{PaletteError, named};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_fonts::{EM, Font, ICONS, TextStyle};
use console_core_shapes::{Covers, Edge, Panel, Round, Shape, Weight, Text};

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use crate::reading::{Tone, StatusItem};

pub use console_onscreen::BAR as WHO;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Share(u32);

impl Share {
    pub const WHOLE: NonZeroU32 = match NonZeroU32::new(1000) {
        Some(whole) => whole,
        None => NonZeroU32::MIN,
    };

    pub fn of(thousandths: u32) -> Result<Share, Never> {
        Ok(Share(thousandths))
    }

    pub fn thousandths(self) -> Result<u32, Never> {
        Ok(self.0)
    }

    pub fn spent(self, of: u32) -> Result<u32, Never> {
        let half = Share::WHOLE.get().div_ceil(2);

        Ok(of.saturating_mul(self.0).saturating_add(half) / Share::WHOLE)
    }
}

pub const DEEP: Share = Share(2111);

pub const THIN: Share = Share(111);

pub const PAD: Share = Share(395);

pub const MARGIN: Share = Share(53);

pub const ROUND: Share = Share(105);

pub const BETWEEN: Share = Share(105);

pub const ICON: Share = Share(579);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Icon,
    Clock,
    Reading,
    Small,
}

impl Face {
    pub fn weight(self) -> Result<Weight, Never> {
        Ok(match self {
            Face::Clock => Weight::Bold,
            Face::Icon | Face::Reading | Face::Small => Weight::Plain,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fitting {
    pub deep: u32,
    pub thin: u32,
    pub pad: u32,
    pub margin: u32,
    pub round: u32,
    pub between: u32,
}

impl Fitting {
    pub fn of(em: u32) -> Result<Fitting, Never> {
        let Ok(deep) = DEEP.spent(em);
        let Ok(thin) = THIN.spent(em);
        let Ok(pad) = PAD.spent(deep);
        let Ok(margin) = MARGIN.spent(deep);
        let Ok(round) = ROUND.spent(deep);
        let Ok(between) = BETWEEN.spent(deep);

        Ok(Fitting { deep, thin, pad, margin, round, between })
    }

    pub fn of_em() -> Result<Fitting, Never> {
        let Ok(em) = fitted::<i32, u32>(EM);

        Fitting::of(em)
    }

    pub fn height(&self) -> Result<u32, Never> {
        Ok(self.deep.saturating_add(self.thin))
    }

    pub fn font(&self, face: Face) -> Result<Font, Never> {
        match face {
            Face::Icon => {
                let Ok(tall) = ICON.spent(self.deep);

                Ok(Font { family: ICONS.to_string(), height: tall })
            }
            Face::Clock | Face::Reading => TextStyle::Callout.font(),
            Face::Small => TextStyle::Caption.font(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub face: Face,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lit {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarAction {
    Calendar,
    Launcher,
    Keyboard,
    Workspace(i64),
    Settings(StatusItem),
    Music,
    Notifications,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub spans: Vec<Span>,
    pub tone: Tone,
    pub lit: Lit,
    pub action: Option<BarAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measured {
    pub slot: Slot,
    pub runs: Vec<Size<u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filling {
    At(u16),
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub left: Vec<Slot>,
    pub middle: Vec<Slot>,
    pub right: Vec<Slot>,
    pub filling: Filling,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bar {
    pub left: Vec<Measured>,
    pub middle: Vec<Measured>,
    pub right: Vec<Measured>,
    pub filling: Filling,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitRegion {
    pub action: BarAction,
    pub panel: Panel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub shapes: Vec<Shape>,
    pub room: Size<u32>,
    pub touching: Vec<HitRegion>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wearing {
    pub ground: Oklch,
    pub text: Oklch,
    pub soft: Oklch,
    pub pink: Oklch,
    pub night: Oklch,
    pub butter: Oklch,
    pub coral: Oklch,
    pub leaf: Oklch,
    pub fill: Oklch,
}

pub const SPENDS: [&str; 9] =
    ["ground", "text", "soft", "pink", "night", "butter", "coral", "leaf", "fill"];

impl Wearing {
    pub fn out_of(spent: &BTreeMap<String, String>) -> Result<Wearing, PaletteError> {
        let ground = named(spent, "ground")?;
        let text = named(spent, "text")?;
        let soft = named(spent, "soft")?;
        let pink = named(spent, "pink")?;
        let night = named(spent, "night")?;
        let butter = named(spent, "butter")?;
        let coral = named(spent, "coral")?;
        let leaf = named(spent, "leaf")?;
        let fill = named(spent, "fill")?;

        Ok(Wearing { ground, text, soft, pink, night, butter, coral, leaf, fill })
    }

    fn ink(&self, tone: Tone, lit: Lit) -> Result<Oklch, Never> {
        Ok(match lit {
            Lit::Yes => self.night,
            Lit::No => match tone {
                Tone::Plain => self.text,
                Tone::Secondary => self.soft,
                Tone::Pressed => self.pink,
                Tone::Low => self.butter,
                Tone::Error => self.coral,
                Tone::Well => self.leaf,
            },
        })
    }
}

pub fn slab(measured: &Measured, fitting: Fitting) -> Result<Size<u32>, Never> {
    let said = measured.runs.iter().map(|run| run.width).fold(0_u32, u32::saturating_add);
    let Ok(many) = fitted::<_, u32>(measured.runs.len());
    let gaps = fitting.between.saturating_mul(many.saturating_sub(1));
    let wide = said.saturating_add(gaps).saturating_add(fitting.pad.saturating_mul(2));

    Ok(Size { width: wide, height: fitting.deep.saturating_sub(fitting.margin.saturating_mul(2)) })
}

fn advance(measured: &Measured, fitting: Fitting) -> Result<u32, Never> {
    let Ok(slab) = slab(measured, fitting);

    Ok(slab.width.saturating_add(fitting.margin.saturating_mul(2)))
}

pub fn group(slots: &[Measured], fitting: Fitting) -> Result<u32, Never> {
    let mut wide = 0_u32;

    for one in slots {
        let Ok(step) = advance(one, fitting);

        wide = wide.saturating_add(step);
    }

    Ok(wide)
}

pub fn filled(wide: u32, thousandths: u16) -> Result<u32, Never> {
    let held = u32::from(thousandths).min(Share::WHOLE.get());

    Ok(wide.saturating_mul(held) / Share::WHOLE)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Room {
    Enough,
    Short,
}

pub fn needs(bar: &Bar, fitting: Fitting) -> Result<u32, Never> {
    let Ok(left) = group(&bar.left, fitting);
    let Ok(middle) = group(&bar.middle, fitting);
    let Ok(right) = group(&bar.right, fitting);

    Ok(left.saturating_add(middle).saturating_add(right))
}

pub fn holds(bar: &Bar, fitting: Fitting, wide: u32) -> Result<Room, Never> {
    let Ok(needs) = needs(bar, fitting);

    Ok(match needs > wide {
        true => Room::Short,
        false => Room::Enough,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Retain {
    Yes,
    No,
}

fn one_of_the_workspaces_no_one_is_on(one: &Measured) -> Result<Retain, Never> {
    let action = match one.slot.action {
        Some(action) => action,
        None => return Ok(Retain::Yes),
    };

    Ok(match action {
        BarAction::Workspace(_) => match one.slot.lit {
            Lit::Yes => Retain::Yes,
            Lit::No => Retain::No,
        },
        BarAction::Calendar
        | BarAction::Launcher
        | BarAction::Keyboard
        | BarAction::Settings(_)
        | BarAction::Music
        | BarAction::Notifications => Retain::Yes,
    })
}

fn only_the_workspace_in_front(slots: &[Measured]) -> Result<Vec<Measured>, Never> {
    let mut kept = Vec::new();

    for one in slots {
        let Ok(keep) = one_of_the_workspaces_no_one_is_on(one);

        match keep {
            Retain::Yes => kept.push(one.clone()),
            Retain::No => {},
        }
    }

    Ok(kept)
}

fn the_icon_alone(one: &Measured) -> Result<Measured, Never> {
    let mut spans = Vec::new();
    let mut runs = Vec::new();

    for (one, run) in one.slot.spans.iter().zip(&one.runs) {
        match one.face {
            Face::Small => {},
            Face::Icon | Face::Clock | Face::Reading => {
                spans.push(one.clone());
                runs.push(*run);
            }
        }
    }

    Ok(Measured { slot: Slot { spans, ..one.slot.clone() }, runs })
}

fn icons_alone(slots: &[Measured]) -> Result<Vec<Measured>, Never> {
    let mut kept = Vec::new();

    for one in slots {
        let Ok(alone) = the_icon_alone(one);

        kept.push(alone);
    }

    Ok(kept)
}

pub fn shed(bar: &Bar, fitting: Fitting, wide: u32) -> Result<Bar, Never> {
    let Ok(room) = holds(bar, fitting, wide);

    match room {
        Room::Enough => return Ok(bar.clone()),
        Room::Short => {},
    }

    let Ok(left) = only_the_workspace_in_front(&bar.left);
    let folded = Bar { left, ..bar.clone() };
    let Ok(room) = holds(&folded, fitting, wide);

    match room {
        Room::Enough => return Ok(folded),
        Room::Short => {},
    }

    let Ok(right) = icons_alone(&folded.right);

    Ok(Bar { right, ..folded })
}

pub fn along(bar: &Bar, wearing: &Wearing, room: Size<u32>) -> Result<Rendered, Never> {
    let Ok(fitting) = Fitting::of_em();
    let Ok(tall) = fitting.height();
    let wide = room.width;
    let Ok(bar) = shed(bar, fitting, wide);
    let mut shapes = vec![Shape::Panel(Panel {
        at: Point { x: 0, y: 0 },
        size: Size { width: wide, height: tall },
        round: Round(0),
        fill: wearing.ground,
        edge: Edge::None,
    })];
    let mut touching = Vec::new();

    let Ok(along_the_left) = group(&bar.left, fitting);
    let Ok(middle) = group(&bar.middle, fitting);
    let Ok(right) = group(&bar.right, fitting);
    let free = wide.saturating_sub(right).saturating_sub(middle);
    let halfway = wide.saturating_sub(middle).div_ceil(2);
    let Ok(center) = fitted::<u32, i32>(halfway.min(free).max(along_the_left.min(free)));
    let Ok(ends) = fitted::<u32, i32>(wide.saturating_sub(right));

    for (slots, from) in [(&bar.left, 0), (&bar.middle, center), (&bar.right, ends)] {
        let Ok(()) = laid(slots, from, fitting, wearing, &mut shapes, &mut touching);
    }

    match bar.filling {
        Filling::None => {}
        Filling::At(thousandths) => {
            let Ok(filled) = filled(wide, thousandths);
            let Ok(down) = fitted::<u32, i32>(fitting.deep);

            shapes.push(Shape::Panel(Panel {
                at: Point { x: 0, y: down },
                size: Size { width: filled, height: fitting.thin },
                round: Round(0),
                fill: wearing.fill,
                edge: Edge::None,
            }));
        }
    }

    Ok(Rendered { shapes, room: Size { width: wide, height: tall }, touching })
}

fn laid(
    slots: &[Measured],
    from: i32,
    fitting: Fitting,
    wearing: &Wearing,
    shapes: &mut Vec<Shape>,
    touching: &mut Vec<HitRegion>,
) -> Result<(), Never> {
    let Ok(margin) = fitted::<u32, i32>(fitting.margin);
    let Ok(pad) = fitted::<u32, i32>(fitting.pad);
    let mut across = from;

    for one in slots {
        let Ok(size) = slab(one, fitting);
        let panel = Panel {
            at: Point { x: across.saturating_add(margin), y: margin },
            size,
            round: Round(fitting.round),
            fill: wearing.pink,
            edge: Edge::None,
        };

        match one.slot.lit {
            Lit::Yes => shapes.push(Shape::Panel(panel)),
            Lit::No => {},
        }

        match one.slot.action {
            Some(action) => touching.push(HitRegion { action, panel }),
            None => {},
        }

        let Ok(ink) = wearing.ink(one.slot.tone, one.slot.lit);
        let mut at = panel.at.x.saturating_add(pad);

        for (span, run) in one.slot.spans.iter().zip(&one.runs) {
            let Ok(font) = fitting.font(span.face);
            let Ok(weight) = span.face.weight();
            let Ok(down) = centered(panel, *run);

            shapes.push(Shape::Text(Text {
                at: Point { x: at, y: down },
                width: run.width,
                said: span.text.clone(),
                weight,
                font,
                ink,
            }));

            let Ok(step) = fitted::<u32, i32>(run.width.saturating_add(fitting.between));

            at = at.saturating_add(step);
        }

        let Ok(step) = advance(one, fitting);
        let Ok(step) = fitted::<u32, i32>(step);

        across = across.saturating_add(step);
    }

    Ok(())
}

fn centered(panel: Panel, run: Size<u32>) -> Result<i32, Never> {
    let Ok(spare) = fitted::<u32, i32>(panel.size.height.saturating_sub(run.height).div_ceil(2));

    Ok(panel.at.y.saturating_add(spare))
}

impl Rendered {
    pub fn on(&self, at: Point<i32>) -> Result<Option<BarAction>, Never> {
        Ok(self.touching.iter().find_map(|touching| match touching.panel.covers(at) {
            Ok(Covers::Yes) => Some(touching.action),
            Ok(Covers::No) | Err(_) => None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPENT: &str = "ground=2b2029\ntext=f7e7f3\nsoft=e5cde0\npink=ff9ecb\nnight=231b26\nbutter=f2d692\ncoral=ff8f8f\nleaf=9fd88f\nfill=723b5f";

    const SCREEN: Size<u32> = Size { width: 1024, height: 640 };

    const ACROSS: u32 = SCREEN.width;

    fn fitting() -> Fitting {
        let Ok(fitting) = Fitting::of(18);

        fitting
    }

    pub(super) fn wearing() -> Wearing {
        let Ok(spent) = console_core_color::palette::read(SPENT);

        match Wearing::out_of(&spent) {
            Ok(wearing) => wearing,
            Err(why) => panic!("a whole palette should dress a bar: {why}"),
        }
    }

    fn slot(spans: Vec<Span>, action: Option<BarAction>) -> Measured {
        let runs = spans
            .iter()
            .map(|one| match one.face {
                Face::Icon => Size { width: 14, height: 22 },
                Face::Clock | Face::Reading => Size { width: 40, height: 18 },
                Face::Small => Size { width: 24, height: 13 },
            })
            .collect();

        Measured {
            slot: Slot { spans, tone: Tone::Plain, lit: Lit::No, action },
            runs,
        }
    }

    fn icon(icon: &str, action: Option<BarAction>) -> Measured {
        slot(vec![Span { text: icon.to_string(), face: Face::Icon }], action)
    }

    fn lit(measured: Measured) -> Measured {
        Measured { slot: Slot { lit: Lit::Yes, ..measured.slot }, ..measured }
    }

    fn bar(left: Vec<Measured>, middle: Vec<Measured>, right: Vec<Measured>) -> Bar {
        Bar { left, middle, right, filling: Filling::None }
    }

    fn drawn(bar: &Bar) -> Rendered {
        let Ok(drawn) = along(bar, &wearing(), SCREEN);

        drawn
    }

    fn panels(drawn: &Rendered) -> Vec<Panel> {
        let Ok(panels) = console_core_shapes::panels(&drawn.shapes);

        panels
    }

    fn words(drawn: &Rendered) -> Vec<Text> {
        let Ok(words) = console_core_shapes::texts(&drawn.shapes);

        words
    }

    fn far(panel: &Panel) -> i32 {
        let Ok(wide) = fitted::<u32, i32>(panel.size.width);

        panel.at.x.saturating_add(wide)
    }

    fn workspace(named: &str, id: i64, lit: Lit) -> Measured {
        let one = slot(
            vec![Span { text: named.to_string(), face: Face::Reading }],
            Some(BarAction::Workspace(id)),
        );

        Measured { slot: Slot { lit, ..one.slot }, ..one }
    }

    fn reading(icon: &str, beside: &str, item: StatusItem) -> Measured {
        slot(
            vec![
                Span { text: icon.to_string(), face: Face::Icon },
                Span { text: beside.to_string(), face: Face::Small },
            ],
            Some(BarAction::Settings(item)),
        )
    }

    fn a_bar_of_everything() -> Bar {
        bar(
            vec![
                icon("\u{f003b}", Some(BarAction::Launcher)),
                icon("\u{f030c}", Some(BarAction::Keyboard)),
                workspace("1", 1, Lit::No),
                workspace("2", 2, Lit::No),
                workspace("3", 3, Lit::Yes),
                workspace("+", 4, Lit::No),
            ],
            vec![slot(
                vec![Span { text: "Sun 20 Sep  02:15".to_string(), face: Face::Clock }],
                Some(BarAction::Calendar),
            )],
            vec![
                icon("\u{f075a}", Some(BarAction::Music)),
                reading("\u{f057e}", "60%", StatusItem::Sound),
                icon("\u{f00af}", Some(BarAction::Settings(StatusItem::Bluetooth))),
                reading("\u{f0079}", "95%", StatusItem::Battery),
                icon("\u{f009c}", Some(BarAction::Notifications)),
            ],
        )
    }

    fn does_of(slots: &[Measured]) -> Vec<Option<BarAction>> {
        slots.iter().map(|one| one.slot.action).collect()
    }

    #[test]
    fn a_bar_with_the_room_for_everything_sheds_nothing() {
        let whole = a_bar_of_everything();
        let Ok(shed) = shed(&whole, fitting(), ACROSS);

        assert_eq!(shed, whole);
    }

    fn runs_of(slots: &[Measured]) -> Vec<u32> {
        slots.iter().map(|one| u32::try_from(one.runs.len()).unwrap()).collect()
    }

    #[test]
    fn a_bar_with_no_room_keeps_the_workspace_you_are_on_and_drops_the_rest() {
        let whole = a_bar_of_everything();
        let Ok(needs) = needs(&whole, fitting());
        let Ok(shed) = shed(&whole, fitting(), needs.saturating_sub(100));

        assert_eq!(
            does_of(&shed.left),
            [Some(BarAction::Launcher), Some(BarAction::Keyboard), Some(BarAction::Workspace(3))],
            "the workspaces no one is on should have gone first"
        );
        assert_eq!(runs_of(&shed.right), runs_of(&whole.right), "a reading lost its words too soon");
        assert_eq!(does_of(&shed.middle), does_of(&whole.middle), "the clock was taken away");
    }

    #[test]
    fn a_bar_with_still_less_room_keeps_the_readings_and_drops_what_they_say() {
        let whole = a_bar_of_everything();
        let Ok(folded) = shed(&whole, fitting(), 0);

        assert_eq!(does_of(&folded.right), does_of(&whole.right), "a reading was taken away");
        assert_eq!(runs_of(&folded.right), [1, 1, 1, 1, 1], "a reading kept its words");
        assert_eq!(does_of(&folded.middle), does_of(&whole.middle), "the clock was taken away");
    }

    #[test]
    fn nothing_on_a_turned_screens_bar_is_drawn_over_anything_else() {
        let whole = a_bar_of_everything();

        for wide in [800_u32, 1024, 1280, 2048, 2560] {
            let room = Size { width: wide, height: wide.saturating_mul(2) };
            let Ok(drawn) = along(&whole, &wearing(), room);

            for pair in drawn.touching.windows(2) {
                match (pair.first(), pair.get(1)) {
                    (Some(one), Some(next)) => {
                        let Ok(far) = fitted::<u32, i32>(one.panel.size.width);

                        assert!(
                            one.panel.at.x.saturating_add(far) <= next.panel.at.x,
                            "at {wide} across, {:?} is drawn over {:?}",
                            one.action,
                            next.action
                        );
                    }
                    (_one, _next) => {}
                }
            }
        }
    }

    #[test]
    fn the_shares_land_on_the_numbers_the_stylesheet_held() {
        let one = fitting();
        let Ok(tall) = one.height();
        let size = |face| {
            let Ok(font) = one.font(face);

            font.height
        };

        assert_eq!(one.deep, 38, "the bar was 38 points deep on this screen");
        assert_eq!(one.thin, 2, "the strip was a row");
        assert_eq!(one.pad, 15, "the padding a thumb lands on was 15 either side");
        assert_eq!(one.margin, 2);
        assert_eq!(one.round, 4);
        assert_eq!(one.between, 4);
        assert_eq!(tall, 40, "the bar and the strip reserved 40 points between them");
        assert_eq!(size(Face::Icon), 22);
        assert_eq!(size(Face::Clock), 16);
        assert_eq!(size(Face::Reading), 16);
        assert_eq!(size(Face::Small), 12);
    }

    #[test]
    fn a_bigger_em_is_a_bigger_bar_in_proportion_and_the_icons_still_fit_in_it() {
        let Ok(one) = Fitting::of(18);
        let Ok(twice) = Fitting::of(36);

        assert!(
            twice.deep.abs_diff(one.deep.saturating_mul(2)) <= 1,
            "twice the type should be twice the bar: {} against {}",
            twice.deep,
            one.deep
        );
        assert!(twice.pad.abs_diff(one.pad.saturating_mul(2)) <= 1, "{twice:?} against {one:?}");

        for em in [12_u32, 18, 24, 36] {
            let Ok(fitting) = Fitting::of(em);
            let Ok(icon) = fitting.font(Face::Icon);
            let room = fitting.deep.saturating_sub(fitting.margin.saturating_mul(2));

            assert!(icon.height <= room, "at an em of {em} an icon {} tall is in {room}", icon.height);
        }
    }

    #[test]
    fn a_share_of_nothing_is_nothing_and_a_share_of_everything_is_everything() {
        let share = |thousandths| {
            let Ok(share) = Share::of(thousandths);

            share
        };

        assert_eq!(share(0).spent(640), Ok(0));
        assert_eq!(share(1000).spent(640), Ok(640));
        assert_eq!(share(500).spent(640), Ok(320));
        assert_eq!(share(59).spent(0), Ok(0));
    }

    #[test]
    fn a_slab_is_what_it_says_and_the_padding_a_thumb_lands_on() {
        let one = fitting();
        let Ok(size) = slab(&icon("\u{f00af}", None), one);

        assert_eq!(size.width, 14 + one.pad * 2);
        assert_eq!(size.height, one.deep - one.margin * 2);
    }

    #[test]
    fn a_slot_saying_two_things_puts_a_gap_between_them_and_pads_the_pair() {
        let both = slot(
            vec![
                Span { text: "\u{f0079}".to_string(), face: Face::Icon },
                Span { text: "64%".to_string(), face: Face::Small },
            ],
            None,
        );
        let fitting = fitting();
        let Ok(one) = slab(&both, fitting);

        assert_eq!(one.width, 14 + fitting.between + 24 + fitting.pad * 2);
    }

    #[test]
    fn nothing_that_can_be_pressed_is_smaller_than_a_thumb() {
        let drawn = drawn(&bar(vec![icon("\u{f0004}", Some(BarAction::Launcher))], vec![], vec![]));

        for touching in &drawn.touching {
            assert!(
                touching.panel.size.width >= 30 && touching.panel.size.height >= 30,
                "{:?} is smaller than a thumb at arm's length",
                touching.panel.size
            );
        }
        assert_eq!(drawn.touching.len(), 1);
    }

    #[test]
    fn the_left_group_starts_at_the_left_edge_and_the_right_group_ends_at_the_right() {
        let drawn = drawn(&bar(
            vec![lit(icon("\u{f0004}", Some(BarAction::Launcher)))],
            vec![],
            vec![lit(icon("\u{f009c}", Some(BarAction::Notifications)))],
        ));
        let round = Round(fitting().round);
        let slabs: Vec<Panel> =
            panels(&drawn).into_iter().filter(|panel| panel.round == round).collect();
        let Ok(margin) = fitted::<u32, i32>(fitting().margin);
        let Ok(across) = fitted::<u32, i32>(ACROSS);

        match slabs.as_slice() {
            [left, right] => {
                assert_eq!(left.at.x, margin, "the first slab is not against the left edge");
                assert_eq!(
                    far(right),
                    across.saturating_sub(margin),
                    "the last slab is not against the right edge"
                );
            }
            _ => panic!("two lit slabs should be drawn: {slabs:?}"),
        }
    }

    #[test]
    fn the_middle_is_the_middle_however_wide_the_screen_is() {
        let clock = slot(vec![Span { text: "14:30".to_string(), face: Face::Clock }], None);
        let one = [clock.clone()];
        let bar = bar(vec![], vec![clock], vec![]);

        for wide in [640_u32, 1024, 1280] {
            let room = Size { width: wide, height: SCREEN.height };
            let Ok(fitting) = Fitting::of_em();
            let Ok(drawn) = along(&bar, &wearing(), room);
            let Ok(one) = group(&one, fitting);
            let run = match words(&drawn).first() {
                Some(run) => run.at.x,
                None => panic!("the clock should be drawn"),
            };
            let Ok(from) = fitted::<u32, i32>(wide.saturating_sub(one).div_ceil(2));
            let Ok(into) = fitted::<u32, i32>(fitting.margin.saturating_add(fitting.pad));

            assert_eq!(run, from.saturating_add(into), "at {wide} across");
        }
    }

    #[test]
    fn lighting_a_slot_moves_nothing_along_the_bar() {
        let dark = bar(
            vec![icon("\u{f0004}", Some(BarAction::Launcher)), icon("\u{f0311}", Some(BarAction::Keyboard))],
            vec![],
            vec![],
        );
        let alight = bar(
            vec![
                lit(icon("\u{f0004}", Some(BarAction::Launcher))),
                icon("\u{f0311}", Some(BarAction::Keyboard)),
            ],
            vec![],
            vec![],
        );

        let placed = |bar: &Bar| {
            drawn(bar).touching.iter().map(|touching| touching.panel.at).collect::<Vec<_>>()
        };

        assert_eq!(placed(&dark), placed(&alight));
        assert_eq!(
            words(&drawn(&dark)).iter().map(|run| run.at).collect::<Vec<_>>(),
            words(&drawn(&alight)).iter().map(|run| run.at).collect::<Vec<_>>()
        );
    }

    #[test]
    fn an_unlit_slot_draws_no_slab_and_the_bar_shows_through() {
        let dark = drawn(&bar(vec![icon("\u{f0004}", Some(BarAction::Launcher))], vec![], vec![]));
        let alight = drawn(&bar(vec![lit(icon("\u{f0004}", Some(BarAction::Launcher)))], vec![], vec![]));

        assert_eq!(panels(&dark).len(), 1, "an unlit slot drew something under itself");
        assert_eq!(panels(&alight).len(), 2, "a lit slot drew no slab");
    }

    #[test]
    fn a_lit_slot_wears_the_deepest_ink_because_it_is_standing_on_pink() {
        let alight = drawn(&bar(vec![lit(icon("\u{f0004}", Some(BarAction::Launcher)))], vec![], vec![]));
        let worn = wearing();

        assert_eq!(words(&alight).first().map(|run| run.ink), Some(worn.night));
    }

    #[test]
    fn a_reading_and_a_thing_that_is_pressed_are_not_the_same_color_at_rest() {
        let worn = wearing();
        let ink = |tone| {
            let Ok(ink) = worn.ink(tone, Lit::No);

            ink
        };

        assert_eq!(ink(Tone::Plain), worn.text);
        assert_eq!(ink(Tone::Pressed), worn.pink);
        assert_eq!(ink(Tone::Secondary), worn.soft);
        assert_ne!(ink(Tone::Plain), ink(Tone::Pressed));
        assert_ne!(ink(Tone::Plain), ink(Tone::Secondary));
    }

    #[test]
    fn every_run_a_slot_says_is_drawn_and_not_only_the_first() {
        let charge = slot(
            vec![
                Span { text: "\u{f0079}".to_string(), face: Face::Icon },
                Span { text: "64%".to_string(), face: Face::Small },
            ],
            Some(BarAction::Settings(StatusItem::Battery)),
        );
        let drawn = drawn(&bar(vec![], vec![], vec![charge]));
        let said: Vec<String> = words(&drawn).into_iter().map(|run| run.said).collect();

        assert_eq!(said, ["\u{f0079}".to_string(), "64%".to_string()]);
    }

    #[test]
    fn what_a_slot_says_is_centred_down_the_bar_whatever_height_it_came_out() {
        let charge = slot(
            vec![
                Span { text: "\u{f0079}".to_string(), face: Face::Icon },
                Span { text: "64%".to_string(), face: Face::Small },
            ],
            None,
        );
        let drawn = drawn(&bar(vec![charge], vec![], vec![]));
        let middles: Vec<u32> = words(&drawn)
            .iter()
            .zip([22_u32, 13])
            .map(|(run, tall)| {
                let down = u32::try_from(run.at.y).unwrap_or(0);

                down.saturating_mul(2).saturating_add(tall)
            })
            .collect();

        match middles.as_slice() {
            [icon, beside] => assert!(
                icon.abs_diff(*beside) <= 1,
                "the icon and the reading beside it sit on different lines: {icon} and {beside}"
            ),
            _ => panic!("two runs should be drawn: {middles:?}"),
        }
    }

    #[test]
    fn a_thumb_on_the_second_icon_is_on_the_second_icon() {
        let drawn = drawn(&bar(
            vec![icon("\u{f0004}", Some(BarAction::Launcher)), icon("\u{f0311}", Some(BarAction::Keyboard))],
            vec![],
            vec![],
        ));
        let second = match drawn.touching.get(1) {
            Some(touching) => touching.panel.at.x.saturating_add(2),
            None => panic!("two slots should answer a tap"),
        };
        let Ok(found) = drawn.on(Point { x: second, y: 20 });

        assert_eq!(found, Some(BarAction::Keyboard));
    }

    #[test]
    fn a_thumb_on_the_bar_itself_is_on_nothing() {
        let drawn = drawn(&bar(vec![icon("\u{f0004}", Some(BarAction::Launcher))], vec![], vec![]));
        let Ok(middle) = drawn.on(Point { x: 500, y: 20 });
        let Ok(under) = drawn.on(Point { x: 20, y: 38 });

        assert_eq!(middle, None, "the empty middle of the bar opened something");
        assert_eq!(under, None, "the strip under the bar opened something");
    }

    #[test]
    fn a_slot_that_only_reads_answers_no_tap_at_all() {
        let clock = slot(vec![Span { text: "14:30".to_string(), face: Face::Clock }], None);
        let drawn = drawn(&bar(vec![], vec![clock], vec![]));

        assert!(drawn.touching.is_empty(), "the clock answered a tap");
    }

    #[test]
    fn the_strip_is_the_last_rows_of_the_bar_and_is_drawn_nowhere_else() {
        let fitting = fitting();
        let Ok(tall) = fitting.height();
        let running = Bar { filling: Filling::At(500), ..bar(vec![], vec![], vec![]) };
        let drawn = drawn(&running);
        let Ok(down) = fitted::<u32, i32>(fitting.deep);

        assert_eq!(drawn.room, Size { width: ACROSS, height: tall });

        match panels(&drawn).as_slice() {
            [ground, strip] => {
                assert_eq!(ground.size.height, tall, "the bar's own ground is not the whole surface");
                assert_eq!(strip.at.y, down);
                assert_eq!(strip.size.height, fitting.thin);
            }
            other => panic!("a ground and a fill should be drawn: {other:?}"),
        }
    }

    #[test]
    fn nothing_fills_the_strip_while_nothing_is_running() {
        let drawn = drawn(&bar(vec![], vec![], vec![]));

        assert_eq!(panels(&drawn).len(), 1, "something filled the strip with no apply running");
    }

    #[test]
    fn the_strip_only_reaches_the_end_when_the_thing_does() {
        let whole = Share::WHOLE.get();
        let Ok(whole) = fitted::<u32, u16>(whole);

        assert_eq!(filled(ACROSS, 0), Ok(0));
        assert_eq!(filled(ACROSS, whole), Ok(ACROSS));
        assert_eq!(filled(ACROSS, u16::MAX), Ok(ACROSS));

        for thousandth in 1..whole {
            let Ok(filled) = filled(ACROSS, thousandth);

            assert!(
                filled > 0 && filled < ACROSS,
                "{thousandth} of a thousand filled {filled} of {ACROSS}"
            );
        }
    }

    #[test]
    fn a_palette_that_does_not_spend_what_the_bar_wears_names_it() {
        let mut spent = BTreeMap::new();

        spent.insert("ground".to_string(), "2b2029".to_string());

        assert_eq!(Wearing::out_of(&spent), Err(PaletteError::Absent("text")));
    }

    #[test]
    fn the_bar_wears_nothing_the_palette_is_not_asked_for() {
        let mut spent = BTreeMap::new();

        for named in SPENDS {
            spent.insert(named.to_string(), "2b2029".to_string());
        }

        match Wearing::out_of(&spent) {
            Ok(_dressed) => {}
            Err(why) => panic!("the names this bar spends should dress it: {why}"),
        }
    }
}
