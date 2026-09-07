//! A panel saying what it drew, and where, so a check can press it.
//!
//! Every question ever asked of a surface here was asked of the wrong thing. Is
//! it up, is it the right size, is it on the right layer, is the pixel in the
//! middle the colour the palette says -- and a surface can answer yes to every
//! one of those and still be a thing nobody can use. Y had no mark on any row
//! in any panel for as long as this desktop has existed. The way out of a
//! picture opened over the whole screen was hidden with the strip that carried
//! it. Both were drawn, both were correct, and neither could be pressed.
//!
//! So the panel writes down what it put on the screen: what each row says, and
//! the rectangle of every part of it a hand could land on. That is enough to
//! ask the three questions worth asking, and the first two need no compositor
//! at all:
//!
//!   * is the mark there -- a row that offers something and draws nothing for
//!     it is Y with no answer for a finger;
//!   * is it *on* the screen -- the way out of a full-screen picture was drawn
//!     at a place a margin past the right edge of the glass, which every
//!     screenshot showed and no check could see;
//!   * and does it do anything -- which is a press at the middle of the
//!     rectangle, with `console-point`, and the next line this writes.
//!
//! Each line also says where the highlight is: on the row, beside it on what
//! else that row offers, or nowhere. A press that only moves the highlight
//! changes nothing else a check could see, so without this the d-pad's own
//! answers could be reasoned about and never pressed.
//!
//! A line per draw, appended. The stage that drives a panel presses several
//! times and looks once, so what a check reads is the whole run in order rather
//! than the last frame of it.
//!
//! Off unless asked for. `CONSOLE_PANEL_TELLS` names the file, nothing else
//! turns it on, and a panel on the device writes nothing.

use std::fmt::Write as _;

