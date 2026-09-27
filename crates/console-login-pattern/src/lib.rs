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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub fn as_str(&self) -> Result<&str, Never> {
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

    pub fn touch(&self, touch: Touch) -> Result<Clicked, Never> {
        let Ok(lifted) = self.lifted();

        match (touch, self.finger) {
            (Touch::Down(at), _) => {
                let Ok(login) = on_login(at);

                match login {
                    Some(login) => Pattern { at: login, path: self.path.clone(), finger: None }.click(),
                    None => {
                        let Ok(start) = self.cleared();
                        let Ok(drawn) = start.cross(at, at);

                        Ok(Clicked::Traced(drawn))
                    }
                }
            }
            (Touch::Moved(to), Some(from)) => {
                let Ok(drawn) = self.cross(from, to);

                Ok(Clicked::Traced(drawn))
            }
            (Touch::Up, Some(_)) => match self.path.is_empty() {
                true => Ok(Clicked::Traced(lifted)),
                false => {
                    let Ok(secret) = secret_from(&self.path);

                    Ok(Clicked::Submitted(secret))
                }
            },
            (Touch::Moved(_) | Touch::Up, None) => Ok(Clicked::Traced(lifted)),
        }
    }

    fn cross(&self, from: Point<i32>, to: Point<i32>) -> Result<Pattern, Never> {
        let Ok(met) = passed_over(&self.path, from, to);
        let at = match met.last() {
            Some(last) => Target::Dot(*last),
            None => self.at,
        };
        let mut path = self.path.clone();

        path.extend(met);

        Ok(Pattern { at, path, finger: Some(to) })
    }

    pub fn click(&self) -> Result<Clicked, Never> {
        Ok(match (self.at, self.path.last()) {
            (Target::Login, None) => Clicked::Traced(self.clone()),
            (Target::Login, Some(_)) => {
                let Ok(secret) = secret_from(&self.path);

                Clicked::Submitted(secret)
            }
            (Target::Dot(on), _) => {
                let Ok(drawn) = join_dot(self, on);

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

fn join_dot(pattern: &Pattern, on: u32) -> Result<Pattern, Never> {
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

fn secret_from(path: &[u32]) -> Result<Secret, Never> {
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
    use std::collections::HashSet;
    use std::error::Error;

    type Failure = Box<dyn Error>;

    const WOBBLE: f64 = 10.0;

    fn at(key: char) -> Result<Target, Failure> {
        match (0..).zip(DOTS).find(|(_, dot)| dot.key == key) {
            Some((at, _)) => Ok(Target::Dot(at)),
            None => Err(Box::from(format!("no dot for {key}"))),
        }
    }

    fn number(key: char) -> Result<u32, Failure> {
        let target = at(key)?;

        match target {
            Target::Dot(number) => Ok(number),
            Target::Login => Err(Box::from(format!("{key} is Login"))),
        }
    }

    fn centre_of(key: char) -> Result<Point<i32>, Failure> {
        let target = at(key)?;
        let Ok(found) = centre(target);

        found.ok_or_else(|| Box::from(format!("no centre for {key}")))
    }

    #[cfg_attr(
        dylint_lib = "explicit051_no_machine_width",
        allow(
            explicit051_no_machine_width,
            reason = "an array's length is a usize by the language, and the array is what lets each test name its keys where it destructures them"
        )
    )]
    fn each<T, const N: usize>(keys: [char; N], one: fn(char) -> Result<T, Failure>) -> Result<[T; N], Failure> {
        let found = keys.into_iter().map(one).collect::<Result<Vec<T>, Failure>>()?;
        let found: [T; N] = found.try_into().map_err(|_not_as_many| "as many answers as keys")?;

        Ok(found)
    }

    fn walked(from: Target, presses: &[Direction]) -> Result<Target, Never> {
        let mut here = from;

        for direction in presses {
            let Ok(next) = toward(here, *direction);

            here = next;
        }

        Ok(here)
    }

    fn drawn(pattern: Pattern) -> Result<Pattern, Failure> {
        let Ok(clicked) = pattern.click();

        match clicked {
            Clicked::Traced(pattern) => Ok(pattern),
            Clicked::Submitted(_) => Err(Box::from("submitted before Login")),
        }
    }

    fn touch_all(pattern: &Pattern, touches: &[Touch]) -> Result<Clicked, Failure> {
        let mut clicked = Clicked::Traced(pattern.clone());

        for touch in touches {
            let Ok(next) = match clicked {
                Clicked::Traced(pattern) => pattern.touch(*touch),
                Clicked::Submitted(_) => return Err(Box::from("submitted before the last touch")),
            };

            clicked = next;
        }

        Ok(clicked)
    }

    fn spelled_by(clicked: &Clicked) -> Result<String, Failure> {
        match clicked {
            Clicked::Submitted(secret) => {
                let Ok(spelled) = secret.as_str();

                Ok(spelled.to_string())
            }
            Clicked::Traced(_) => Err(Box::from("nothing was submitted")),
        }
    }

    fn path_of(clicked: &Clicked) -> Result<Vec<u32>, Failure> {
        match clicked {
            Clicked::Traced(pattern) => Ok(pattern.path.clone()),
            Clicked::Submitted(_) => Err(Box::from("submitted while drawing")),
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
                let run = (
                    f64::from(to.centre.x.saturating_sub(from.centre.x)),
                    f64::from(to.centre.y.saturating_sub(from.centre.y)),
                );
                let long = run.0.hypot(run.1);
                let aside = |point: Point<i32>, side: f64| {
                    let Ok(across) = toward_zero_i32((side * WOBBLE * run.1 / long).round());
                    let Ok(down) = toward_zero_i32((side * WOBBLE * run.0 / long).round());

                    Point { x: point.x.saturating_add(across), y: point.y.saturating_sub(down) }
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
        let Ok(start) = Pattern::new();
        let every = [Direction::Up, Direction::Down, Direction::Left, Direction::Right];
        let grown = std::iter::successors(Some(HashSet::from([start.at])), |reached: &HashSet<Target>| {
            let wider: HashSet<Target> = reached
                .iter()
                .flat_map(|here| {
                    every.map(|direction| {
                        let Ok(next) = toward(*here, direction);

                        next
                    })
                })
                .chain(reached.iter().copied())
                .collect();

            match wider.len() > reached.len() {
                true => Some(wider),
                false => None,
            }
        })
        .last();
        let reached = match grown {
            Some(reached) => reached,
            None => HashSet::new(),
        };

        assert_eq!(reached.len(), DOTS.len().saturating_add(1));
    }

    #[test]
    fn down_from_the_bottom_of_the_ring_is_login_and_up_from_login_is_back() -> Result<(), Failure> {
        let from = at('e')?;
        let Ok(below) = toward(from, Direction::Down);
        let Ok(above) = toward(Target::Login, Direction::Up);

        assert_eq!(below, Target::Login);
        assert!(matches!(above, Target::Dot(_)));

        Ok(())
    }

    #[test]
    fn a_step_goes_to_the_nearest_dot_the_way_it_was_pressed() -> Result<(), Failure> {
        let [a, b, e, f, i] = each(['a', 'b', 'e', 'f', 'i'], at)?;

        assert_eq!(walked(a, &[Direction::Right]), Ok(b));
        assert_eq!(walked(a, &[Direction::Left]), Ok(i));
        assert_eq!(walked(e, &[Direction::Left]), Ok(f));

        Ok(())
    }

    #[test]
    fn nothing_that_way_is_staying_put() -> Result<(), Failure> {
        let a = at('a')?;

        assert_eq!(walked(a, &[Direction::Up]), Ok(a));

        Ok(())
    }

    #[test]
    fn a_pattern_spells_the_keys_of_its_dots_in_order() -> Result<(), Failure> {
        let [a, b, c] = each(['a', 'b', 'c'], at)?;
        let pattern = drawn(Pattern { at: a, path: Vec::new(), finger: None })?;
        let pattern = drawn(Pattern { at: c, path: pattern.path, finger: None })?;
        let pattern = drawn(Pattern { at: b, path: pattern.path, finger: None })?;
        let Ok(clicked) = Pattern { at: Target::Login, path: pattern.path, finger: None }.click();
        let spelled = spelled_by(&clicked)?;

        assert_eq!(spelled, "acb");

        Ok(())
    }

    #[test]
    fn a_dot_already_in_the_pattern_is_not_joined_again() -> Result<(), Failure> {
        let joined = each(['a', 'c'], number)?;
        let a = at('a')?;
        let pattern = drawn(Pattern { at: a, path: joined.to_vec(), finger: None })?;

        assert_eq!(pattern.path, joined);

        Ok(())
    }

    #[test]
    fn login_with_nothing_drawn_submits_nothing() {
        let Ok(clicked) = Pattern { at: Target::Login, path: Vec::new(), finger: None }.click();

        assert!(matches!(clicked, Clicked::Traced(_)));
    }

    #[test]
    fn b_takes_the_last_dot_back() -> Result<(), Failure> {
        let a = at('a')?;
        let pattern = drawn(Pattern { at: a, path: vec![1, 2], finger: None })?;
        let Ok(undone) = pattern.undone();

        assert_eq!(undone.path, vec![1, 2]);

        Ok(())
    }

    #[test]
    fn a_secret_does_not_print_itself() {
        let Ok(clicked) = Pattern { at: Target::Login, path: vec![0], finger: None }.click();

        assert_eq!(format!("{clicked:?}"), "Submitted(Secret(..))");
    }

    #[test]
    fn a_finger_joins_each_dot_it_is_drawn_over_and_lifting_it_submits() -> Result<(), Failure> {
        let Ok(fresh) = Pattern::new();
        let [a, b, c, d] = each(['a', 'b', 'c', 'd'], centre_of)?;
        let touches = [Touch::Down(a), Touch::Moved(b), Touch::Moved(c), Touch::Moved(d), Touch::Up];
        let clicked = touch_all(&fresh, &touches)?;
        let spelled = spelled_by(&clicked)?;

        assert_eq!(spelled, "abcd");

        Ok(())
    }

    #[test]
    fn a_finger_follows_every_sample_and_the_line_ends_under_it() -> Result<(), Failure> {
        let Ok(fresh) = Pattern::new();
        let halfway = Point { x: 300, y: 100 };
        let from = centre_of('a')?;
        let clicked = touch_all(&fresh, &[Touch::Down(from), Touch::Moved(halfway)])?;
        let a = number('a')?;

        match clicked {
            Clicked::Traced(pattern) => {
                assert_eq!(pattern.path, vec![a]);
                assert_eq!(pattern.finger, Some(halfway));
            }
            Clicked::Submitted(_) => return Err(Box::from("submitted while drawing")),
        }

        Ok(())
    }

    #[test]
    fn a_finger_sampled_past_a_dot_joins_it_on_the_way() -> Result<(), Failure> {
        let Ok(fresh) = Pattern::new();
        let [a, b] = each(['a', 'b'], centre_of)?;
        let beyond = Point {
            x: b.x.saturating_add(b.x.saturating_sub(a.x).saturating_div(2)),
            y: b.y.saturating_add(b.y.saturating_sub(a.y).saturating_div(2)),
        };
        let clicked = touch_all(&fresh, &[Touch::Down(a), Touch::Moved(beyond)])?;
        let path = path_of(&clicked)?;
        let joined = each(['a', 'b'], number)?;

        assert_eq!(path, joined);

        Ok(())
    }

    #[test]
    fn a_finger_drawn_back_over_joined_dots_joins_nothing() -> Result<(), Failure> {
        let Ok(fresh) = Pattern::new();
        let [a, b, c] = each(['a', 'b', 'c'], centre_of)?;
        let touches = [Touch::Down(a), Touch::Moved(b), Touch::Moved(c), Touch::Moved(a)];
        let clicked = touch_all(&fresh, &touches)?;
        let path = path_of(&clicked)?;
        let joined = each(['a', 'b', 'c'], number)?;

        assert_eq!(path, joined);

        Ok(())
    }

    #[test]
    fn a_touch_that_meets_no_dot_submits_nothing() -> Result<(), Failure> {
        let Ok(fresh) = Pattern::new();
        let clicked = touch_all(&fresh, &[Touch::Down(Point { x: 220, y: 170 }), Touch::Up])?;

        match clicked {
            Clicked::Traced(pattern) => {
                assert!(pattern.path.is_empty());
                assert_eq!(pattern.finger, None);
            }
            Clicked::Submitted(_) => return Err(Box::from("an empty touch submitted")),
        }

        Ok(())
    }

    #[test]
    fn a_new_touch_starts_a_new_pattern() -> Result<(), Failure> {
        let b = at('b')?;
        let joined = each(['a', 'b'], number)?;
        let drawn = Pattern { at: b, path: joined.to_vec(), finger: None };
        let e = centre_of('e')?;
        let clicked = touch_all(&drawn, &[Touch::Down(e)])?;
        let path = path_of(&clicked)?;
        let e = number('e')?;

        assert_eq!(path, vec![e]);

        Ok(())
    }

    #[test]
    fn touching_login_submits_what_the_pad_drew() -> Result<(), Failure> {
        let b = at('b')?;
        let joined = each(['a', 'b'], number)?;
        let drawn = Pattern { at: b, path: joined.to_vec(), finger: None };
        let clicked = touch_all(&drawn, &[Touch::Down(LOGIN)])?;
        let spelled = spelled_by(&clicked)?;

        assert_eq!(spelled, "ab");

        Ok(())
    }
}
