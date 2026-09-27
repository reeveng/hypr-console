//! A command line, split the way a shell would split it.
//!
//! A .desktop file holds one string and something has to be run from it. The
//! quoting is the specification's own, which is the shell's: a word ends at a
//! space unless the space is quoted.

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Between,
    Word,
    Quoted(char),
    Escaped,
    EscapedQuoted,
}

#[derive(Debug)]
struct Splitting {
    words: Vec<String>,
    word: String,
    place: Place,
}

impl Splitting {
    fn with(mut self, letter: char) -> Result<Self, Never> {
        self.place = match (self.place, letter) {
            (Place::Between, ' ' | '\t') => Place::Between,
            (Place::Word, ' ' | '\t') => {
                self.words.push(std::mem::take(&mut self.word));

                Place::Between
            }
            (Place::Between | Place::Word, '\'' | '"') => Place::Quoted(letter),
            (Place::Between | Place::Word, '\\') => Place::Escaped,
            (Place::Quoted('"'), '\\') => Place::EscapedQuoted,
            (Place::Quoted(quote), letter) => match letter == quote {
                true => Place::Word,
                false => {
                    self.word.push(letter);

                    Place::Quoted(quote)
                }
            },
            (Place::Escaped, letter) => {
                self.word.push(letter);

                Place::Word
            }
            (Place::EscapedQuoted, letter) => {
                self.word.push(letter);

                Place::Quoted('"')
            }
            (Place::Between | Place::Word, letter) => {
                self.word.push(letter);

                Place::Word
            }
        };

        Ok(self)
    }
}

pub fn split(said: &str) -> Result<Option<Vec<String>>, Never> {
    let begun = Splitting { words: Vec::new(), word: String::new(), place: Place::Between };
    let split = said.chars().fold(begun, |split, letter| {
        let Ok(split) = split.with(letter);

        split
    });
    let Splitting { mut words, word, place } = split;

    Ok(match place {
        Place::Between => Some(words),
        Place::Word => {
            words.push(word);

            Some(words)
        }
        Place::Quoted(_) | Place::Escaped | Place::EscapedQuoted => None,
    })
}

const RESERVED: &str = " \t\"'\\<>~|&;$*?#()`%";

pub fn join_quoted(arguments: &[String]) -> Result<String, Never> {
    let mut said = String::new();

    for word in arguments {
        match said.is_empty() {
            true => {},
            false => said.push(' '),
        }

        let word = quote(word)?;

        said.push_str(&word);
    }

    Ok(said)
}

fn quote(word: &str) -> Result<String, Never> {
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

const FIELD_CODES: &str = "cdDfFikmnNuUvm";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    Percent,
    None,
}

pub fn without_field_codes(command: &str) -> Result<String, Never> {
    let (mut said, pending) = command.chars().fold((String::new(), Pending::None), |(mut said, pending), letter| {
        let next = match (pending, letter == '%', FIELD_CODES.contains(letter)) {
            (Pending::Percent, true, _) => {
                said.push('%');

                Pending::None
            }
            (Pending::Percent, false, true) => Pending::None,
            (Pending::Percent, false, false) => {
                said.push('%');
                said.push(letter);

                Pending::None
            }
            (Pending::None, true, _) => Pending::Percent,
            (Pending::None, false, _) => {
                said.push(letter);

                Pending::None
            }
        };

        (said, next)
    });

    match pending {
        Pending::Percent => said.push('%'),
        Pending::None => {},
    }

    Ok(said.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn a_command_is_its_words() {
        assert_eq!(split("firefox --new-window"), Ok(Some(vec!["firefox".to_string(), "--new-window".to_string()])));
    }

    #[test]
    fn a_quoted_space_is_part_of_the_word() {
        assert_eq!(
            split("\"/home/a name/thing\" --go"),
            Ok(Some(vec!["/home/a name/thing".to_string(), "--go".to_string()]))
        );
        assert_eq!(
            split("env A='a b' run"),
            Ok(Some(vec!["env".to_string(), "A=a b".to_string(), "run".to_string()]))
        );
    }

    #[test]
    fn quoting_that_never_closes_is_not_a_command() {
        assert_eq!(split("firefox \"--new"), Ok(None));
    }

    #[test]
    fn an_empty_command_is_no_words_rather_than_one_empty_one() {
        assert_eq!(split("   "), Ok(Some(Vec::new())));
    }

    #[test]
    fn the_field_codes_are_taken_out() {
        assert_eq!(without_field_codes("firefox %u"), Ok("firefox".to_string()));
        assert_eq!(without_field_codes("gimp %U --no-splash"), Ok("gimp  --no-splash".to_string()));
    }

    #[test]
    fn something_that_is_not_a_field_code_is_left_where_it_is() {
        assert_eq!(without_field_codes("thing --at 50%"), Ok("thing --at 50%".to_string()));
    }

    #[test]
    fn a_doubled_percent_is_the_one_a_command_meant() {
        assert_eq!(without_field_codes("open https://x/a%%20b"), Ok("open https://x/a%20b".to_string()));
        assert_eq!(without_field_codes("open %%D0%%BF"), Ok("open %D0%BF".to_string()));
    }

    #[test]
    fn a_word_with_nothing_in_it_to_hide_is_written_as_it_is() {
        assert_eq!(
            join_quoted(&["xdg-open".to_string(), "https://example.com/a".to_string()]),
            Ok("xdg-open https://example.com/a".to_string())
        );
    }

    #[test]
    fn what_is_joined_is_what_is_split_back_out() -> Result<(), Box<dyn Error>> {
        let arguments = vec![
            "xdg-open".to_string(),
            "https://example.com/a b?q=1&r=2#top".to_string(),
            "a \"quoted\" $thing".to_string(),
        ];
        let Ok(said) = join_quoted(&arguments);
        let Ok(read) = without_field_codes(&said);

        assert_eq!(split(&read), Ok(Some(arguments)));

        Ok(())
    }

    #[test]
    fn a_percent_in_an_address_survives_the_reading() {
        let arguments = vec!["xdg-open".to_string(), "https://example.com/%D0%BF".to_string()];
        let Ok(said) = join_quoted(&arguments);

        assert!(said.contains("%%D0%%BF"), "{said}");

        let Ok(read) = without_field_codes(&said);

        assert_eq!(split(&read), Ok(Some(arguments)));
    }
}
