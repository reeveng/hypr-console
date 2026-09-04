//! A command line, split the way a shell would split it.
//!
//! A .desktop file holds one string and something has to be run from it. The
//! quoting is the specification's own, which is the shell's: a word ends at a
//! space unless the space is quoted.

/// The words of a command, or nothing if the quoting never closes.
pub fn split(said: &str) -> Option<Vec<String>> {
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
                word.push(letters.next()?);
                started = true;
            }
            (Some('"'), '\\') => {
                word.push(letters.next()?);
            }
            (Some(quote), letter) if letter == quote => held = None,
            (_, letter) => {
                word.push(letter);
                started = true;
            }
        }
    }

    if held.is_some() {
        return None;
    }

    if started {
        words.push(word);
    }

    Some(words)
}

/// The field codes a .desktop file puts in a command for the file manager to
/// fill in. Nothing here opens a file with an application, so they are taken
/// out rather than answered.
pub fn without_field_codes(command: &str) -> String {
    let mut said = String::new();
    let mut letters = command.chars().peekable();

    while let Some(letter) = letters.next() {
        if letter == '%'
            && letters.peek().is_some_and(|next| "cdDfFikmnNuUvm".contains(*next))
        {
            letters.next();
            continue;
        }

        said.push(letter);
    }

    said.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_its_words() {
        assert_eq!(split("firefox --new-window"), Some(vec!["firefox".into(), "--new-window".into()]));
    }

    /// An application in a directory somebody named after themselves.
    #[test]
    fn a_quoted_space_is_part_of_the_word() {
        assert_eq!(
            split("\"/home/a name/thing\" --go"),
            Some(vec!["/home/a name/thing".into(), "--go".into()])
        );
        assert_eq!(split("env A='a b' run"), Some(vec!["env".into(), "A=a b".into(), "run".into()]));
    }

    #[test]
    fn quoting_that_never_closes_is_not_a_command() {
        assert_eq!(split("firefox \"--new"), None);
    }

    #[test]
    fn an_empty_command_is_no_words_rather_than_one_empty_one() {
        assert_eq!(split("   "), Some(Vec::new()));
    }

    #[test]
    fn the_field_codes_are_taken_out() {
        assert_eq!(without_field_codes("firefox %u"), "firefox");
        assert_eq!(without_field_codes("gimp %U --no-splash"), "gimp  --no-splash");
    }

    /// A per cent that is not a field code is a per cent.
    #[test]
    fn something_that_is_not_a_field_code_is_left_where_it_is() {
        assert_eq!(without_field_codes("thing --at 50%"), "thing --at 50%");
    }
}
