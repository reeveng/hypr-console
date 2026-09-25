//! What kind of thing a file is, by the name it has.
//!
//! Three crates asked GTK this -- the files panel for every row it draws, the
//! thumbnails behind it, and the viewer deciding whether a thing is a
//! photograph or a film -- and gio's answer was never gio's. It is
//! shared-mime-info's table, which is on the machine as `globs2`: one line per
//! pattern, a weight, the type it means, and the pattern itself. So the toolkit
//! was a way of reading a file this desktop can read.
//!
//! **The name and not the contents.** gio calls this a fast content type and
//! the word is the argument for it: sniffing the first bytes of two hundred
//! files to draw one folder is work a listing cannot afford, and the name is
//! right for everything a person keeps. What it is wrong for -- a file with no
//! ending, a file named wrongly -- is a row that says the wrong thing rather
//! than a panel that stalls, and the viewer refuses it at the point where it
//! would have to decode it anyway.
//!
//! **Which pattern wins is the spec's order, not the file's.** A literal name
//! beats a pattern (`makefile` is not `*.file`), the longest ending beats a
//! shorter one (`.tar.gz` before `.gz`), and weight breaks what is left. A
//! table read in the order it happens to be written answers `image/jpeg` for
//! one machine's JPEG and `image/jp2` for another's, which is the kind of
//! disagreement nobody finds by reading the code.
//!
//! **A pattern with a `cs` flag is the case-sensitive one**, which is how `*.C`
//! stays C++ while `*.c` is C. Everything else is matched with the name folded
//! down, because a photograph called `BEACH.JPG` is a photograph.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use console_core_atomic_writes::Stored;
use console_core_never::Never;

pub const GLOBS: &str = "/usr/share/mime/globs2";

pub const NOTHING_SAYS_WHAT_IT_IS: &str = "";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Letters {
    AsWritten,
    Folded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Pattern {
    weight: u32,
    kind: String,
    letters: Letters,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    named: BTreeMap<String, Pattern>,
    ending: BTreeMap<String, Pattern>,
    other: Vec<(Glob, Pattern)>,
}

#[derive(Debug)]
pub enum TableError {
    Read(PathBuf, String),
}

impl std::fmt::Display for TableError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TableError::Read(at, why) => write!(to, "{}: {why}", at.display()),
        }
    }
}

impl std::error::Error for TableError {}

impl Table {
    pub fn read(at: &Path) -> Result<Table, TableError> {
        let held = console_core_atomic_writes::read(at);

        let said = match held {
            Ok(Stored::Text(said)) => said,
            Ok(Stored::Absent) => String::new(),
            Ok(Stored::Failed(why)) => {
                return Err(TableError::Read(at.to_path_buf(), why.to_string()));
            },
            Err(never) => match never {},
        };

        parsed(&said)
    }

    pub fn here() -> Result<Table, TableError> {
        Table::read(Path::new(GLOBS))
    }

    pub fn of(&self, name: &str) -> Result<Option<&str>, Never> {
        let folded = name.to_lowercase();
        let spelled = Name { written: name, folded: &folded };

        let named = self
            .named
            .get(name)
            .filter(|found| found.letters == Letters::AsWritten)
            .or_else(|| self.named.get(&folded).filter(|found| found.letters == Letters::Folded));

        match named {
            Some(found) => return Ok(Some(&found.kind)),
            None => {},
        }

        let Ok(ending) = self.by_ending(spelled);

        match ending {
            Some(found) => return Ok(Some(found)),
            None => {},
        }

        self.by_pattern(spelled)
    }

    fn by_ending(&self, name: Name<'_>) -> Result<Option<&str>, Never> {
        let mut best: Option<(&str, &Pattern)> = None;

        for (ending, found) in &self.ending {
            let Ok(said) = name.as_matched(found.letters);
            let matched = said.len() > ending.len() && said.ends_with(ending.as_str());

            match matched {
                true => {
                    best = match best {
                        Some(had) => match (ending.len(), found.weight) > (had.0.len(), had.1.weight) {
                            true => Some((ending, found)),
                            false => Some(had),
                        },
                        None => Some((ending, found)),
                    };
                },
                false => {},
            }
        }

        Ok(best.map(|(_, found)| found.kind.as_str()))
    }

    fn by_pattern(&self, name: Name<'_>) -> Result<Option<&str>, Never> {
        let mut best: Option<(u32, &str)> = None;

        for (glob, found) in &self.other {
            let Ok(said) = name.as_matched(found.letters);
            let Ok(matched) = glob.matches(said);

            match matched {
                Matched::Yes => {
                    let mine = (found.weight, found.kind.as_str());

                    best = match best {
                        Some(had) => match mine.0 > had.0 {
                            true => Some(mine),
                            false => Some(had),
                        },
                        None => Some(mine),
                    };
                },
                Matched::No => {},
            }
        }

        Ok(best.map(|(_, kind)| kind))
    }
}

