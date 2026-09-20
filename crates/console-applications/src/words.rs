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

const RESERVED: &str = " \t\"'\\<>~|&;$*?#()`%";

pub fn joined(argv: &[String]) -> Result<String, Never> {
    let mut said = String::new();

    for word in argv {
        match said.is_empty() {
            true => {},
            false => said.push(' '),
        }

        let word = quoted(word)?;

        said.push_str(&word);
    }

    Ok(said)
}

fn quoted(word: &str) -> Result<String, Never> {
    let plain = !word.is_empty() && !word.chars().any(|letter| RESERVED.contains(letter));

    match plain {
        true => return Ok(word.to_string()),
        false => {},
    }

    let mut said = String::from("\"");

    for letter in word.chars() {
        match letter {
            '"' | '\\' | '$' | '`' => said.push('\\'),
            '%' => said.push('%'),
            _ => {},
        }

        said.push(letter);
    }

    said.push('"');

    Ok(said)
}

pub fn without_field_codes(command: &str) -> Result<String, Never> {
    let mut said = String::new();
    let mut letters = command.chars().peekable();

    while let Some(letter) = letters.next() {
        match letter == '%' && letters.peek() == Some(&'%') {
            true => {
                letters.next();
                said.push('%');

                continue;
            }
            false => {},
        }

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

    #[test]
    fn a_doubled_percent_is_the_one_a_command_meant() {
        assert_eq!(ok(without_field_codes("open https://x/a%%20b")), "open https://x/a%20b");
        assert_eq!(ok(without_field_codes("open %%D0%%BF")), "open %D0%BF");
    }

    #[test]
    fn a_word_with_nothing_in_it_to_hide_is_written_as_it_is() {
        assert_eq!(ok(joined(&["xdg-open".into(), "https://example.com/a".into()])), "xdg-open https://example.com/a");
    }

    #[test]
    fn what_is_joined_is_what_is_split_back_out() {
        let argv = vec![
            "xdg-open".to_string(),
            "https://example.com/a b?q=1&r=2#top".to_string(),
            "a \"quoted\" $thing".to_string(),
        ];
        let Ok(said) = joined(&argv);
        let Ok(read) = without_field_codes(&said);

        assert_eq!(ok(split(&read)), Some(argv));
    }

    #[test]
    fn a_percent_in_an_address_survives_the_reading() {
        let argv = vec!["xdg-open".to_string(), "https://example.com/%D0%BF".to_string()];
        let Ok(said) = joined(&argv);

        assert!(said.contains("%%D0%%BF"), "{said}");
        let Ok(read) = without_field_codes(&said);

        assert_eq!(ok(split(&read)), Some(argv));
    }
}
