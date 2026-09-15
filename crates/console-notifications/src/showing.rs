//! A card, in numbers: how wide, how far down, and how far along a reading is.
//!
//! The measurements are mako's own, because they were argued for once already
//! and nothing about the screen has changed: the compositor draws a 768 by 480
//! picture onto a 1920 by 1200 panel, so 320 wide is a little under half the
//! room and 44 down clears the bar with six to spare. What is different is
//! where they live. They were in a configuration file a daemon read at
//! startup, beside colours a second stylesheet wrote; they are here, in the
//! crate that draws the card, and the stylesheet is written out of them.
//!
//! Three at once and no more. A fourth card would be a column of cards down
//! the right of a screen somebody is playing a game on, and what the fourth
//! one says is already in the panel and in the bell.
//!
//! The fill is the one piece of arithmetic here that is not a constant. A
//! reading -- the volume, the brightness, what is left in the battery -- is
//! sent as a number between nought and a hundred in a hint, and what a card
//! does with it is fill a bar that far across. Which is a proportion of a
//! width this file already knows, so it is answered here rather than measured
//! by whatever is drawing: a bar filled to a width nobody worked out is the
//! strip under the bar all over again, painting to a mark the number never
//! meant.

use std::collections::BTreeMap;

use console_core_colour::Oklch;
use console_core_colour::spent::named;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted, toward_zero_i32};
use console_core_shapes::{Edge as Border, Font, Panel, Round, Shape, Weight, Words};

use crate::reading::Urgency;

pub use console_onscreen::NOTICE as WHO;

pub const WIDE: i32 = 320;

pub const DOWN: i32 = 44;

pub const IN: i32 = 8;

pub const PAD: i32 = 10;

pub const EDGE: i32 = 2;

pub const ROUND: i32 = 4;

pub const BETWEEN: i32 = 8;

pub const DEEP: i32 = 6;

pub const MOST: usize = 3;

const WHOLE: i64 = 100;

pub fn across() -> Result<i32, Never> {
    Ok(WIDE
        .saturating_sub(PAD.saturating_mul(2))
        .saturating_sub(EDGE.saturating_mul(2)))
}