#[derive(Debug, Clone, Copy)]
struct Name<'a> {
    written: &'a str,
    folded: &'a str,
}

impl<'a> Name<'a> {
    fn as_matched(&self, letters: Letters) -> Result<&'a str, Never> {
        Ok(match letters {
            Letters::AsWritten => self.written,
            Letters::Folded => self.folded,
        })
    }
}

pub fn of(table: &Table, path: &Path) -> Result<String, Never> {
    let name = match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => name,
        None => return Ok(NOTHING_SAYS_WHAT_IT_IS.to_string()),
    };

    let Ok(found) = table.of(name);

    Ok(match found {
        Some(kind) => kind.to_string(),
        None => NOTHING_SAYS_WHAT_IT_IS.to_string(),
    })
}

const AN_ENDING: &str = "*.";

fn parsed(said: &str) -> Result<Table, TableError> {
    let mut table = Table::default();

    for line in said.lines() {
        let line = line.trim_end();

        match line.starts_with('#') || line.is_empty() {
            true => continue,
            false => {},
        }

        let mut parts = line.split(':');

        let weight = match parts.next().map(str::parse::<u32>) {
            Some(Ok(weight)) => weight,
            Some(Err(_a_weight_that_is_not_a_number)) => continue,
            None => continue,
        };

        let kind = parts.next();
        let glob = parts.next();
        let letters = match parts.next() {
            Some("cs") => Letters::AsWritten,
            Some(_another_flag) => Letters::Folded,
            None => Letters::Folded,
        };

        let (kind, glob) = match (kind, glob) {
            (Some(kind), Some(glob)) => (kind, glob),
            (None, _) | (_, None) => continue,
        };

        let found = Pattern { weight, kind: kind.to_string(), letters };
        let wild = glob.contains(['*', '?', '[']);
        let ending = glob.strip_prefix(AN_ENDING).filter(|rest| !rest.contains(['*', '?', '[']));

        match (wild, ending) {
            (false, _) => {
                let Ok(()) = kept(&mut table.named, glob.to_string(), found);
            },
            (true, Some(ending)) => {
                let Ok(()) = kept(&mut table.ending, format!(".{ending}"), found);
            },
            (true, None) => {
                let Ok(read) = Glob::read(glob);

                table.other.push((read, found));
            },
        }
    }

    Ok(table)
}

