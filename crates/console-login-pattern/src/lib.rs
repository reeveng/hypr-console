//! The dots a person connects to log in.
//!
//! **Not a password field, because there is no keyboard.** The only way to
//! type on this machine is a keyboard drawn over a session, and the way in is
//! the one screen with no session under it. So the secret is something a hand
//! can say: dots, joined by a finger drawn across the screen the way a phone
//! is unlocked, or by the pad -- a cursor the d-pad moves from one dot to the
//! nearest in the direction pressed, A to add the dot under it, B to take the
//! last one back, and Login at the end. A finger that lifts is Login. Each dot
//! joins the one before it, each dot is joined once, and a pattern need not
//! touch them all.
//!
//! **Each dot is a letter.** The pattern is kept and checked as the word its
//! dots spell. It is not the account's password: it once was, and that made a
//! forgotten password a greeter nobody could get past.
//! `console_login_window::stored_pattern` is where it is kept instead.
//!
//! **The dots stand on a ring.** A finger drawn from one dot to another must
//! not cross a third on the way, or the pattern it draws is not the one the
//! hand meant. Rows of dots always have a third between two of them, and
//! staggering the rows only narrows the gap a line has to thread; with every
//! dot on the edge of an ellipse no line between two of them comes near a
//! third, which is what a search for the widest gap comes back to by itself.
//! Nine is as many as a ring this size holds with a finger's width to spare
//! either side of every line, and nine is the number a phone has taught every
//! hand. A finger moving fast is sampled far apart, so what joins is every dot
//! the line between two samples passes over, in the order it passes them.
//!
//! **The ring is fixed.** The same dot in the same place every time is what
//! lets a hand remember a pattern, so the layout is a constant rather than
//! anything drawn fresh. It is in points, in a room of its own, and whoever
//! draws it scales the room.
//!
//! The d-pad goes to the dot that is furthest along the way it was pressed for
//! the least distance off it: along plus twice across, which is how television
//! remotes have walked a screen of targets for as long as there have been any.
//! Login is a target like the dots, below them, so down from the bottom of the
//! ring is the way to it and up from it is the way back.

use std::collections::BTreeSet;
use std::fmt;

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::index;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dot {
    pub key: char,
    pub centre: Point<i32>,
    pub size: Size<u32>,
}

const SIZE: Size<u32> = Size { width: 44, height: 44 };

const REACH: f64 = 22.0;

pub const DOTS: [Dot; 9] = [
    Dot { key: 'a', centre: Point { x: 220, y: 22 }, size: SIZE },
    Dot { key: 'b', centre: Point { x: 347, y: 57 }, size: SIZE },
    Dot { key: 'c', centre: Point { x: 415, y: 144 }, size: SIZE },
    Dot { key: 'd', centre: Point { x: 391, y: 244 }, size: SIZE },
    Dot { key: 'e', centre: Point { x: 288, y: 309 }, size: SIZE },
    Dot { key: 'f', centre: Point { x: 152, y: 309 }, size: SIZE },
    Dot { key: 'g', centre: Point { x: 49, y: 244 }, size: SIZE },
    Dot { key: 'h', centre: Point { x: 25, y: 144 }, size: SIZE },
    Dot { key: 'i', centre: Point { x: 93, y: 57 }, size: SIZE },
];

pub const LOGIN: Point<i32> = Point { x: 220, y: 385 };

pub const LOGIN_SIZE: Size<u32> = Size { width: 195, height: 60 };

pub const ROOM: Size<u32> = Size { width: 440, height: 420 };

