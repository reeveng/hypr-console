//! A card, in numbers: how wide, how far down, and how far along a reading is.
//!
//! The measurements are mako's own, because they were argued for once already
//! and nothing about the screen has changed: the compositor draws a 768 by 480
//! picture onto a 1920 by 1200 panel, so 320 wide is a little under half the
//! room and 44 down clears the bar with six to spare. What is different is
//! where they live. They were in a configuration file a daemon read at
//! startup, beside colors a second stylesheet wrote; they are here, in the
//! crate that draws the card, and the stylesheet is written out of them.
//!
//! Three at once and no more. A fourth card would be a column of cards down
//! the right of a screen someone is playing a game on, and what the fourth
//! one says is already in the panel and in the bell.
//!
//! The fill is the one piece of arithmetic here that is not a constant. A
//! reading -- the volume, the brightness, what is left in the battery -- is
//! sent as a number between zero and a hundred in a hint, and what a card
//! does with it is fill a bar that far across. Which is a proportion of a
//! width this file already knows, so it is answered here rather than measured
//! by whatever is drawing: a bar filled to a width no one worked out is the
//! strip under the bar all over again, painting to a mark the number never
//! meant.

use console_core_color::Oklch;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted, index, toward_zero_i32};
use console_core_fonts::{EM, TextStyle};
use console_core_shapes::{Edge as Border, Panel, Round, Shape, Text};

use crate::reading::Urgency;

pub use console_onscreen::NOTIFICATION as WHO;

pub use console_core_color::palette::{SPENDS, PaletteError, Wearing};

pub const WIDE: i32 = EM * 160 / 9;

pub const DOWN: i32 = 44;

pub const IN: i32 = EM * 4 / 9;

pub const PAD: i32 = EM * 5 / 9;

pub const EDGE: i32 = EM / 9;

pub const ROUND: i32 = EM * 2 / 9;

pub const BETWEEN: i32 = EM * 4 / 9;

pub const DEEP: i32 = EM / 3;

pub const MOST: u32 = 3;

const WHOLE: i64 = 100;

pub fn x() -> Result<i32, Never> {
    Ok(WIDE
        .saturating_sub(PAD.saturating_mul(2))
        .saturating_sub(EDGE.saturating_mul(2)))
}