fn kept(into: &mut BTreeMap<String, Pattern>, key: String, found: Pattern) -> Result<(), Never> {
    let had = into.get(&key).map(|had| had.weight);

    match had {
        Some(had) => match found.weight > had {
            true => {
                into.insert(key, found);
            },
            false => {},
        },
        None => {
            into.insert(key, found);
        },
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Matched {
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Any,
    One,
    Letter(char),
    Class(Vec<char>),
    Unclosed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Glob {
    tokens: Vec<Token>,
}

impl Glob {
    fn read(written: &str) -> Result<Glob, Never> {
        let mut letters = written.chars();
        let mut tokens = Vec::new();

        loop {
            let token = match letters.next() {
                Some('*') => Token::Any,
                Some('?') => Token::One,
                Some('[') => match letters.as_str().split_once(']') {
                    Some((held, rest)) => {
                        let class = Token::Class(held.chars().collect());
                        letters = rest.chars();

                        class
                    },
                    None => Token::Unclosed,
                },
                Some(letter) => Token::Letter(letter),
                None => break,
            };

            tokens.push(token);
        }

        Ok(Glob { tokens })
    }

    fn matches(&self, said: &str) -> Result<Matched, Never> {
        let said: Vec<char> = said.chars().collect();
        let mut at: (&[Token], &[char]) = (&self.tokens, &said);
        let mut last_any = None;

        while let Some((letter, letters_after)) = at.1.split_first() {
            let (token, tokens_after) = match at.0.split_first() {
                Some((token, tokens_after)) => (Some(token), tokens_after),
                None => (None, at.0),
            };

            let fits = match token {
                Some(Token::Any) => {
                    at = (tokens_after, at.1);
                    last_any = Some(at);
                    continue;
                },
                Some(Token::One) => Matched::Yes,
                Some(Token::Letter(one)) => match one == letter {
                    true => Matched::Yes,
                    false => Matched::No,
                },
                Some(Token::Class(held)) => {
                    let Ok(inside) = one_of(held, *letter);

                    inside
                },
                Some(Token::Unclosed) | None => Matched::No,
            };

            match (fits, last_any) {
                (Matched::Yes, _) => at = (tokens_after, letters_after),
                (Matched::No, Some((after, from))) => {
                    let from_next = match from.split_first() {
                        Some((_, from_next)) => from_next,
                        None => from,
                    };

                    at = (after, from_next);
                    last_any = Some(at);
                },
                (Matched::No, None) => return Ok(Matched::No),
            }
        }

        Ok(match at.0.iter().all(|token| *token == Token::Any) {
            true => Matched::Yes,
            false => Matched::No,
        })
    }
}

fn one_of(held: &[char], letter: char) -> Result<Matched, Never> {
    let mut rest = held;

    loop {
        match rest {
            [] => return Ok(Matched::No),
            [first, '-', last, after @ ..] => match *first <= letter && letter <= *last {
                true => return Ok(Matched::Yes),
                false => rest = after,
            },
            [one, after @ ..] => match *one == letter {
                true => return Ok(Matched::Yes),
                false => rest = after,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAID: &str = "# a head\n\
        50:image/jpeg:*.jpg\n\
        50:image/jpeg:*.jpeg\n\
        50:application/gzip:*.gz\n\
        50:application/x-compressed-tar:*.tar.gz\n\
        50:text/x-csrc:*.c:cs\n\
        50:text/x-c++src:*.C:cs\n\
        50:text/x-makefile:makefile\n\
        60:application/x-sharedlib:*.so.[0-9]*\n\
        50:text/x-scons:sconscript.*\n";

    fn table() -> Table {
        parsed(SAID).expect("the table this test wrote")
    }

    #[test]
    fn an_ending_says_what_a_photograph_is() {
        let table = table();

        assert_eq!(table.of("beach.jpg"), Ok(Some("image/jpeg")));
        assert_eq!(table.of("beach.jpeg"), Ok(Some("image/jpeg")));
    }

    #[test]
    fn the_longest_ending_wins_so_an_archive_is_not_its_wrapper() {
        let table = table();

        assert_eq!(table.of("console.tar.gz"), Ok(Some("application/x-compressed-tar")));
        assert_eq!(table.of("console.gz"), Ok(Some("application/gzip")));
    }

    #[test]
    fn a_name_that_is_the_whole_pattern_beats_an_ending() {
        let table = table();

        assert_eq!(table.of("makefile"), Ok(Some("text/x-makefile")));
        assert_eq!(table.of("Makefile"), Ok(Some("text/x-makefile")));
    }

    #[test]
    fn a_case_sensitive_pattern_is_the_one_that_tells_two_languages_apart() {
        let table = table();

        assert_eq!(table.of("main.c"), Ok(Some("text/x-csrc")));
        assert_eq!(table.of("main.C"), Ok(Some("text/x-c++src")));
    }

    #[test]
    fn a_photograph_shouted_is_still_a_photograph() {
        let table = table();

        assert_eq!(table.of("BEACH.JPG"), Ok(Some("image/jpeg")));
    }

    #[test]
    fn a_pattern_that_is_neither_a_name_nor_an_ending_is_still_matched() {
        let table = table();

        assert_eq!(table.of("libc.so.6"), Ok(Some("application/x-sharedlib")));
        assert_eq!(table.of("sconscript.build"), Ok(Some("text/x-scons")));
    }

    #[test]
    fn a_glob_goes_back_to_its_last_star_rather_than_giving_up() {
        let matched = |written: &str, said: &str| {
            let Ok(glob) = Glob::read(written);
            let Ok(matched) = glob.matches(said);

            matched
        };

        assert_eq!(matched("*.so.[0-9]*", "libc.so.so.6"), Matched::Yes);
        assert_eq!(matched("a?c*", "abcdef"), Matched::Yes);
        assert_eq!(matched("a?c", "ac"), Matched::No);
        assert_eq!(matched("*[a-c]x", "zzbx"), Matched::Yes);
        assert_eq!(matched("*[a-c]x", "zzdx"), Matched::No);
        assert_eq!(matched("*[ab", "za"), Matched::No);
        assert_eq!(matched("**", ""), Matched::Yes);
    }

    #[test]
    fn a_name_nothing_in_the_table_knows_is_nothing_rather_than_a_guess() {
        let table = table();

        assert_eq!(table.of("notes"), Ok(None));
        assert_eq!(table.of("beach.jpgx"), Ok(None));
        assert_eq!(of(&table, Path::new("/home/someone/notes")), Ok(String::new()));
    }

    #[test]
    fn the_table_the_machine_carries_is_the_one_a_folder_is_read_with() {
        let here = Table::here();

        let here = match here {
            Ok(here) => here,
            Err(why) => panic!("{why}"),
        };

        assert_eq!(here.of("beach.jpg"), Ok(Some("image/jpeg")));
        assert_eq!(here.of("song.flac"), Ok(Some("audio/flac")));
        let film = here.of("film.mkv");

        assert!(
            matches!(film, Ok(Some(said)) if said.starts_with("video/") && said.ends_with("matroska")),
            "a film is a film whichever of the two names this machine's table keeps: {film:?}"
        );
    }
}
