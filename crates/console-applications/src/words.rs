//! A command line, split the way a shell would split it.
//!
//! A .desktop file holds one string and something has to be run from it. The
//! quoting is the specification's own, which is the shell's: a word ends at a
//! space unless the space is quoted.

use console_core_never::Never;

pub fn split(said: &str) -> Result<Option<Vec<String>>, Never> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut held: Option<char> = None;
    let mut started = false;
    let mut letters = said.chars();

    while let Some(letter) = letters.next() {
        match (held, letter) {
            (None, ' ' | '\t') if started => {
                words.push(std::mem::take(&mut word));
                started = false;
            }
            (None, ' ' | '\t') => (),
            (None, '\'' | '"') => {
                held = Some(letter);
                started = true;
            }
            (None, '\\') => {
                let escaped = match letters.next() {
                    Some(escaped) => escaped,
                    None => return Ok(None),
                };

                word.push(escaped);
                started = true;
            }
            (Some('"'), '\\') => {
                let escaped = match letters.next() {
                    Some(escaped) => escaped,
                    None => return Ok(None),
                };

                word.push(escaped);
            }
            (Some(quote), letter) if letter == quote => held = None,
            (_, letter) => {
                word.push(letter);
                started = true;
            }
        }
    }

    match held.is_some() {
        true => return Ok(None),
        false => {},
    }

    match started {
        true => words.push(word),
        false => {},
    }

    Ok(Some(words))
}

pub fn without_field_codes(command: &str) -> Result<String, Never> {
    let mut said = String::new();
    let mut letters = command.chars().peekable();

    while let Some(letter) = letters.next() {
        match letter == '%'
            && letters.peek().is_some_and(|next| "cdDfFikmnNuUvm".contains(*next))
        {
            true => {
                letters.next();
                continue;
            }
            false => {},
        }

        said.push(letter);
    }

    Ok(said.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    #[test]
    fn a_command_is_its_words() {
        assert_eq!(ok(split("firefox --new-window")), Some(vec!["firefox".into(), "--new-window".into()]));
    }

    #[test]
    fn a_quoted_space_is_part_of_the_word() {
        assert_eq!(
            ok(split("\"/home/a name/thing\" --go")),
            Some(vec!["/home/a name/thing".into(), "--go".into()])
        );
        assert_eq!(
            ok(split("env A='a b' run")),
            Some(vec!["env".into(), "A=a b".into(), "run".into()])
        );
    }

    #[test]
    fn quoting_that_never_closes_is_not_a_command() {
        assert_eq!(ok(split("firefox \"--new")), None);
    }

    #[test]
    fn an_empty_command_is_no_words_rather_than_one_empty_one() {
        assert_eq!(ok(split("   ")), Some(Vec::new()));
    }

    #[test]
    fn the_field_codes_are_taken_out() {
        assert_eq!(ok(without_field_codes("firefox %u")), "firefox");
        assert_eq!(ok(without_field_codes("gimp %U --no-splash")), "gimp  --no-splash");
    }

    #[test]
    fn something_that_is_not_a_field_code_is_left_where_it_is() {
        assert_eq!(ok(without_field_codes("thing --at 50%")), "thing --at 50%");
    }
}
