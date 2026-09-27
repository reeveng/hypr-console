//! Which applications a typed word leaves standing.
//!
//! Plain containment rather than anything cleverer. A fuzzy match is worth
//! having where someone types fast and looks at the results; here the letters
//! arrive one thumb at a time off a keyboard that covers the bottom of the
//! screen, and a list that reorders itself around a letter you did not mean to
//! press is worse than one that simply gets shorter.
//!
//! What it does do is put the names that begin with the word first. Typing
//! "fi" and finding Files under Firefox because Firefox is opened more often
//! is the usage order being right about everything except the thing just
//! asked for.

use console_core_never::Never;

pub fn matching(names: &[String], word: &str) -> Result<Vec<String>, Never> {
    let wanted = word.trim().to_lowercase();

    match wanted.is_empty() {
        true => return Ok(names.to_vec()),
        false => {},
    }

    let (starts, holds): (Vec<String>, Vec<String>) = names
        .iter()
        .filter(|name| name.to_lowercase().contains(&wanted))
        .cloned()
        .partition(|name| name.to_lowercase().starts_with(&wanted));

    Ok(starts.into_iter().chain(holds).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(said: &[&str]) -> Result<Vec<String>, Never> {
        Ok(said.iter().map(|word| (*word).to_string()).collect())
    }

    const EVERY: [&str; 4] = ["Firefox", "Files", "Settings", "Profile Manager"];

    #[test]
    fn nothing_typed_leaves_the_list_as_it_was() {
        let Ok(every) = names(&EVERY);

        assert_eq!(matching(&every, ""), Ok(every.clone()));
        assert_eq!(matching(&every, "   "), Ok(every.clone()));
    }

    #[test]
    fn a_name_answers_to_a_word_anywhere_in_it() {
        let Ok(every) = names(&EVERY);

        assert_eq!(matching(&every, "file"), names(&["Files", "Profile Manager"]));
    }

    #[test]
    fn what_begins_with_the_word_comes_before_what_merely_holds_it() {
        let Ok(every) = names(&EVERY);

        assert_eq!(matching(&every, "fi"), names(&["Firefox", "Files", "Profile Manager"]));
    }

    #[test]
    fn the_case_it_was_typed_in_does_not_matter() {
        let Ok(every) = names(&EVERY);

        assert_eq!(matching(&every, "SETT"), names(&["Settings"]));
        assert_eq!(matching(&every, "sEtT"), names(&["Settings"]));
    }

    #[test]
    fn a_word_nothing_answers_to_leaves_nothing() {
        let Ok(every) = names(&EVERY);

        assert_eq!(matching(&every, "kangaroo"), Ok(Vec::new()));
    }
}