pub fn filled(value: i64) -> Result<i32, Never> {
    let Ok(across) = across();

    let held = value.clamp(0, WHOLE);
    let Ok(held) = held.float();
    let Ok(across) = i64::from(across).float();

    toward_zero_i32(across * held / 100.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wearing {
    pub panel: Oklch,
    pub text: Oklch,
    pub edge: Oklch,
    pub soft: Oklch,
    pub coral: Oklch,
    pub ground: Oklch,
    pub fill: Oklch,
}

pub const SPENDS: [&str; 7] = ["panel", "text", "edge", "soft", "coral", "ground", "fill"];

pub use console_core_colour::spent::Undressed;

impl Wearing {
    pub fn out_of(spent: &BTreeMap<String, String>) -> Result<Wearing, Undressed> {
        let panel = named(spent, "panel")?;
        let text = named(spent, "text")?;
        let edge = named(spent, "edge")?;
        let soft = named(spent, "soft")?;
        let coral = named(spent, "coral")?;
        let ground = named(spent, "ground")?;
        let fill = named(spent, "fill")?;

        Ok(Wearing { panel, text, edge, soft, coral, ground, fill })
    }

    fn worn(&self, urgency: Urgency) -> Result<Worn, Never> {
        Ok(match urgency {
            Urgency::Normal => Worn { edge: self.edge, ink: self.text },
            Urgency::Low => Worn { edge: self.soft, ink: self.soft },
            Urgency::Critical => Worn { edge: self.coral, ink: self.text },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Worn {
    edge: Oklch,
    ink: Oklch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measured {
    pub summary: Size<u32>,
    pub body: Option<Size<u32>>,
}

pub const FONT: &str = "Noto Sans";

pub const TALL: u32 = 21;

pub fn font() -> Result<Font, Never> {
    Ok(Font { family: FONT.to_string(), tall: TALL })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Saying<'a> {
    pub id: u32,
    pub summary: &'a str,
    pub body: &'a str,
    pub urgency: Urgency,
    pub value: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub shapes: Vec<Shape>,
    pub tall: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Touching {
    pub id: u32,
    pub panel: Panel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stack {
    pub shapes: Vec<Shape>,
    pub room: Size<u32>,
    pub touching: Vec<Touching>,
}

pub fn tall(measured: &Measured, value: Option<i64>) -> Result<u32, Never> {
    let Ok(edges) = twice(EDGE);
    let Ok(padding) = twice(PAD);
    let mut down = measured.summary.tall;

    match measured.body {
        Some(body) => {
            let Ok(gap) = gap();

            down = down.saturating_add(gap).saturating_add(body.tall);
        }
        None => {},
    }

    match value {
        Some(_) => {
            let Ok(gap) = gap();
            let Ok(deep) = deep();

            down = down.saturating_add(gap).saturating_add(deep);
        }
        None => {},
    }

    Ok(down.saturating_add(edges).saturating_add(padding))
}

pub fn card(
    saying: &Saying<'_>,
    measured: &Measured,
    wearing: &Wearing,
    at: Point<i32>,
) -> Result<Card, Never> {
    let Ok(tall) = tall(measured, saying.value);
    let Ok(worn) = wearing.worn(saying.urgency);
    let Ok(wide) = fitted::<i32, u32>(WIDE);
    let Ok(inner) = across();
    let Ok(inner) = fitted::<i32, u32>(inner);
    let Ok(inset) = fitted::<i32, i32>(EDGE.saturating_add(PAD));
    let Ok(border) = fitted::<i32, u32>(EDGE);
    let Ok(round) = fitted::<i32, u32>(ROUND);
    let Ok(font) = font();

    let mut shapes = vec![Shape::Panel(Panel {
        at,
        size: Size { wide, tall },
        round: Round(round),
        fill: wearing.panel,
        edge: Border::Of { wide: border, colour: worn.edge },
    })];
    let across = at.across.saturating_add(inset);
    let mut down = at.down.saturating_add(inset);

    shapes.push(Shape::Words(Words {
        at: Point { across, down },
        wide: inner,
        said: saying.summary.to_string(),
        weight: Weight::Bold,
        font: font.clone(),
        ink: worn.ink,
    }));

    let Ok(over) = step(measured.summary.tall);

    down = down.saturating_add(over);

    match measured.body {
        Some(body) => {
            shapes.push(Shape::Words(Words {
                at: Point { across, down },
                wide: inner,
                said: saying.body.to_string(),
                weight: Weight::Plain,
                font: font.clone(),
                ink: worn.ink,
            }));

            let Ok(over) = step(body.tall);

            down = down.saturating_add(over);
        }
        None => {},
    }

    match saying.value {
        Some(value) => {
            let Ok(filled) = filled(value);
            let Ok(filled) = fitted::<i32, u32>(filled);

            let Ok(deep) = deep();

            shapes.push(Shape::Panel(Panel {
                at: Point { across, down },
                size: Size { wide: inner, tall: deep },
                round: Round(round),
                fill: wearing.ground,
                edge: Border::None,
            }));
            shapes.push(Shape::Panel(Panel {
                at: Point { across, down },
                size: Size { wide: filled, tall: deep },
                round: Round(round),
                fill: wearing.fill,
                edge: Border::None,
            }));
        }
        None => {},
    }

    Ok(Card { shapes, tall })
}

impl Stack {
    pub fn on(&self, at: Point<i32>) -> Result<Option<u32>, Never> {
        Ok(self.touching.iter().find_map(|touching| match touching.panel.covers(at) {
            Ok(console_core_shapes::Covers::Yes) => Some(touching.id),
            Ok(console_core_shapes::Covers::No) | Err(_) => None,
        }))
    }
}

pub fn cards(said: &[(Saying<'_>, Measured)], wearing: &Wearing) -> Result<Stack, Never> {
    let Ok(wide) = fitted::<i32, u32>(WIDE);
    let mut shapes = Vec::new();
    let mut touching = Vec::new();
    let mut down: u32 = 0;

    for (saying, measured) in said.iter().take(MOST) {
        let Ok(at) = fitted::<u32, i32>(down);
        let Ok(card) = card(saying, measured, wearing, Point { across: 0, down: at });

        touching.push(Touching {
            id: saying.id,
            panel: Panel {
                at: Point { across: 0, down: at },
                size: Size { wide, tall: card.tall },
                round: Round(0),
                fill: wearing.panel,
                edge: Border::None,
            },
        });
        shapes.extend(card.shapes);

        let Ok(gap) = gap();

        down = down.saturating_add(card.tall).saturating_add(gap);
    }

    let Ok(gap) = gap();
    let tall = down.saturating_sub(gap);

    Ok(Stack { shapes, room: Size { wide, tall }, touching })
}

fn step(over: u32) -> Result<i32, Never> {
    let Ok(gap) = gap();

    fitted(over.saturating_add(gap))
}

fn gap() -> Result<u32, Never> {
    fitted(BETWEEN)
}

fn deep() -> Result<u32, Never> {
    fitted(DEEP)
}

fn twice(many: i32) -> Result<u32, Never> {
    fitted(many.saturating_mul(2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reading_of_nothing_fills_nothing_and_a_full_one_fills_the_width() {
        assert_eq!(filled(0), Ok(0));
        assert_eq!(filled(100), across());
    }

    #[test]
    fn a_reading_halfway_is_halfway_across() {
        let Ok(across) = across();
        let Ok(half) = filled(50);

        assert!((half.saturating_mul(2).saturating_sub(across)).abs() <= 1, "{half} of {across}");
    }

    #[test]
    fn a_reading_past_either_end_is_the_end_rather_than_a_bar_off_the_card() {
        assert_eq!(filled(-20), Ok(0));
        assert_eq!(filled(4000), across());
    }

    pub(super) fn wearing() -> Wearing {
        let mut spent = BTreeMap::new();

        for (named, six) in [
            ("panel", "372c3a"),
            ("text", "f7e7f3"),
            ("edge", "8a7d8e"),
            ("soft", "e5cde0"),
            ("coral", "ff8f8f"),
            ("ground", "231b26"),
            ("fill", "723b5f"),
        ] {
            spent.insert(named.to_string(), six.to_string());
        }

        match Wearing::out_of(&spent) {
            Ok(wearing) => wearing,
            Err(why) => panic!("a whole palette should dress a card: {why}"),
        }
    }

    pub(super) fn measured() -> Measured {
        Measured { summary: Size { wide: 280, tall: 20 }, body: None }
    }

    pub(super) fn saying() -> Saying<'static> {
        Saying {
            id: 1,
            summary: "something happened",
            body: "",
            urgency: Urgency::Normal,
            value: None,
        }
    }

    #[test]
    fn a_palette_missing_a_colour_names_it_rather_than_drawing_in_black() {
        let mut spent = BTreeMap::new();

        spent.insert("panel".to_string(), "372c3a".to_string());

        assert_eq!(Wearing::out_of(&spent), Err(Undressed::Absent("text")));
    }

    #[test]
    fn a_palette_spending_something_that_is_not_a_colour_says_so() {
        let mut spent = BTreeMap::new();

        for named in SPENDS {
            spent.insert(named.to_string(), "372c3a".to_string());
        }

        spent.insert("edge".to_string(), "mauve".to_string());

        assert_eq!(
            Wearing::out_of(&spent),
            Err(Undressed::Unreadable { named: "edge", said: "mauve".to_string() })
        );
    }

    #[test]
    fn a_card_is_its_edges_its_padding_and_what_it_says() {
        let Ok(down) = tall(&measured(), None);

        let Ok(padding) = twice(PAD);
        let Ok(edges) = twice(EDGE);

        assert_eq!(down, 20 + padding + edges);
    }

    #[test]
    fn a_reading_makes_a_card_taller_by_the_bar_and_the_gap_over_it() {
        let Ok(without) = tall(&measured(), None);
        let Ok(with) = tall(&measured(), Some(50));

        let Ok(gap) = gap();
        let Ok(deep) = deep();

        assert_eq!(with - without, gap + deep);
    }

    #[test]
    fn a_card_with_nothing_but_a_summary_is_a_panel_and_a_run_of_words() {
        let Ok(card) = card(&saying(), &measured(), &wearing(), Point { across: 0, down: 0 });

        assert_eq!(card.shapes.len(), 2, "{:?}", card.shapes);
    }

    #[test]
    fn a_reading_adds_the_bar_and_the_fill_over_it() {
        let saying = Saying { value: Some(50), ..saying() };
        let Ok(card) = card(&saying, &measured(), &wearing(), Point { across: 0, down: 0 });

        assert_eq!(card.shapes.len(), 4, "{:?}", card.shapes);
    }

    #[test]
    fn what_a_card_says_starts_inside_its_edge_and_its_padding() {
        let Ok(card) = card(&saying(), &measured(), &wearing(), Point { across: 0, down: 0 });
        let inset = EDGE.saturating_add(PAD);
        let words = card.shapes.iter().find_map(|shape| match shape {
            Shape::Words(words) => Some(words.at),
            Shape::Panel(_) => None,
        });

        assert_eq!(words, Some(Point { across: inset, down: inset }));
    }

    #[test]
    fn a_low_notice_wears_the_soft_colour_on_both_its_edge_and_its_words() {
        let saying = Saying { urgency: Urgency::Low, ..saying() };
        let Ok(card) = card(&saying, &measured(), &wearing(), Point { across: 0, down: 0 });
        let worn = wearing();
        let ink = card.shapes.iter().find_map(|shape| match shape {
            Shape::Words(words) => Some(words.ink),
            Shape::Panel(_) => None,
        });

        assert_eq!(ink, Some(worn.soft));
    }

    #[test]
    fn a_critical_notice_wears_coral_on_its_edge_and_keeps_its_words_readable() {
        let saying = Saying { urgency: Urgency::Critical, ..saying() };
        let Ok(card) = card(&saying, &measured(), &wearing(), Point { across: 0, down: 0 });
        let worn = wearing();
        let edge = card.shapes.iter().find_map(|shape| match shape {
            Shape::Panel(panel) => Some(panel.edge),
            Shape::Words(_) => None,
        });
        let ink = card.shapes.iter().find_map(|shape| match shape {
            Shape::Words(words) => Some(words.ink),
            Shape::Panel(_) => None,
        });

        assert!(matches!(edge, Some(console_core_shapes::Edge::Of { colour, .. }) if colour == worn.coral));
        assert_eq!(ink, Some(worn.text));
    }

    #[test]
    fn no_more_than_three_cards_are_ever_drawn() {
        let said: Vec<(Saying<'_>, Measured)> =
            (0..6).map(|_| (saying(), measured())).collect();
        let Ok(stack) = cards(&said, &wearing());

        assert_eq!(stack.shapes.len(), MOST * 2);
        assert_eq!(stack.touching.len(), MOST);
    }

    #[test]
    fn a_thumb_on_the_second_card_is_on_the_second_card_and_not_the_first() {
        let first = Saying { id: 7, ..saying() };
        let second = Saying { id: 9, ..saying() };
        let said = vec![(first, measured()), (second, measured())];
        let Ok(stack) = cards(&said, &wearing());
        let Ok(one) = tall(&measured(), None);
        let Ok(gap) = gap();
        let Ok(into) = fitted::<u32, i32>(one.saturating_add(gap).saturating_add(2));
        let Ok(found) = stack.on(Point { across: 10, down: into });

        assert_eq!(found, Some(9));
    }

    #[test]
    fn a_thumb_in_the_gap_between_two_cards_is_on_neither() {
        let said = vec![(saying(), measured()), (Saying { id: 9, ..saying() }, measured())];
        let Ok(stack) = cards(&said, &wearing());
        let Ok(one) = tall(&measured(), None);
        let Ok(into) = fitted::<u32, i32>(one.saturating_add(1));
        let Ok(found) = stack.on(Point { across: 10, down: into });

        assert_eq!(found, None);
    }

    #[test]
    fn a_stack_of_cards_is_as_tall_as_the_cards_and_the_gaps_between_them() {
        let said = vec![(saying(), measured()), (saying(), measured())];
        let Ok(stack) = cards(&said, &wearing());
        let Ok(one) = tall(&measured(), None);
        let Ok(wide) = fitted::<i32, u32>(WIDE);

        let Ok(gap) = gap();

        assert_eq!(stack.room.tall, one * 2 + gap);
        assert_eq!(stack.room.wide, wide);
    }
}

#[cfg(test)]
mod the_stack {
    use super::*;
    use super::tests::{measured, saying, wearing};

    #[test]
    fn the_last_card_ends_exactly_where_the_stack_does() {
        let said = vec![
            (saying(), measured()),
            (saying(), Measured { summary: Size { wide: 280, tall: 44 }, body: None }),
            (saying(), measured()),
        ];
        let Ok(stack) = cards(&said, &wearing());
        let ended = stack.touching.iter().map(|touching| {
            touching.panel.at.down.saturating_add(
                console_core_number_conversion::fitted::<u32, i32>(touching.panel.size.tall)
                    .unwrap_or(0),
            )
        }).max();
        let Ok(room) = fitted::<u32, i32>(stack.room.tall);

        assert_eq!(ended, Some(room), "the stack is not as tall as what is in it");
    }
}
