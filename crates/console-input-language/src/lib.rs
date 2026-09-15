//! What a keyboard somebody plugged in is set to type.
//!
//! The keyboard drawn on the screen wears an arrangement and switches it
//! itself, because the keys are its own. A keyboard on a desk wears an xkb
//! layout and cannot: the compositor decides what its keys produce, so the
//! switch has to be asked for. Both walk the same list -- the alphabets this
//! machine is set to type -- and both remember per board, which is
//! `console_input_alphabets::wearing`.
//!
//! What is here is the arithmetic: the layout list a compositor is told, which
//! alphabet lies either way round from the one being worn, and where in the
//! list it sits. Nothing in this file has seen a machine. `language-switch` is
//! the program that asks.
//!
//! Both ways round, because a walk of three is a walk somebody overshoots. The
//! keyboard on the screen has had two shoulders for it since it had a walk at
//! all -- L1 back, R1 on -- and a keyboard on a desk that could only go
//! forward made the way back a matter of how many alphabets this machine is
//! set to type. One step either way is a step, and going round twice to
//! undo one is not.
//!
//! ## Why the list is pushed rather than written
//!
//! `kb_layout` in the compositor's own file is one word, and it was `us`. A
//! list written there would be one more place saying what this machine types,
//! against the setting that already says it -- and it would be wrong for
//! everybody who changed the setting, which is the whole reason the setting
//! exists. So the list is set from the alphabets every time, and the file
//! keeps the one layout that is right before anything of ours has run.

use console_core_never::Never;
use console_input_alphabets::Alphabet;
use console_core_walking::{Ring, Step};

pub const NAMED: &str = "language-switch";

pub const SETTLE: &str = "--settle";

pub const BACK: &str = "--back";

pub fn layouts(walk: &[&'static Alphabet]) -> Result<String, Never> {
    let said: Vec<&str> = walk.iter().map(|alphabet| alphabet.xkb).collect();

    match said.is_empty() {
        true => {
            let Ok(latin) = console_input_alphabets::latin();

            Ok(latin.xkb.to_string())
        }
        false => Ok(said.join(",")),
    }
}

pub fn at(walk: &[&'static Alphabet], alphabet: &Alphabet) -> Result<usize, Never> {
    let found = walk.iter().position(|kept| kept.key == alphabet.key);

    Ok(match found {
        Some(at) => at,
        None => THE_FIRST_ONE,
    })
}

const THE_FIRST_ONE: usize = 0;

pub fn along(
    walk: &[&'static Alphabet],
    alphabet: &Alphabet,
    step: Step,
) -> Result<&'static Alphabet, Never> {
    let Ok(round) = Ring::of(walk);

    let ring = match round {
        Some(ring) => ring,
        None => return console_input_alphabets::latin(),
    };

    let Ok(now) = at(walk, alphabet);
    let Ok(next) = ring.stepped(now, step);

    match walk.get(next) {
        Some(alphabet) => Ok(alphabet),
        None => console_input_alphabets::latin(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    fn walk(said: &str) -> Vec<&'static Alphabet> {
        ok(console_input_alphabets::read(said))
    }

    fn one(key: &str) -> &'static Alphabet {
        let Ok(one) = console_input_alphabets::one(key);

        one.expect("an alphabet")
    }

    #[test]
    fn the_list_a_compositor_is_told_is_the_alphabets_in_the_order_they_are_walked() {
        assert_eq!(ok(layouts(&walk("greek,thai"))), "us,gr,th");
    }

    #[test]
    fn a_machine_that_types_nothing_still_types_english() {
        assert_eq!(ok(layouts(&[])), "us");
    }

    #[test]
    fn the_next_one_comes_after_and_the_last_one_comes_round() {
        let walk = walk("greek,thai");

        assert_eq!(ok(along(&walk, one("latin"), Step::Forward)).key, "greek");
        assert_eq!(ok(along(&walk, one("greek"), Step::Forward)).key, "thai");
        assert_eq!(ok(along(&walk, one("thai"), Step::Forward)).key, "latin");
    }

    #[test]
    fn the_one_before_comes_back_and_the_first_one_comes_round() {
        let walk = walk("greek,thai");

        assert_eq!(ok(along(&walk, one("thai"), Step::Back)).key, "greek");
        assert_eq!(ok(along(&walk, one("greek"), Step::Back)).key, "latin");
        assert_eq!(ok(along(&walk, one("latin"), Step::Back)).key, "thai");
    }

    #[test]
    fn a_step_each_way_is_where_it_started() {
        let walk = walk("greek,thai");

        for alphabet in &walk {
            let Ok(on) = along(&walk, alphabet, Step::Forward);
            let Ok(back) = along(&walk, on, Step::Back);

            assert_eq!(back.key, alphabet.key, "{} does not come back", alphabet.key);
        }
    }

    #[test]
    fn an_alphabet_this_machine_no_longer_types_steps_on_from_the_front() {
        let walk = walk("thai");

        assert_eq!(
            ok(along(&walk, one("greek"), Step::Forward)).key,
            "thai",
            "greek is not in the walk, so where it sits is the front and the next is the second"
        );
    }

    #[test]
    fn where_an_alphabet_sits_is_what_the_compositor_is_handed() {
        let walk = walk("greek,thai");

        assert_eq!(ok(at(&walk, one("latin"))), 0);
        assert_eq!(ok(at(&walk, one("greek"))), 1);
        assert_eq!(ok(at(&walk, one("thai"))), 2);
    }

    #[test]
    fn a_list_with_one_thing_in_it_stays_where_it_is() {
        let walk = walk("");

        assert_eq!(ok(along(&walk, one("latin"), Step::Forward)).key, "latin");
        assert_eq!(ok(along(&walk, one("latin"), Step::Back)).key, "latin");
    }
}