const FIRST: u32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Dot(u32),
    Login,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn spelled(&self) -> Result<&str, Never> {
        Ok(&self.0)
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(to, "Secret(..)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touch {
    Down(Point<i32>),
    Moved(Point<i32>),
    Up,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    pub at: Target,
    pub path: Vec<u32>,
    pub finger: Option<Point<i32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clicked {
    Traced(Pattern),
    Submitted(Secret),
}

impl Pattern {
    pub fn new() -> Result<Pattern, Never> {
        Ok(Pattern { at: Target::Dot(FIRST), path: Vec::new(), finger: None })
    }

    pub fn cleared(&self) -> Result<Pattern, Never> {
        Ok(Pattern { at: self.at, path: Vec::new(), finger: None })
    }

    pub fn lifted(&self) -> Result<Pattern, Never> {
        Ok(Pattern { at: self.at, path: self.path.clone(), finger: None })
    }

    pub fn moved(&self, direction: Direction) -> Result<Pattern, Never> {
        let Ok(at) = toward(self.at, direction);

        Ok(Pattern { at, path: self.path.clone(), finger: None })
    }

    pub fn touched(&self, touch: Touch) -> Result<Clicked, Never> {
        let Ok(lifted) = self.lifted();

        match (touch, self.finger) {
            (Touch::Down(at), _) => {
                let Ok(login) = on_login(at);

                match login {
                    Some(login) => Pattern { at: login, path: self.path.clone(), finger: None }.clicked(),
                    None => {
                        let Ok(start) = self.cleared();
                        let Ok(drawn) = start.crossed(at, at);

                        Ok(Clicked::Traced(drawn))
                    }
                }
            }
            (Touch::Moved(to), Some(from)) => {
                let Ok(drawn) = self.crossed(from, to);

                Ok(Clicked::Traced(drawn))
            }
            (Touch::Up, Some(_)) => match self.path.is_empty() {
                true => Ok(Clicked::Traced(lifted)),
                false => {
                    let Ok(secret) = spelled(&self.path);

                    Ok(Clicked::Submitted(secret))
                }
            },
            (Touch::Moved(_) | Touch::Up, None) => Ok(Clicked::Traced(lifted)),
        }
    }

    fn crossed(&self, from: Point<i32>, to: Point<i32>) -> Result<Pattern, Never> {
        let Ok(met) = passed_over(&self.path, from, to);
        let at = match met.last() {
            Some(last) => Target::Dot(*last),
            None => self.at,
        };
        let mut path = self.path.clone();

        path.extend(met);

        Ok(Pattern { at, path, finger: Some(to) })
    }

    pub fn clicked(&self) -> Result<Clicked, Never> {
        Ok(match (self.at, self.path.last()) {
            (Target::Login, None) => Clicked::Traced(self.clone()),
            (Target::Login, Some(_)) => {
                let Ok(secret) = spelled(&self.path);

                Clicked::Submitted(secret)
            }
            (Target::Dot(on), _) => {
                let Ok(drawn) = joined(self, on);

                Clicked::Traced(drawn)
            }
        })
    }

    pub fn undone(&self) -> Result<Pattern, Never> {
        let mut path = self.path.clone();

        path.pop();

        Ok(Pattern { at: self.at, path, finger: None })
    }
}

fn joined(pattern: &Pattern, on: u32) -> Result<Pattern, Never> {
    let mut path = pattern.path.clone();

    match pattern.path.contains(&on) {
        true => {}
        false => path.push(on),
    }

    Ok(Pattern { at: pattern.at, path, finger: None })
}

fn on_login(at: Point<i32>) -> Result<Option<Target>, Never> {
    let across = i64::from(at.x).saturating_sub(i64::from(LOGIN.x)).saturating_abs();
    let down = i64::from(at.y).saturating_sub(i64::from(LOGIN.y)).saturating_abs();
    let half_wide = i64::from(LOGIN_SIZE.width.saturating_div(2));
    let half_tall = i64::from(LOGIN_SIZE.height.saturating_div(2));

    Ok(match (across <= half_wide, down <= half_tall) {
        (true, true) => Some(Target::Login),
        (true, false) | (false, _) => None,
    })
}

fn passed_over(path: &[u32], from: Point<i32>, to: Point<i32>) -> Result<Vec<u32>, Never> {
    let start = Point { x: f64::from(from.x), y: f64::from(from.y) };
    let run = Point { x: f64::from(to.x) - start.x, y: f64::from(to.y) - start.y };
    let long = run.x * run.x + run.y * run.y;
    let joined: BTreeSet<u32> = path.iter().copied().collect();
    let mut met: Vec<(f64, u32)> = (0_u32..)
        .zip(DOTS)
        .filter(|(at, _)| !joined.contains(at))
        .filter_map(|(at, dot)| {
            let there = Point { x: f64::from(dot.centre.x) - start.x, y: f64::from(dot.centre.y) - start.y };
            let along = match long > 0.0 {
                true => ((there.x * run.x + there.y * run.y) / long).clamp(0.0, 1.0),
                false => 0.0,
            };
            let off = Point { x: there.x - along * run.x, y: there.y - along * run.y };

            match off.x * off.x + off.y * off.y <= REACH * REACH {
                true => Some((along, at)),
                false => None,
            }
        })
        .collect();

    met.sort_by(|one, other| one.0.total_cmp(&other.0));

    Ok(met.into_iter().map(|(_, at)| at).collect())
}

fn spelled(path: &[u32]) -> Result<Secret, Never> {
    let keys = path.iter().filter_map(|at| {
        let Ok(dot) = dot(*at);

        dot
    });

    Ok(Secret(keys.map(|dot| dot.key).collect()))
}

pub fn dot(at: u32) -> Result<Option<Dot>, Never> {
    let Ok(at) = index(at);

    Ok(DOTS.get(at).copied())
}

pub fn centre(target: Target) -> Result<Option<Point<i32>>, Never> {
    Ok(match target {
        Target::Dot(at) => {
            let Ok(dot) = dot(at);

            dot.map(|dot| dot.centre)
        }
        Target::Login => Some(LOGIN),
    })
}

pub fn toward(from: Target, direction: Direction) -> Result<Target, Never> {
    let Ok(here) = centre(from);
    let here = match here {
        Some(here) => here,
        None => return Ok(from),
    };
    let every = (0..).zip(DOTS).map(|(at, _dot)| Target::Dot(at)).chain([Target::Login]);
    let best = every
        .filter(|target| *target != from)
        .filter_map(|target| {
            let Ok(there) = centre(target);
            let cost = match there {
                Some(there) => {
                    let Ok(cost) = cost(here, there, direction);

                    cost
                }
                None => None,
            };

            cost.map(|cost| (cost, target))
        })
        .min_by_key(|(cost, _)| *cost);

    Ok(match best {
        Some((_, target)) => target,
        None => from,
    })
}

fn cost(here: Point<i32>, there: Point<i32>, direction: Direction) -> Result<Option<i64>, Never> {
    let across = i64::from(there.x).saturating_sub(i64::from(here.x));
    let down = i64::from(there.y).saturating_sub(i64::from(here.y));
    let (along, off) = match direction {
        Direction::Up => (down.saturating_neg(), across),
        Direction::Down => (down, across),
        Direction::Left => (across.saturating_neg(), down),
        Direction::Right => (across, down),
    };

    Ok(match along > 0 {
        true => Some(along.saturating_add(off.saturating_abs().saturating_mul(2))),
        false => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_core_number_conversion::toward_zero_i32;

    const WOBBLE: f64 = 10.0;

    fn at(key: char) -> Target {
        match (0..).zip(DOTS).find(|(_, dot)| dot.key == key) {
            Some((at, _)) => Target::Dot(at),
            None => panic!("no dot for {key}"),
        }
    }

    fn number(key: char) -> u32 {
        match at(key) {
            Target::Dot(number) => number,
            Target::Login => panic!("{key} is Login"),
        }
    }

    fn centre_of(key: char) -> Point<i32> {
        match centre(at(key)) {
            Ok(Some(centre)) => centre,
            Ok(None) => panic!("no centre for {key}"),
        }
    }

    fn walked(from: Target, presses: &[Direction]) -> Target {
        presses.iter().fold(from, |here, direction| {
            let Ok(next) = toward(here, *direction);

            next
        })
    }

    fn drawn(pattern: Pattern) -> Pattern {
        let Ok(clicked) = pattern.clicked();

        match clicked {
            Clicked::Traced(pattern) => pattern,
            Clicked::Submitted(_) => panic!("submitted before Login"),
        }
    }

    fn touched(pattern: &Pattern, touches: &[Touch]) -> Clicked {
        touches.iter().fold(Clicked::Traced(pattern.clone()), |clicked, touch| match clicked {
            Clicked::Traced(pattern) => {
                let Ok(next) = pattern.touched(*touch);

                next
            }
            Clicked::Submitted(_) => panic!("submitted before the last touch"),
        })
    }

    fn fresh() -> Pattern {
        let Ok(pattern) = Pattern::new();

        pattern
    }

    fn spelled_by(clicked: &Clicked) -> String {
        match clicked {
            Clicked::Submitted(secret) => {
                let Ok(spelled) = secret.spelled();

                spelled.to_string()
            }
            Clicked::Traced(_) => panic!("nothing was submitted"),
        }
    }

    fn path_of(clicked: &Clicked) -> Vec<u32> {
        match clicked {
            Clicked::Traced(pattern) => pattern.path.clone(),
            Clicked::Submitted(_) => panic!("submitted while drawing"),
        }
    }

    #[test]
    fn every_key_is_a_different_letter() {
        let mut keys: Vec<char> = DOTS.iter().map(|dot| dot.key).collect();

        keys.sort_unstable();
        keys.dedup();

        assert_eq!(keys.len(), DOTS.len());
    }

    #[test]
    fn every_dot_sits_inside_the_room() {
        let inside = |across: i32, down: i32| {
            across >= 0 && down >= 0 && across.unsigned_abs() <= ROOM.width && down.unsigned_abs() <= ROOM.height
        };

        assert!(DOTS.iter().all(|dot| inside(dot.centre.x, dot.centre.y)));
        assert!(inside(LOGIN.x, LOGIN.y));
    }

    #[test]
    fn a_line_from_any_dot_to_any_other_crosses_no_third() {
        for (one, from) in (0_u32..).zip(DOTS) {
            for (other, to) in (0_u32..).zip(DOTS).filter(|(other, _)| *other != one) {
                let Ok(met) = passed_over(&[one], from.centre, to.centre);

                assert_eq!(met, vec![other], "{} to {} passes over another dot", from.key, to.key);
            }
        }
    }

    #[test]
    fn a_hand_that_wanders_off_the_line_still_crosses_no_third() {
        for (one, from) in (0_u32..).zip(DOTS) {
            for (other, to) in (0_u32..).zip(DOTS).filter(|(other, _)| *other != one) {
                let run = (f64::from(to.centre.x - from.centre.x), f64::from(to.centre.y - from.centre.y));
                let long = run.0.hypot(run.1);
                let aside = |point: Point<i32>, side: f64| {
                    let Ok(across) = toward_zero_i32((side * WOBBLE * run.1 / long).round());
                    let Ok(down) = toward_zero_i32((side * WOBBLE * run.0 / long).round());

                    Point { x: point.x + across, y: point.y - down }
                };

                for side in [-1.0, 1.0] {
                    let Ok(met) = passed_over(&[one, other], aside(from.centre, side), aside(to.centre, side));

                    assert!(met.is_empty(), "{} to {} drawn {WOBBLE} aside meets another dot", from.key, to.key);
                }
            }
        }
    }

    #[test]
    fn every_dot_and_login_can_be_reached_from_the_first() {
        let start = fresh();
        let mut reached = vec![start.at];
        let mut frontier = vec![start.at];
        let every = [Direction::Up, Direction::Down, Direction::Left, Direction::Right];

        while let Some(here) = frontier.pop() {
            for direction in every {
                let Ok(next) = toward(here, direction);

                match reached.contains(&next) {
                    true => {}
                    false => {
                        reached.push(next);
                        frontier.push(next);
                    }
                }
            }
        }

        assert_eq!(reached.len(), DOTS.len().saturating_add(1));
    }

    #[test]
    fn down_from_the_bottom_of_the_ring_is_login_and_up_from_login_is_back() {
        let Ok(below) = toward(at('e'), Direction::Down);
        let Ok(above) = toward(Target::Login, Direction::Up);

        assert_eq!(below, Target::Login);
        assert!(matches!(above, Target::Dot(_)));
    }

    #[test]
    fn a_step_goes_to_the_nearest_dot_the_way_it_was_pressed() {
        assert_eq!(walked(at('a'), &[Direction::Right]), at('b'));
        assert_eq!(walked(at('a'), &[Direction::Left]), at('i'));
        assert_eq!(walked(at('e'), &[Direction::Left]), at('f'));
    }

    #[test]
    fn nothing_that_way_is_staying_put() {
        assert_eq!(walked(at('a'), &[Direction::Up]), at('a'));
    }

    #[test]
    fn a_pattern_spells_the_keys_of_its_dots_in_order() {
        let pattern = drawn(Pattern { at: at('a'), path: Vec::new(), finger: None });
        let pattern = drawn(Pattern { at: at('c'), path: pattern.path, finger: None });
        let pattern = drawn(Pattern { at: at('b'), path: pattern.path, finger: None });
        let Ok(clicked) = Pattern { at: Target::Login, path: pattern.path, finger: None }.clicked();

        assert_eq!(spelled_by(&clicked), "acb");
    }

    #[test]
    fn a_dot_already_in_the_pattern_is_not_joined_again() {
        let pattern = drawn(Pattern { at: at('a'), path: vec![number('a'), number('c')], finger: None });

        assert_eq!(pattern.path, vec![number('a'), number('c')]);
    }

    #[test]
    fn login_with_nothing_drawn_submits_nothing() {
        let Ok(clicked) = Pattern { at: Target::Login, path: Vec::new(), finger: None }.clicked();

        assert!(matches!(clicked, Clicked::Traced(_)));
    }

    #[test]
    fn b_takes_the_last_dot_back() {
        let pattern = drawn(Pattern { at: at('a'), path: vec![1, 2], finger: None });
        let Ok(undone) = pattern.undone();

        assert_eq!(undone.path, vec![1, 2]);
    }

    #[test]
    fn a_secret_does_not_print_itself() {
        let Ok(clicked) = Pattern { at: Target::Login, path: vec![0], finger: None }.clicked();

        assert_eq!(format!("{clicked:?}"), "Submitted(Secret(..))");
    }

    #[test]
    fn a_finger_joins_each_dot_it_is_drawn_over_and_lifting_it_submits() {
        let touches = [
            Touch::Down(centre_of('a')),
            Touch::Moved(centre_of('b')),
            Touch::Moved(centre_of('c')),
            Touch::Moved(centre_of('d')),
            Touch::Up,
        ];

        assert_eq!(spelled_by(&touched(&fresh(), &touches)), "abcd");
    }

    #[test]
    fn a_finger_follows_every_sample_and_the_line_ends_under_it() {
        let halfway = Point { x: 300, y: 100 };
        let clicked = touched(&fresh(), &[Touch::Down(centre_of('a')), Touch::Moved(halfway)]);

        match clicked {
            Clicked::Traced(pattern) => {
                assert_eq!(pattern.path, vec![number('a')]);
                assert_eq!(pattern.finger, Some(halfway));
            }
            Clicked::Submitted(_) => panic!("submitted while drawing"),
        }
    }

    #[test]
    fn a_finger_sampled_past_a_dot_joins_it_on_the_way() {
        let a = centre_of('a');
        let b = centre_of('b');
        let beyond = Point { x: b.x + (b.x - a.x) / 2, y: b.y + (b.y - a.y) / 2 };
        let clicked = touched(&fresh(), &[Touch::Down(a), Touch::Moved(beyond)]);

        assert_eq!(path_of(&clicked), vec![number('a'), number('b')]);
    }

    #[test]
    fn a_finger_drawn_back_over_joined_dots_joins_nothing() {
        let touches = [
            Touch::Down(centre_of('a')),
            Touch::Moved(centre_of('b')),
            Touch::Moved(centre_of('c')),
            Touch::Moved(centre_of('a')),
        ];

        assert_eq!(path_of(&touched(&fresh(), &touches)), vec![number('a'), number('b'), number('c')]);
    }

    #[test]
    fn a_touch_that_meets_no_dot_submits_nothing() {
        let clicked = touched(&fresh(), &[Touch::Down(Point { x: 220, y: 170 }), Touch::Up]);

        match clicked {
            Clicked::Traced(pattern) => {
                assert!(pattern.path.is_empty());
                assert_eq!(pattern.finger, None);
            }
            Clicked::Submitted(_) => panic!("an empty touch submitted"),
        }
    }

    #[test]
    fn a_new_touch_starts_a_new_pattern() {
        let drawn = Pattern { at: at('b'), path: vec![number('a'), number('b')], finger: None };

        assert_eq!(path_of(&touched(&drawn, &[Touch::Down(centre_of('e'))])), vec![number('e')]);
    }

    #[test]
    fn touching_login_submits_what_the_pad_drew() {
        let drawn = Pattern { at: at('b'), path: vec![number('a'), number('b')], finger: None };

        assert_eq!(spelled_by(&touched(&drawn, &[Touch::Down(LOGIN)])), "ab");
    }
}
