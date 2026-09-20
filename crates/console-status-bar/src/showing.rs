//! The bar, in numbers: how deep it is, what stands along it, and what a thumb
//! on it lands on.
//!
//! **Every measurement is a share of something rather than a number of
//! points.** The stylesheet held them as points -- 38 deep, 15 either side of
//! what a slot says, 2 of margin -- and they were right for one screen at one
//! density and silently wrong at any other. The Screen tab changes how many
//! logical points this glass is laid out in, and a bar written in points gets
//! physically smaller every time somebody raises that, which makes the one
//! control on the desktop that has to be hittable the one that shrinks. So the
//! bar's depth is a share of the screen's shorter side and everything inside it
//! is a share of the bar's depth, which is what those numbers always were: 15 of
//! padding was never fifteen of anything, it was a bit under four tenths of the
//! depth, and saying so is what makes it hold on a screen nobody has yet.
//!
//! At this device's 1024 by 640 they come out at exactly the points the
//! stylesheet held, which is how the shares were chosen and is what
//! `the_shares_land_on_the_numbers_the_stylesheet_held` keeps true.
//!
//! **The shorter side, and not the height, because the screen turns.** The two
//! were the same number for as long as this panel only ever stood the way it is
//! screwed in, where the height is the short edge. Turned a quarter the height
//! is the long edge, and a share of it is a bar two and a half times as deep
//! with every glyph in it grown to match, which is what a person sees first and
//! is what they say is wrong. The shorter side is the panel's own short edge
//! whichever way up it stands, so the bar comes out the same number of real
//! pixels deep either way: the depth of a bar is a question about a thumb, and
//! a thumb does not turn with the screen.
//!
//! What the shape buys is a second knob: the bar as a whole is `DEEP`, and
//! anything in it can be nudged against the bar without moving the rest.
//!
//! **A turned screen is a shorter bar, and something has to go.** The depth
//! holds because a thumb does not turn with the screen -- but neither does
//! anything else in here, and the edge the bar runs along is the panel's long
//! one one way up and its short one the other. So the same slots, at the same
//! size to a thumb, are asked to stand in about two thirds of the room, and
//! what they did instead was stand on top of each other: the clock painted
//! across the music, which is a bar that looks broken rather than a bar that is
//! full.
//!
//! [`shed`] is the order things go in, and it is an order rather than a
//! shrink, because everything here is already the size a thumb needs. The
//! workspaces nobody is on go first -- the shoulders are what move between
//! windows on this device and the bar is where you read which one you are on,
//! so the one in front is the whole of what that row has to say when there is
//! no room for the rest. Then the readings drop what is written beside them
//! and keep their icon, which is the other half of a slot that was never the
//! half being read at a glance. If it still does not fit the middle is put
//! against the group on its right rather than in the centre of the screen, so
//! what a short bar looks like is a clock off-centre and not a clock over an
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

use console_core_colour::Oklch;
use console_core_colour::spent::{Undressed, named};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_shapes::{Covers, Edge, Font, Panel, Round, Shape, Weight, Words};

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use crate::reading::{Tone, What};

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

pub const DEEP: Share = Share(59);

pub const THIN: Share = Share(3);

pub const PAD: Share = Share(395);

pub const MARGIN: Share = Share(53);

pub const ROUND: Share = Share(105);

pub const BETWEEN: Share = Share(105);

pub const ICON: Share = Share(579);

pub const WORD: Share = Share(395);

pub const SMALL: Share = Share(289);

pub const LETTERS: &str = "Noto Sans";