pub fn filled(value: i64) -> Result<i32, Never> {
    let Ok(across) = x();

    let held = value.clamp(0, WHOLE);
    let Ok(held) = held.float();
    let Ok(across) = i64::from(across).float();

    toward_zero_i32(across * held / 100.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Worn {
    edge: Oklch,
    ink: Oklch,
}

fn worn(wearing: &Wearing, urgency: Urgency) -> Result<Worn, Never> {
    Ok(match urgency {
        Urgency::Normal => Worn { edge: wearing.edge, ink: wearing.text },
        Urgency::Low => Worn { edge: wearing.soft, ink: wearing.soft },
        Urgency::Critical => Worn { edge: wearing.coral, ink: wearing.text },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measured {
    pub summary: Size<u32>,
    pub body: Option<Size<u32>>,
}

pub const SUMMARY: TextStyle = TextStyle::Headline;

pub const BODY: TextStyle = TextStyle::Callout;

#[derive(Debug, Clone, PartialEq)]
pub struct CardContent<'a> {
    pub id: u32,
    pub summary: &'a str,
    pub body: &'a str,
    pub urgency: Urgency,
    pub value: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub shapes: Vec<Shape>,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitRegion {
    pub id: u32,
    pub panel: Panel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stack {
    pub shapes: Vec<Shape>,
    pub room: Size<u32>,
    pub touching: Vec<HitRegion>,
}

pub fn height(measured: &Measured, value: Option<i64>) -> Result<u32, Never> {
    let Ok(edges) = twice(EDGE);
    let Ok(padding) = twice(PAD);
    let mut down = measured.summary.height;

    match measured.body {
        Some(body) => {
            let Ok(gap) = gap();

            down = down.saturating_add(gap).saturating_add(body.height);
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
    saying: &CardContent<'_>,
    measured: &Measured,
    wearing: &Wearing,
    at: Point<i32>,
) -> Result<Card, Never> {
    let Ok(tall) = height(measured, saying.value);
    let Ok(worn) = worn(wearing, saying.urgency);
    let Ok(wide) = fitted::<i32, u32>(WIDE);
    let Ok(inner) = x();
    let Ok(inner) = fitted::<i32, u32>(inner);
    let Ok(inset) = fitted::<i32, i32>(EDGE.saturating_add(PAD));
    let Ok(border) = fitted::<i32, u32>(EDGE);
    let Ok(round) = fitted::<i32, u32>(ROUND);
    let Ok(summary_font) = SUMMARY.font();
    let Ok(summary_weight) = SUMMARY.weight();
    let Ok(body_font) = BODY.font();
    let Ok(body_weight) = BODY.weight();

    let mut shapes = vec![Shape::Panel(Panel {
        at,
        size: Size { width: wide, height: tall },
        round: Round(round),
        fill: wearing.panel,
        edge: Border::Of { wide: border, color: worn.edge },
    })];
    let across = at.x.saturating_add(inset);
    let mut down = at.y.saturating_add(inset);

    shapes.push(Shape::Text(Text {
        at: Point { x: across, y: down },
        width: inner,
        said: saying.summary.to_string(),
        weight: summary_weight,
        font: summary_font,
        ink: worn.ink,
    }));

    let Ok(over) = step(measured.summary.height);

    down = down.saturating_add(over);

    match measured.body {
        Some(body) => {
            shapes.push(Shape::Text(Text {
                at: Point { x: across, y: down },
                width: inner,
                said: saying.body.to_string(),
                weight: body_weight,
                font: body_font,
                ink: worn.ink,
            }));

            let Ok(over) = step(body.height);

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
                at: Point { x: across, y: down },
                size: Size { width: inner, height: deep },
                round: Round(round),
                fill: wearing.ground,
                edge: Border::None,
            }));
            shapes.push(Shape::Panel(Panel {
                at: Point { x: across, y: down },
                size: Size { width: filled, height: deep },
                round: Round(round),
                fill: wearing.fill,
                edge: Border::None,
            }));
        }
        None => {},
    }

    Ok(Card { shapes, height: tall })
}

impl Stack {
    pub fn on(&self, at: Point<i32>) -> Result<Option<u32>, Never> {
        Ok(self.touching.iter().find_map(|touching| match touching.panel.covers(at) {
            Ok(console_core_shapes::Covers::Yes) => Some(touching.id),
            Ok(console_core_shapes::Covers::No) | Err(_) => None,
        }))
    }
}

pub fn cards(said: &[(CardContent<'_>, Measured)], wearing: &Wearing) -> Result<Stack, Never> {
    let Ok(wide) = fitted::<i32, u32>(WIDE);
    let mut shapes = Vec::new();
    let mut touching = Vec::new();
    let mut down: u32 = 0;

    let Ok(most) = index(MOST);

    for (saying, measured) in said.iter().take(most) {
        let Ok(at) = fitted::<u32, i32>(down);
        let Ok(card) = card(saying, measured, wearing, Point { x: 0, y: at });

        touching.push(HitRegion {
            id: saying.id,
            panel: Panel {
                at: Point { x: 0, y: at },
                size: Size { width: wide, height: card.height },
                round: Round(0),
                fill: wearing.panel,
                edge: Border::None,
            },
        });
        shapes.extend(card.shapes);

        let Ok(gap) = gap();

        down = down.saturating_add(card.height).saturating_add(gap);
    }

    let Ok(gap) = gap();
    let tall = down.saturating_sub(gap);

    Ok(Stack { shapes, room: Size { width: wide, height: tall }, touching })
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
    use std::collections::BTreeMap;
    use super::*;

    #[test]
    fn a_reading_of_nothing_fills_nothing_and_a_full_one_fills_the_width() {
        assert_eq!(filled(0), Ok(0));
        assert_eq!(filled(100), x());
    }

    #[test]
    fn a_reading_halfway_is_halfway_across() {
        let Ok(across) = x();
        let Ok(half) = filled(50);

        assert!((half.saturating_mul(2).saturating_sub(across)).abs() <= 1, "{half} of {across}");
    }

    #[test]
    fn a_reading_past_either_end_is_the_end_rather_than_a_bar_off_the_card() {
        assert_eq!(filled(-20), Ok(0));
        assert_eq!(filled(4000), x());
    }

    const SPENT: &str = "panel=372c3a\ntext=f7e7f3\nedge=8a7d8e\nsoft=e5cde0\ncoral=ff8f8f\nground=231b26\nfill=723b5f\nnight=1a1320\npink=f9b8d8";

    pub(super) fn wearing() -> Wearing {
        let Ok(spent) = console_core_color::palette::read(SPENT);

        match Wearing::out_of(&spent) {
            Ok(wearing) => wearing,
            Err(why) => panic!("a whole palette should dress a card: {why}"),
        }
    }

    pub(super) fn measured() -> Measured {
        Measured { summary: Size { width: 280, height: 20 }, body: None }
    }

    pub(super) fn saying() -> CardContent<'static> {
        CardContent {
            id: 1,
            summary: "something happened",
            body: "",
            urgency: Urgency::Normal,
            value: None,
        }
    }

    #[test]
    fn a_palette_missing_a_color_names_it_rather_than_drawing_in_black() {
        let mut spent = BTreeMap::new();

        spent.insert("panel".to_string(), "372c3a".to_string());

        assert_eq!(Wearing::out_of(&spent), Err(PaletteError::Absent("text")));
    }

    #[test]
    fn a_palette_spending_something_that_is_not_a_color_says_so() {
        let mut spent = BTreeMap::new();

        for named in SPENDS {
            spent.insert(named.to_string(), "372c3a".to_string());
        }

        spent.insert("edge".to_string(), "mauve".to_string());

        assert_eq!(
            Wearing::out_of(&spent),
            Err(PaletteError::Invalid { color: "edge", value: "mauve".to_string() })
        );
    }

    #[test]
    fn a_card_is_its_edges_its_padding_and_what_it_says() {
        let Ok(down) = height(&measured(), None);

        let Ok(padding) = twice(PAD);
        let Ok(edges) = twice(EDGE);

        assert_eq!(down, 20 + padding + edges);
    }

    #[test]
    fn a_reading_makes_a_card_taller_by_the_bar_and_the_gap_over_it() {
        let Ok(without) = height(&measured(), None);
        let Ok(with) = height(&measured(), Some(50));

        let Ok(gap) = gap();
        let Ok(deep) = deep();

        assert_eq!(with - without, gap + deep);
    }

    #[test]
    fn a_card_with_nothing_but_a_summary_is_a_panel_and_a_run_of_words() {
        let Ok(card) = card(&saying(), &measured(), &wearing(), Point { x: 0, y: 0 });

        assert_eq!(card.shapes.len(), 2, "{:?}", card.shapes);
    }

    #[test]
    fn a_reading_adds_the_bar_and_the_fill_over_it() {
        let saying = CardContent { value: Some(50), ..saying() };
        let Ok(card) = card(&saying, &measured(), &wearing(), Point { x: 0, y: 0 });

        assert_eq!(card.shapes.len(), 4, "{:?}", card.shapes);
    }

    #[test]
    fn what_a_card_says_starts_inside_its_edge_and_its_padding() {
        let Ok(card) = card(&saying(), &measured(), &wearing(), Point { x: 0, y: 0 });
        let inset = EDGE.saturating_add(PAD);
        let words = card.shapes.iter().find_map(|shape| match shape {
            Shape::Text(words) => Some(words.at),
            Shape::Panel(_) | Shape::Picture(_) | Shape::Cropped(_) | Shape::Line(_) | Shape::Clip(_) => None,
        });

        assert_eq!(words, Some(Point { x: inset, y: inset }));
    }

    #[test]
    fn a_low_notification_wears_the_soft_color_on_both_its_edge_and_its_words() {
        let saying = CardContent { urgency: Urgency::Low, ..saying() };
        let Ok(card) = card(&saying, &measured(), &wearing(), Point { x: 0, y: 0 });
        let worn = wearing();
        let ink = card.shapes.iter().find_map(|shape| match shape {
            Shape::Text(words) => Some(words.ink),
            Shape::Panel(_) | Shape::Picture(_) | Shape::Cropped(_) | Shape::Line(_) | Shape::Clip(_) => None,
        });

        assert_eq!(ink, Some(worn.soft));
    }

    #[test]
    fn a_critical_notification_wears_coral_on_its_edge_and_keeps_its_words_readable() {
        let saying = CardContent { urgency: Urgency::Critical, ..saying() };
        let Ok(card) = card(&saying, &measured(), &wearing(), Point { x: 0, y: 0 });
        let worn = wearing();
        let edge = card.shapes.iter().find_map(|shape| match shape {
            Shape::Panel(panel) => Some(panel.edge),
            Shape::Text(_) | Shape::Picture(_) | Shape::Cropped(_) | Shape::Line(_) | Shape::Clip(_) => None,
        });
        let ink = card.shapes.iter().find_map(|shape| match shape {
            Shape::Text(words) => Some(words.ink),
            Shape::Panel(_) | Shape::Picture(_) | Shape::Cropped(_) | Shape::Line(_) | Shape::Clip(_) => None,
        });

        assert!(matches!(edge, Some(console_core_shapes::Edge::Of { color, .. }) if color == worn.coral));
        assert_eq!(ink, Some(worn.text));
    }

    #[test]
    fn no_more_than_three_cards_are_ever_drawn() {
        let said: Vec<(CardContent<'_>, Measured)> =
            (0..6).map(|_| (saying(), measured())).collect();
        let Ok(stack) = cards(&said, &wearing());

        assert_eq!(u32::try_from(stack.shapes.len()).unwrap(), MOST * 2);
        assert_eq!(u32::try_from(stack.touching.len()).unwrap(), MOST);
    }

    #[test]
    fn a_thumb_on_the_second_card_is_on_the_second_card_and_not_the_first() {
        let first = CardContent { id: 7, ..saying() };
        let second = CardContent { id: 9, ..saying() };
        let said = vec![(first, measured()), (second, measured())];
        let Ok(stack) = cards(&said, &wearing());
        let Ok(one) = height(&measured(), None);
        let Ok(gap) = gap();
        let Ok(into) = fitted::<u32, i32>(one.saturating_add(gap).saturating_add(2));
        let Ok(found) = stack.on(Point { x: 10, y: into });

        assert_eq!(found, Some(9));
    }

    #[test]
    fn a_thumb_in_the_gap_between_two_cards_is_on_neither() {
        let said = vec![(saying(), measured()), (CardContent { id: 9, ..saying() }, measured())];
        let Ok(stack) = cards(&said, &wearing());
        let Ok(one) = height(&measured(), None);
        let Ok(into) = fitted::<u32, i32>(one.saturating_add(1));
        let Ok(found) = stack.on(Point { x: 10, y: into });

        assert_eq!(found, None);
    }

    #[test]
    fn a_stack_of_cards_is_as_tall_as_the_cards_and_the_gaps_between_them() {
        let said = vec![(saying(), measured()), (saying(), measured())];
        let Ok(stack) = cards(&said, &wearing());
        let Ok(one) = height(&measured(), None);
        let Ok(wide) = fitted::<i32, u32>(WIDE);

        let Ok(gap) = gap();

        assert_eq!(stack.room.height, one * 2 + gap);
        assert_eq!(stack.room.width, wide);
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
            (saying(), Measured { summary: Size { width: 280, height: 44 }, body: None }),
            (saying(), measured()),
        ];
        let Ok(stack) = cards(&said, &wearing());
        let ended = stack.touching.iter().map(|touching| {
            touching.panel.at.y.saturating_add(
                console_core_number_conversion::fitted::<u32, i32>(touching.panel.size.height)
                    .unwrap_or(0),
            )
        }).max();
        let Ok(room) = fitted::<u32, i32>(stack.room.height);

        assert_eq!(ended, Some(room), "the stack is not as tall as what is in it");
    }
}