use console_core_never::Never;
use console_core_number_conversion::fitted;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spot {
    pub name: String,
    pub at: (i32, i32),
    pub big: (i32, i32),
    pub scrolls: Scrolls,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scrolls {
    Yes,
    No,
}

impl Spot {
    pub fn middle(&self) -> Result<(i32, i32), Never> {
        Ok((
            self.at.0.saturating_add(self.big.0.saturating_div(2)),
            self.at.1.saturating_add(self.big.1.saturating_div(2)),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reachable {
    Yes,
    No,
}

pub fn reachable(spot: &Spot, room: (i32, i32)) -> Result<Reachable, Never> {
    let right = spot.at.0.saturating_add(spot.big.0);
    let bottom = spot.at.1.saturating_add(spot.big.1);

    let across = spot.big.0 > 0 && spot.at.0 >= 0 && right <= room.0;

    let down = match spot.scrolls {
        Scrolls::Yes => spot.big.1 > 0,
        Scrolls::No => spot.big.1 > 0 && spot.at.1 >= 0 && bottom <= room.1,
    };

    Ok(match across && down {
        true => Reachable::Yes,
        false => Reachable::No,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offers {
    Yes,
    No,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bare {
    Yes,
    No,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    On,
    Beside,
    No,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub at: usize,
    pub says: String,
    pub aside: String,
    pub offers: Offers,
    pub bare: Bare,
    pub standing: Standing,
    pub spots: Vec<Spot>,
}

impl Line {
    pub fn wearing(&self, name: &str) -> Result<Option<&Spot>, Never> {
        Ok(self.spots.iter().find(|spot| spot.name == name))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    Yes,
    No,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Told {
    pub panel: String,
    pub tab: String,
    pub out: Out,
    pub room: (i32, i32),
    pub lines: Vec<Line>,
    pub spots: Vec<Spot>,
}

impl Told {
    pub fn every_spot(&self) -> Result<Vec<&Spot>, Never> {
        Ok(self.spots.iter().chain(self.lines.iter().flat_map(|line| line.spots.iter())).collect())
    }

    pub fn wearing(&self, name: &str) -> Result<Option<&Spot>, Never> {
        let Ok(every) = self.every_spot();

        Ok(every.into_iter().find(|spot| spot.name == name))
    }

    pub fn line_saying(&self, said: &str) -> Result<Option<&Line>, Never> {
        Ok(self.lines.iter().find(|line| line.says == said))
    }
}

pub const OFFERED: [&str; 7] = ["tab", "shut", "more", "step", "else", "press", "answer"];

pub fn where_to() -> Result<Option<String>, Never> {
    Ok(match std::env::var("CONSOLE_PANEL_TELLS") {
        Ok(said) if !said.is_empty() => Some(said),
        Ok(_) => None,
        Err(std::env::VarError::NotPresent) => None,
        Err(fault) => {
            eprintln!("console-panel: CONSOLE_PANEL_TELLS: {fault}");

            None
        }
    })
}

fn quoted(said: &str) -> Result<String, Never> {
    Ok(serde_json::Value::String(said.to_string()).to_string())
}

fn spots_said(spots: &[Spot]) -> Result<String, Never> {
    let every: Vec<String> = spots
        .iter()
        .map(|spot| {
            let Ok(name) = quoted(&spot.name);

            format!(
                "{{\"name\":{},\"at\":[{},{}],\"big\":[{},{}],\"scrolls\":{}}}",
                name,
                spot.at.0,
                spot.at.1,
                spot.big.0,
                spot.big.1,
                match spot.scrolls {
                    Scrolls::Yes => "true",
                    Scrolls::No => "false",
                }
            )
        })
        .collect();

    Ok(format!("[{}]", every.join(",")))
}

pub fn said(told: &Told) -> Result<String, Never> {
    let mut out = String::new();
    let lines: Vec<String> = told
        .lines
        .iter()
        .map(|line| {
            let Ok(says) = quoted(&line.says);
            let Ok(aside) = quoted(&line.aside);
            let Ok(spots) = spots_said(&line.spots);

            format!(
                "{{\"at\":{},\"says\":{},\"aside\":{},\"offers\":{},\"bare\":{},\
                 \"standing\":{},\"spots\":{}}}",
                line.at,
                says,
                aside,
                match line.offers {
                    Offers::Yes => "true",
                    Offers::No => "false",
                },
                match line.bare {
                    Bare::Yes => "true",
                    Bare::No => "false",
                },
                match line.standing {
                    Standing::On => "\"on\"",
                    Standing::Beside => "\"beside\"",
                    Standing::No => "\"no\"",
                },
                spots
            )
        })
        .collect();
    let Ok(panel) = quoted(&told.panel);
    let Ok(tab) = quoted(&told.tab);
    let Ok(spots) = spots_said(&told.spots);

    let _ = write!(
        out,
        "{{\"panel\":{},\"tab\":{},\"out\":{},\"room\":[{},{}],\"spots\":{},\"lines\":[{}]}}",
        panel,
        tab,
        match told.out {
            Out::Yes => "true",
            Out::No => "false",
        },
        told.room.0,
        told.room.1,
        spots,
        lines.join(",")
    );

    Ok(out)
}

fn number(held: Option<&serde_json::Value>) -> Result<Option<i32>, Never> {
    let Some(held) = held else { return Ok(None) };

    let Some(whole) = held.as_i64() else { return Ok(None) };

    let Ok(whole) = fitted::<i64, i32>(whole);

    Ok(Some(whole))
}

fn pair(held: &serde_json::Value, called: &str) -> Result<Option<(i32, i32)>, Never> {
    let Some(named) = held.get(called) else { return Ok(None) };

    let Some(every) = named.as_array() else { return Ok(None) };

    let Ok(Some(across)) = number(every.first()) else { return Ok(None) };

    let Ok(Some(down)) = number(every.get(1)) else { return Ok(None) };

    Ok(Some((across, down)))
}

fn spot_of(held: &serde_json::Value) -> Result<Option<Spot>, Never> {
    let Some(named) = held.get("name") else { return Ok(None) };

    let Some(name) = named.as_str() else { return Ok(None) };

    let Ok(Some(at)) = pair(held, "at") else { return Ok(None) };

    let Ok(Some(big)) = pair(held, "big") else { return Ok(None) };

    Ok(Some(Spot {
        name: name.to_string(),
        at,
        big,
        scrolls: match held.get("scrolls").and_then(serde_json::Value::as_bool) {
            Some(true) => Scrolls::Yes,
            Some(false) | None => Scrolls::No,
        },
    }))
}

pub fn read(said: &str) -> Result<Told, String> {
    let held: serde_json::Value =
        serde_json::from_str(said).map_err(|fault| format!("{fault}: {said}"))?;

    let Ok(room) = pair(&held, "room");

    let Some(room) = room else { return Err("no room".to_string()) };

    let lines = held.get("lines").and_then(|held| held.as_array()).ok_or("no lines")?;

    Ok(Told {
        panel: held.get("panel").and_then(|held| held.as_str()).unwrap_or_default().to_string(),
        tab: held.get("tab").and_then(|held| held.as_str()).unwrap_or_default().to_string(),
        out: match held.get("out").and_then(|held| held.as_bool()) {
            Some(true) => Out::Yes,
            Some(false) | None => Out::No,
        },
        room,
        spots: held
            .get("spots")
            .and_then(|held| held.as_array())
            .map(|every| {
                every
                    .iter()
                    .filter_map(|held| {
                        let Ok(spot) = spot_of(held);

                        spot
                    })
                    .collect()
            })
            .unwrap_or_default(),
        lines: lines
            .iter()
            .enumerate()
            .map(|(at, held)| Line {
                at: match held.get("at").and_then(|held| held.as_u64()) {
                    Some(held) => {
                        let Ok(held) = fitted::<u64, usize>(held);

                        held
                    },
                    None => at,
                },
                says: held.get("says").and_then(|held| held.as_str()).unwrap_or_default().to_string(),
                aside: held
                    .get("aside")
                    .and_then(|held| held.as_str())
                    .unwrap_or_default()
                    .to_string(),
                offers: match held.get("offers").and_then(|held| held.as_bool()) {
                    Some(true) => Offers::Yes,
                    Some(false) | None => Offers::No,
                },
                bare: match held.get("bare").and_then(|held| held.as_bool()) {
                    Some(true) => Bare::Yes,
                    Some(false) | None => Bare::No,
                },
                standing: match held.get("standing").and_then(|held| held.as_str()) {
                    Some("on") => Standing::On,
                    Some("beside") => Standing::Beside,
                    Some(_) | None => Standing::No,
                },
                spots: held
                    .get("spots")
                    .and_then(|held| held.as_array())
                    .map(|every| {
                every
                    .iter()
                    .filter_map(|held| {
                        let Ok(spot) = spot_of(held);

                        spot
                    })
                    .collect()
            })
                    .unwrap_or_default(),
            })
            .collect(),
    })
}


pub fn every(said: &str) -> Result<Vec<Told>, String> {
    said.lines().filter(|line| !line.trim().is_empty()).map(read).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spot(name: &str, at: (i32, i32), big: (i32, i32)) -> Spot {
        Spot { name: name.to_string(), at, big, scrolls: Scrolls::No }
    }

    fn scrolling(name: &str, at: (i32, i32), big: (i32, i32)) -> Spot {
        Spot { name: name.to_string(), at, big, scrolls: Scrolls::Yes }
    }

    fn told() -> Told {
        Told {
            panel: "viewer-panel".to_string(),
            tab: "Looking".to_string(),
            out: Out::No,
            room: (1024, 640),
            spots: vec![spot("shut", (954, 14), (56, 44))],
            lines: vec![
                Line {
                    at: 0,
                    says: String::new(),
                    aside: String::new(),
                    offers: Offers::Yes,
                    bare: Bare::Yes,
                    standing: Standing::No,
                    spots: Vec::new(),
                },
                Line {
                    at: 1,
                    says: "beach.jpg".to_string(),
                    aside: "2 of 7".to_string(),
                    offers: Offers::Yes,
                    bare: Bare::No,
                    standing: Standing::Beside,
                    spots: vec![spot("else", (900, 300), (40, 30))],
                },
            ],
        }
    }

    #[test]
    fn what_was_drawn_survives_the_trip_through_a_file() {
        let Ok(said) = said(&told());

        assert_eq!(read(&said), Ok(told()));
    }

    #[test]
    fn a_run_is_read_back_as_the_draws_it_was() {
        let Ok(said) = said(&told());
        let run = format!("{said}\n{said}\n");

        assert_eq!(every(&run).map(|every| every.len()), Ok(2));
    }

    #[test]
    fn a_line_that_is_not_this_files_own_is_said_rather_than_skipped() {
        assert!(every("{\"panel\":\"x\"}").is_err());
        assert!(read("not json at all").is_err());
    }

    #[test]
    fn a_mark_is_found_by_the_name_it_is_drawn_under() {
        let told = told();

        let Ok(Some(shut)) = told.wearing("shut") else { panic!("the way out is drawn") };

        let Ok(middle) = shut.middle();
        let Ok(Some(line)) = told.line_saying("beach.jpg") else { panic!("the row is drawn") };

        let Ok(worn) = line.wearing("else");

        assert_eq!(middle, (982, 36));
        assert!(worn.is_some());
        assert_eq!(told.wearing("nothing-is-called-this"), Ok(None));
    }

    #[test]
    fn a_mark_inside_the_screen_can_be_reached_and_one_hanging_off_it_cannot() {
        let room = (1024, 640);
        assert_eq!(reachable(&spot("shut", (954, 14), (56, 44)), room), Ok(Reachable::Yes));
        assert_eq!(reachable(&spot("shut", (0, 0), (1024, 640)), room), Ok(Reachable::Yes));
    }

    #[test]
    fn the_fault_this_was_written_for_is_one_this_can_see() {
        let room = (1024, 640);
        assert_eq!(reachable(&spot("shut", (982, 14), (56, 44)), room), Ok(Reachable::No));
        assert_eq!(reachable(&spot("shut", (-4, 14), (56, 44)), room), Ok(Reachable::No));
        assert_eq!(reachable(&spot("shut", (900, 620), (56, 44)), room), Ok(Reachable::No));
    }

    #[test]
    fn a_mark_the_thumb_can_scroll_to_is_a_mark_a_hand_can_reach() {
        let room = (1024, 640);
        assert_eq!(reachable(&scrolling("else", (865, 4805), (44, 29)), room), Ok(Reachable::Yes));
        assert_eq!(reachable(&spot("else", (865, 4805), (44, 29)), room), Ok(Reachable::No));
    }

    #[test]
    fn scrolling_does_not_excuse_a_mark_off_the_side() {
        let room = (1024, 640);
        assert_eq!(reachable(&scrolling("else", (1000, 300), (44, 29)), room), Ok(Reachable::No));
        assert_eq!(reachable(&scrolling("else", (-4, 300), (44, 29)), room), Ok(Reachable::No));
    }

    #[test]
    fn a_mark_with_no_size_is_a_mark_nothing_can_land_on() {
        assert_eq!(reachable(&spot("else", (10, 10), (0, 30)), (1024, 640)), Ok(Reachable::No));
        assert_eq!(reachable(&spot("else", (10, 10), (40, 0)), (1024, 640)), Ok(Reachable::No));
    }

    #[test]
    fn a_row_that_offers_something_and_wears_nothing_is_the_fault_being_looked_for() {
        let told = told();

        for line in &told.lines {
            let Ok(worn) = line.wearing(crate::marks::named::ELSE);

            let answered = worn.is_some() || line.bare == Bare::Yes;

            assert!(answered, "row {} offers something a finger cannot reach", line.at);
        }
    }

    #[test]
    fn every_part_a_hand_is_offered_is_one_the_walk_looks_for() {
        for name in [
            crate::marks::named::TAB,
            crate::marks::named::SHUT,
            crate::marks::named::MORE,
            crate::marks::named::STEP,
            crate::marks::named::ELSE,
            crate::marks::named::PRESS,
            crate::marks::named::ANSWER,
        ] {
            assert!(OFFERED.contains(&name), "{name} is drawn to be pressed and never looked for");
        }
    }
}