pub const ICONS: &str = "FantasqueSansM Nerd Font Mono";

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
    pub fn of(room: Size<u32>) -> Result<Fitting, Never> {
        let across = room.wide.min(room.tall);

        let Ok(deep) = DEEP.spent(across);
        let Ok(thin) = THIN.spent(across);
        let Ok(pad) = PAD.spent(deep);
        let Ok(margin) = MARGIN.spent(deep);
        let Ok(round) = ROUND.spent(deep);
        let Ok(between) = BETWEEN.spent(deep);

        Ok(Fitting { deep, thin, pad, margin, round, between })
    }

    pub fn tall(&self) -> Result<u32, Never> {
        Ok(self.deep.saturating_add(self.thin))
    }

    pub fn font(&self, face: Face) -> Result<Font, Never> {
        let share = match face {
            Face::Icon => ICON,
            Face::Clock | Face::Reading => WORD,
            Face::Small => SMALL,
        };
        let family = match face {
            Face::Icon => ICONS,
            Face::Clock | Face::Reading | Face::Small => LETTERS,
        };
        let Ok(tall) = share.spent(self.deep);

        Ok(Font { family: family.to_string(), tall })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    pub said: String,
    pub face: Face,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lit {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Does {
    Calendar,
    Launcher,
    Keyboard,
    Workspace(i64),
    Settings(What),
    Music,
    Notices,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub said: Vec<Said>,
    pub tone: Tone,
    pub lit: Lit,
    pub does: Option<Does>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measured {
    pub slot: Slot,
    pub runs: Vec<Size<u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filling {
    At(u16),
    Nothing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saying {
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
pub struct Touching {
    pub does: Does,
    pub panel: Panel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Drawn {
    pub shapes: Vec<Shape>,
    pub room: Size<u32>,
    pub touching: Vec<Touching>,
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
    pub fn out_of(spent: &BTreeMap<String, String>) -> Result<Wearing, Undressed> {
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
                Tone::Quiet => self.soft,
                Tone::Pressed => self.pink,
                Tone::Low => self.butter,
                Tone::Wrong => self.coral,
                Tone::Well => self.leaf,
            },
        })
    }
}

pub fn slab(measured: &Measured, fitting: Fitting) -> Result<Size<u32>, Never> {
    let said = measured.runs.iter().map(|run| run.wide).fold(0_u32, u32::saturating_add);
    let Ok(many) = fitted::<usize, u32>(measured.runs.len());
    let gaps = fitting.between.saturating_mul(many.saturating_sub(1));
    let wide = said.saturating_add(gaps).saturating_add(fitting.pad.saturating_mul(2));

    Ok(Size { wide, tall: fitting.deep.saturating_sub(fitting.margin.saturating_mul(2)) })
}

fn advance(measured: &Measured, fitting: Fitting) -> Result<u32, Never> {
    let Ok(slab) = slab(measured, fitting);

    Ok(slab.wide.saturating_add(fitting.margin.saturating_mul(2)))
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
enum Keep {
    Yes,
    No,
}

fn one_of_the_workspaces_nobody_is_on(one: &Measured) -> Result<Keep, Never> {
    let does = match one.slot.does {
        Some(does) => does,
        None => return Ok(Keep::Yes),
    };

    Ok(match does {
        Does::Workspace(_) => match one.slot.lit {
            Lit::Yes => Keep::Yes,
            Lit::No => Keep::No,
        },
        Does::Calendar
        | Does::Launcher
        | Does::Keyboard
        | Does::Settings(_)
        | Does::Music
        | Does::Notices => Keep::Yes,
    })
}

fn only_the_workspace_in_front(slots: &[Measured]) -> Result<Vec<Measured>, Never> {
    let mut kept = Vec::new();

    for one in slots {
        let Ok(keep) = one_of_the_workspaces_nobody_is_on(one);

        match keep {
            Keep::Yes => kept.push(one.clone()),
            Keep::No => {},
        }
    }

    Ok(kept)
}

fn the_icon_alone(one: &Measured) -> Result<Measured, Never> {
    let mut said = Vec::new();
    let mut runs = Vec::new();

    for (one, run) in one.slot.said.iter().zip(&one.runs) {
        match one.face {
            Face::Small => {},
            Face::Icon | Face::Clock | Face::Reading => {
                said.push(one.clone());
                runs.push(*run);
            }
        }
    }

    Ok(Measured { slot: Slot { said, ..one.slot.clone() }, runs })
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

pub fn along(bar: &Bar, wearing: &Wearing, room: Size<u32>) -> Result<Drawn, Never> {
    let Ok(fitting) = Fitting::of(room);
    let Ok(tall) = fitting.tall();
    let wide = room.wide;
    let Ok(bar) = shed(bar, fitting, wide);
    let mut shapes = vec![Shape::Panel(Panel {
        at: Point { across: 0, down: 0 },
        size: Size { wide, tall },
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
    let Ok(centre) = fitted::<u32, i32>(halfway.min(free).max(along_the_left.min(free)));
    let Ok(ends) = fitted::<u32, i32>(wide.saturating_sub(right));

    for (slots, from) in [(&bar.left, 0), (&bar.middle, centre), (&bar.right, ends)] {
        let Ok(()) = laid(slots, from, fitting, wearing, &mut shapes, &mut touching);
    }

    match bar.filling {
        Filling::Nothing => {}
        Filling::At(thousandths) => {
            let Ok(filled) = filled(wide, thousandths);
            let Ok(down) = fitted::<u32, i32>(fitting.deep);

            shapes.push(Shape::Panel(Panel {
                at: Point { across: 0, down },
                size: Size { wide: filled, tall: fitting.thin },
                round: Round(0),
                fill: wearing.fill,
                edge: Edge::None,
            }));
        }
    }

    Ok(Drawn { shapes, room: Size { wide, tall }, touching })
}

fn laid(
    slots: &[Measured],
    from: i32,
    fitting: Fitting,
    wearing: &Wearing,
    shapes: &mut Vec<Shape>,
    touching: &mut Vec<Touching>,
) -> Result<(), Never> {
    let Ok(margin) = fitted::<u32, i32>(fitting.margin);
    let Ok(pad) = fitted::<u32, i32>(fitting.pad);
    let mut across = from;

    for one in slots {
        let Ok(size) = slab(one, fitting);
        let panel = Panel {
            at: Point { across: across.saturating_add(margin), down: margin },
            size,
            round: Round(fitting.round),
            fill: wearing.pink,
            edge: Edge::None,
        };

        match one.slot.lit {
            Lit::Yes => shapes.push(Shape::Panel(panel)),
            Lit::No => {},
        }

        match one.slot.does {
            Some(does) => touching.push(Touching { does, panel }),
            None => {},
        }

        let Ok(ink) = wearing.ink(one.slot.tone, one.slot.lit);
        let mut at = panel.at.across.saturating_add(pad);

        for (said, run) in one.slot.said.iter().zip(&one.runs) {
            let Ok(font) = fitting.font(said.face);
            let Ok(weight) = said.face.weight();
            let Ok(down) = centred(panel, *run);

            shapes.push(Shape::Words(Words {
                at: Point { across: at, down },
                wide: run.wide,
                said: said.said.clone(),
                weight,
                font,
                ink,
            }));

            let Ok(step) = fitted::<u32, i32>(run.wide.saturating_add(fitting.between));

            at = at.saturating_add(step);
        }

        let Ok(step) = advance(one, fitting);
        let Ok(step) = fitted::<u32, i32>(step);

        across = across.saturating_add(step);
    }

    Ok(())
}

fn centred(panel: Panel, run: Size<u32>) -> Result<i32, Never> {
    let Ok(spare) = fitted::<u32, i32>(panel.size.tall.saturating_sub(run.tall).div_ceil(2));

    Ok(panel.at.down.saturating_add(spare))
}

impl Drawn {
    pub fn on(&self, at: Point<i32>) -> Result<Option<Does>, Never> {
        Ok(self.touching.iter().find_map(|touching| match touching.panel.covers(at) {
            Ok(Covers::Yes) => Some(touching.does),
            Ok(Covers::No) | Err(_) => None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIX: [(&str, &str); 9] = [
        ("ground", "2b2029"),
        ("text", "f7e7f3"),
        ("soft", "e5cde0"),
        ("pink", "ff9ecb"),
        ("night", "231b26"),
        ("butter", "f2d692"),
        ("coral", "ff8f8f"),
        ("leaf", "9fd88f"),
        ("fill", "723b5f"),
    ];

    const SCREEN: Size<u32> = Size { wide: 1024, tall: 640 };

    const ACROSS: u32 = SCREEN.wide;

    fn fitting() -> Fitting {
        let Ok(fitting) = Fitting::of(SCREEN);

        fitting
    }

    pub(super) fn wearing() -> Wearing {
        let mut spent = BTreeMap::new();

        for (named, six) in SIX {
            spent.insert(named.to_string(), six.to_string());
        }

        match Wearing::out_of(&spent) {
            Ok(wearing) => wearing,
            Err(why) => panic!("a whole palette should dress a bar: {why}"),
        }
    }

    fn slot(said: Vec<Said>, does: Option<Does>) -> Measured {
        let runs = said
            .iter()
            .map(|one| match one.face {
                Face::Icon => Size { wide: 14, tall: 22 },
                Face::Clock | Face::Reading => Size { wide: 40, tall: 18 },
                Face::Small => Size { wide: 24, tall: 13 },
            })
            .collect();

        Measured {
            slot: Slot { said, tone: Tone::Plain, lit: Lit::No, does },
            runs,
        }
    }

    fn icon(said: &str, does: Option<Does>) -> Measured {
        slot(vec![Said { said: said.to_string(), face: Face::Icon }], does)
    }

    fn lit(measured: Measured) -> Measured {
        Measured { slot: Slot { lit: Lit::Yes, ..measured.slot }, ..measured }
    }

    fn bar(left: Vec<Measured>, middle: Vec<Measured>, right: Vec<Measured>) -> Bar {
        Bar { left, middle, right, filling: Filling::Nothing }
    }

    fn drawn(bar: &Bar) -> Drawn {
        let Ok(drawn) = along(bar, &wearing(), SCREEN);

        drawn
    }

    fn panels(drawn: &Drawn) -> Vec<Panel> {
        drawn
            .shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Panel(panel) => Some(*panel),
                Shape::Words(_) => None,
            })
            .collect()
    }

    fn words(drawn: &Drawn) -> Vec<Words> {
        drawn
            .shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Words(words) => Some(words.clone()),
                Shape::Panel(_) => None,
            })
            .collect()
    }

    fn far(panel: &Panel) -> i32 {
        let Ok(wide) = fitted::<u32, i32>(panel.size.wide);

        panel.at.across.saturating_add(wide)
    }

    #[test]
    fn a_turned_screen_is_the_same_bar_and_not_a_bar_two_and_a_half_times_as_deep() {
        let Ok(mounted) = Fitting::of(Size { wide: 1024, tall: 640 });
        let Ok(turned) = Fitting::of(Size { wide: 1280, tall: 2048 });
        let Ok(mounted_pixels) = mounted.tall();
        let Ok(turned_pixels) = turned.tall();

        assert_eq!(
            mounted_pixels.saturating_mul(5) / 2,
            100,
            "the panel as it is mounted, at the rung it was drawn for"
        );
        assert_eq!(
            turned_pixels.saturating_mul(5) / 4,
            100,
            "a quarter turn is the same real pixels deep, at the scale that turn is worn at"
        );
    }

    fn workspace(named: &str, id: i64, lit: Lit) -> Measured {
        let one = slot(
            vec![Said { said: named.to_string(), face: Face::Reading }],
            Some(Does::Workspace(id)),
        );

        Measured { slot: Slot { lit, ..one.slot }, ..one }
    }

    fn reading(icon: &str, beside: &str, what: What) -> Measured {
        slot(
            vec![
                Said { said: icon.to_string(), face: Face::Icon },
                Said { said: beside.to_string(), face: Face::Small },
            ],
            Some(Does::Settings(what)),
        )
    }

    fn a_bar_of_everything() -> Bar {
        bar(
            vec![
                icon("\u{f003b}", Some(Does::Launcher)),
                icon("\u{f030c}", Some(Does::Keyboard)),
                workspace("1", 1, Lit::No),
                workspace("2", 2, Lit::No),
                workspace("3", 3, Lit::Yes),
                workspace("+", 4, Lit::No),
            ],
            vec![slot(
                vec![Said { said: "Sun 20 Sep  02:15".to_string(), face: Face::Clock }],
                Some(Does::Calendar),
            )],
            vec![
                icon("\u{f075a}", Some(Does::Music)),
                reading("\u{f057e}", "60%", What::Sound),
                icon("\u{f00af}", Some(Does::Settings(What::Bluetooth))),
                reading("\u{f0079}", "95%", What::Battery),
                icon("\u{f009c}", Some(Does::Notices)),
            ],
        )
    }

    fn does_of(slots: &[Measured]) -> Vec<Option<Does>> {
        slots.iter().map(|one| one.slot.does).collect()
    }

    #[test]
    fn a_bar_with_the_room_for_everything_sheds_nothing() {
        let whole = a_bar_of_everything();
        let Ok(shed) = shed(&whole, fitting(), ACROSS);

        assert_eq!(shed, whole);
    }

    fn runs_of(slots: &[Measured]) -> Vec<usize> {
        slots.iter().map(|one| one.runs.len()).collect()
    }

    #[test]
    fn a_bar_with_no_room_keeps_the_workspace_you_are_on_and_drops_the_rest() {
        let whole = a_bar_of_everything();
        let Ok(needs) = needs(&whole, fitting());
        let Ok(shed) = shed(&whole, fitting(), needs.saturating_sub(100));

        assert_eq!(
            does_of(&shed.left),
            [Some(Does::Launcher), Some(Does::Keyboard), Some(Does::Workspace(3))],
            "the workspaces nobody is on should have gone first"
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
            let room = Size { wide, tall: wide.saturating_mul(2) };
            let Ok(drawn) = along(&whole, &wearing(), room);

            for pair in drawn.touching.windows(2) {
                match (pair.first(), pair.get(1)) {
                    (Some(one), Some(next)) => {
                        let Ok(far) = fitted::<u32, i32>(one.panel.size.wide);

                        assert!(
                            one.panel.at.across.saturating_add(far) <= next.panel.at.across,
                            "at {wide} across, {:?} is drawn over {:?}",
                            one.does,
                            next.does
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
        let Ok(tall) = one.tall();
        let size = |face| {
            let Ok(font) = one.font(face);

            font.tall
        };

        assert_eq!(one.deep, 38, "the bar was 38 points deep on this screen");
        assert_eq!(one.thin, 2, "the strip was a row");
        assert_eq!(one.pad, 15, "the padding a thumb lands on was 15 either side");
        assert_eq!(one.margin, 2);
        assert_eq!(one.round, 4);
        assert_eq!(one.between, 4);
        assert_eq!(tall, 40, "the bar and the strip reserved 40 points between them");
        assert_eq!(size(Face::Icon), 22);
        assert_eq!(size(Face::Clock), 15);
        assert_eq!(size(Face::Reading), 15);
        assert_eq!(size(Face::Small), 11);
    }

    #[test]
    fn a_denser_screen_is_a_bar_that_is_the_same_size_to_a_thumb() {
        let Ok(loose) = Fitting::of(Size { wide: 1024, tall: 640 });
        let Ok(dense) = Fitting::of(Size { wide: 1280, tall: 800 });
        let quarter = loose.deep.saturating_mul(5).div_ceil(4);

        assert!(dense.deep > loose.deep, "{dense:?} against {loose:?}");
        assert!(
            dense.deep.abs_diff(quarter) <= 1,
            "a screen a quarter denser should give a bar a quarter deeper: {} against {quarter}",
            dense.deep
        );
        assert!(dense.pad > loose.pad, "the target shrank when the screen got denser");
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

        assert_eq!(size.wide, 14 + one.pad * 2);
        assert_eq!(size.tall, one.deep - one.margin * 2);
    }

    #[test]
    fn a_slot_saying_two_things_puts_a_gap_between_them_and_pads_the_pair() {
        let both = slot(
            vec![
                Said { said: "\u{f0079}".to_string(), face: Face::Icon },
                Said { said: "64%".to_string(), face: Face::Small },
            ],
            None,
        );
        let fitting = fitting();
        let Ok(one) = slab(&both, fitting);

        assert_eq!(one.wide, 14 + fitting.between + 24 + fitting.pad * 2);
    }

    #[test]
    fn nothing_that_can_be_pressed_is_smaller_than_a_thumb() {
        let drawn = drawn(&bar(vec![icon("\u{f0004}", Some(Does::Launcher))], vec![], vec![]));

        for touching in &drawn.touching {
            assert!(
                touching.panel.size.wide >= 30 && touching.panel.size.tall >= 30,
                "{:?} is smaller than a thumb at arm's length",
                touching.panel.size
            );
        }
        assert_eq!(drawn.touching.len(), 1);
    }

    #[test]
    fn the_left_group_starts_at_the_left_edge_and_the_right_group_ends_at_the_right() {
        let drawn = drawn(&bar(
            vec![lit(icon("\u{f0004}", Some(Does::Launcher)))],
            vec![],
            vec![lit(icon("\u{f009c}", Some(Does::Notices)))],
        ));
        let round = Round(fitting().round);
        let slabs: Vec<Panel> =
            panels(&drawn).into_iter().filter(|panel| panel.round == round).collect();
        let Ok(margin) = fitted::<u32, i32>(fitting().margin);
        let Ok(across) = fitted::<u32, i32>(ACROSS);

        match slabs.as_slice() {
            [left, right] => {
                assert_eq!(left.at.across, margin, "the first slab is not against the left edge");
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
        let clock = slot(vec![Said { said: "14:30".to_string(), face: Face::Clock }], None);
        let one = [clock.clone()];
        let bar = bar(vec![], vec![clock], vec![]);

        for wide in [640_u32, 1024, 1280] {
            let room = Size { wide, tall: SCREEN.tall };
            let Ok(fitting) = Fitting::of(room);
            let Ok(drawn) = along(&bar, &wearing(), room);
            let Ok(one) = group(&one, fitting);
            let run = match words(&drawn).first() {
                Some(run) => run.at.across,
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
            vec![icon("\u{f0004}", Some(Does::Launcher)), icon("\u{f0311}", Some(Does::Keyboard))],
            vec![],
            vec![],
        );
        let alight = bar(
            vec![
                lit(icon("\u{f0004}", Some(Does::Launcher))),
                icon("\u{f0311}", Some(Does::Keyboard)),
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
        let dark = drawn(&bar(vec![icon("\u{f0004}", Some(Does::Launcher))], vec![], vec![]));
        let alight = drawn(&bar(vec![lit(icon("\u{f0004}", Some(Does::Launcher)))], vec![], vec![]));

        assert_eq!(panels(&dark).len(), 1, "an unlit slot drew something under itself");
        assert_eq!(panels(&alight).len(), 2, "a lit slot drew no slab");
    }

    #[test]
    fn a_lit_slot_wears_the_deepest_ink_because_it_is_standing_on_pink() {
        let alight = drawn(&bar(vec![lit(icon("\u{f0004}", Some(Does::Launcher)))], vec![], vec![]));
        let worn = wearing();

        assert_eq!(words(&alight).first().map(|run| run.ink), Some(worn.night));
    }

    #[test]
    fn a_reading_and_a_thing_that_is_pressed_are_not_the_same_colour_at_rest() {
        let worn = wearing();
        let ink = |tone| {
            let Ok(ink) = worn.ink(tone, Lit::No);

            ink
        };

        assert_eq!(ink(Tone::Plain), worn.text);
        assert_eq!(ink(Tone::Pressed), worn.pink);
        assert_eq!(ink(Tone::Quiet), worn.soft);
        assert_ne!(ink(Tone::Plain), ink(Tone::Pressed));
        assert_ne!(ink(Tone::Plain), ink(Tone::Quiet));
    }

    #[test]
    fn every_run_a_slot_says_is_drawn_and_not_only_the_first() {
        let charge = slot(
            vec![
                Said { said: "\u{f0079}".to_string(), face: Face::Icon },
                Said { said: "64%".to_string(), face: Face::Small },
            ],
            Some(Does::Settings(What::Battery)),
        );
        let drawn = drawn(&bar(vec![], vec![], vec![charge]));
        let said: Vec<String> = words(&drawn).into_iter().map(|run| run.said).collect();

        assert_eq!(said, ["\u{f0079}".to_string(), "64%".to_string()]);
    }

    #[test]
    fn what_a_slot_says_is_centred_down_the_bar_whatever_height_it_came_out() {
        let charge = slot(
            vec![
                Said { said: "\u{f0079}".to_string(), face: Face::Icon },
                Said { said: "64%".to_string(), face: Face::Small },
            ],
            None,
        );
        let drawn = drawn(&bar(vec![charge], vec![], vec![]));
        let middles: Vec<u32> = words(&drawn)
            .iter()
            .zip([22_u32, 13])
            .map(|(run, tall)| {
                let down = u32::try_from(run.at.down).unwrap_or(0);

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
            vec![icon("\u{f0004}", Some(Does::Launcher)), icon("\u{f0311}", Some(Does::Keyboard))],
            vec![],
            vec![],
        ));
        let second = match drawn.touching.get(1) {
            Some(touching) => touching.panel.at.across.saturating_add(2),
            None => panic!("two slots should answer a tap"),
        };
        let Ok(found) = drawn.on(Point { across: second, down: 20 });

        assert_eq!(found, Some(Does::Keyboard));
    }

    #[test]
    fn a_thumb_on_the_bar_itself_is_on_nothing() {
        let drawn = drawn(&bar(vec![icon("\u{f0004}", Some(Does::Launcher))], vec![], vec![]));
        let Ok(middle) = drawn.on(Point { across: 500, down: 20 });
        let Ok(under) = drawn.on(Point { across: 20, down: 38 });

        assert_eq!(middle, None, "the empty middle of the bar opened something");
        assert_eq!(under, None, "the strip under the bar opened something");
    }

    #[test]
    fn a_slot_that_only_reads_answers_no_tap_at_all() {
        let clock = slot(vec![Said { said: "14:30".to_string(), face: Face::Clock }], None);
        let drawn = drawn(&bar(vec![], vec![clock], vec![]));

        assert!(drawn.touching.is_empty(), "the clock answered a tap");
    }

    #[test]
    fn the_strip_is_the_last_rows_of_the_bar_and_is_drawn_nowhere_else() {
        let fitting = fitting();
        let Ok(tall) = fitting.tall();
        let running = Bar { filling: Filling::At(500), ..bar(vec![], vec![], vec![]) };
        let drawn = drawn(&running);
        let Ok(down) = fitted::<u32, i32>(fitting.deep);

        assert_eq!(drawn.room, Size { wide: ACROSS, tall });

        match panels(&drawn).as_slice() {
            [ground, strip] => {
                assert_eq!(ground.size.tall, tall, "the bar's own ground is not the whole surface");
                assert_eq!(strip.at.down, down);
                assert_eq!(strip.size.tall, fitting.thin);
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

        assert_eq!(Wearing::out_of(&spent), Err(Undressed::Absent("text")));
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
