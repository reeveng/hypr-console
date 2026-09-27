//! `rename-words pick`: the open lines of `renames.plan`, one at a time, with
//! the code around each definition on screen and a key per option.
//!
//! The plan stays the file it was. Every key writes the line it answered back
//! into it at once, so quitting halfway loses nothing and a choice made here is
//! the same choice as one typed into the file by hand. The lines come sorted by
//! word, because the question worth answering once is what `said` means in
//! this tree, and a word's sites are easier to tell apart side by side.
//!
//! The terminal is driven with `stty` and ANSI escapes rather than a library:
//! a screen that redraws on every key is all this needs.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::plan::{self, KEEP, PICK};

#[derive(Clone, Debug, PartialEq)]
pub struct Open {
    pub at: usize,
    pub file: String,
    pub key: String,
    pub word: String,
    pub options: Vec<String>,
    pub line: Option<usize>,
}

pub fn open_lines(text: &str) -> Vec<Open> {
    let mut file = String::new();
    let mut found = Vec::new();

    for (at, raw) in text.lines().enumerate() {
        if let Some(section) = raw.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            file = section.to_string();
            continue;
        }
        if raw.starts_with('#') {
            continue;
        }
        let (body, note) = match raw.split_once('#') {
            Some((body, note)) => (body, note),
            None => (raw, ""),
        };
        let Some((key, right)) = body.rsplit_once('=') else { continue };
        let right = right.trim();

        if !right.starts_with(PICK) {
            continue;
        }
        let key = key.trim().to_string();
        let word = key.split_whitespace().next().unwrap_or("").to_string();
        let options = right
            .trim_start_matches(PICK)
            .split('|')
            .map(str::trim)
            .filter(|option| !option.is_empty())
            .map(str::to_string)
            .collect();
        let line = note.trim().split(':').next().and_then(|number| number.trim().parse::<usize>().ok());

        found.push(Open { at, file: file.clone(), key, word, options, line });
    }
    found.sort_by(|a, b| (&a.word, &a.file, a.at).cmp(&(&b.word, &b.file, b.at)));

    found
}

pub fn answered(text: &str, at: usize, right: &str) -> String {
    let lines: Vec<String> = text
        .lines()
        .enumerate()
        .map(|(here, raw)| match here == at {
            true => {
                let (body, note) = match raw.split_once('#') {
                    Some((body, note)) => (body, format!("    #{note}")),
                    None => (raw, String::new()),
                };
                let key = body.rsplit_once('=').map(|(key, _)| key.trim_end()).unwrap_or(body.trim_end());

                format!("{key} = {right}{note}")
            }
            false => raw.to_string(),
        })
        .collect();

    lines.join("\n") + "\n"
}

enum Key {
    Option(usize),
    All,
    Type,
    Keep,
    Next,
    Back,
    Quit,
    Other,
}

struct Raw {
    saved: String,
}

impl Raw {
    fn enter() -> std::io::Result<Raw> {
        let saved = stty(&["-g"])?;

        stty(&["raw", "-echo"])?;
        print!("\x1b[?25l");

        Ok(Raw { saved: saved.trim().to_string() })
    }

    fn cooked<T>(&self, work: impl FnOnce() -> T) -> std::io::Result<T> {
        stty(&[&self.saved])?;
        print!("\x1b[?25h");
        std::io::stdout().flush()?;
        let done = work();

        stty(&["raw", "-echo"])?;
        print!("\x1b[?25l");

        Ok(done)
    }
}

impl Drop for Raw {
    fn drop(&mut self) {
        let _ = stty(&[&self.saved]);
        print!("\x1b[?25h\x1b[2J\x1b[H");
        let _ = std::io::stdout().flush();
    }
}

fn stty(arguments: &[&str]) -> std::io::Result<String> {
    let out = Command::new("stty").args(arguments).stdin(Stdio::inherit()).output()?;

    match out.status.success() {
        true => Ok(String::from_utf8_lossy(&out.stdout).to_string()),
        false => Err(std::io::Error::other(format!("stty {}: {}", arguments.join(" "), String::from_utf8_lossy(&out.stderr).trim()))),
    }
}

fn key() -> std::io::Result<Key> {
    let mut byte = [0u8; 1];
    let mut stdin = std::io::stdin();

    stdin.read_exact(&mut byte)?;
    Ok(match byte[0] {
        b'1'..=b'9' => Key::Option(usize::from(byte[0] - b'1')),
        b'a' | b'A' => Key::All,
        b'n' | b't' | b'\r' => Key::Type,
        b'k' | b'-' => Key::Keep,
        b's' | b' ' | b'j' => Key::Next,
        b'b' | b'p' => Key::Back,
        b'q' | 3 => Key::Quit,
        0x1b => {
            let mut rest = [0u8; 2];

            match stdin.read_exact(&mut rest) {
                Ok(()) => match rest {
                    [b'[', b'C'] | [b'[', b'B'] => Key::Next,
                    [b'[', b'D'] | [b'[', b'A'] => Key::Back,
                    _ => Key::Other,
                },
                Err(_) => Key::Quit,
            }
        }
        _ => Key::Other,
    })
}

fn draw(root: &Path, open: &Open, here: usize, all: &[Open], chosen: usize) -> std::io::Result<()> {
    let mut screen = String::from("\x1b[2J\x1b[H");
    let same = all.iter().filter(|other| other.word == open.word).count();
    let nth = all.iter().take(here + 1).filter(|other| other.word == open.word).count();

    screen.push_str(&format!(
        "\x1b[1m{}\x1b[0m  {nth} of {same} for this word   {} of {} open   {chosen} answered this run\r\n",
        open.word,
        here + 1,
        all.len()
    ));
    screen.push_str(&format!("\x1b[2m{}:{}\x1b[0m   {}\r\n\r\n", open.file, open.line.unwrap_or(0), open.key));

    let source = std::fs::read_to_string(root.join(&open.file)).unwrap_or_default();
    let lines: Vec<&str> = source.lines().collect();
    let target = open.line.unwrap_or(1).saturating_sub(1);
    let from = target.saturating_sub(4);
    let to = (target + 9).min(lines.len());

    for (number, text) in lines.iter().enumerate().take(to).skip(from) {
        let shown: String = text.chars().take(150).collect();
        let marked = shown.replace(&open.word, &format!("\x1b[7m{}\x1b[27m", open.word));

        match number == target {
            true => screen.push_str(&format!("\x1b[33m{:>5}\x1b[0m \x1b[1m{marked}\x1b[0m\r\n", number + 1)),
            false => screen.push_str(&format!("\x1b[2m{:>5}\x1b[0m {marked}\r\n", number + 1)),
        }
    }
    screen.push_str("\r\n");
    for (number, option) in open.options.iter().enumerate().take(9) {
        screen.push_str(&format!("  \x1b[1m{}\x1b[0m  {option}\r\n", number + 1));
    }
    screen.push_str(
        "\r\n\x1b[2m1-9 choose   a then 1-9 choose for every open site of this word   n type a name   k keep   → skip   ← back   q quit\x1b[0m\r\n",
    );
    print!("{screen}");
    std::io::stdout().flush()
}

fn write(root: &Path, text: &str) -> std::io::Result<()> {
    let path = root.join(plan::FILE);
    let staged = root.join(format!("{}.picking", plan::FILE));

    std::fs::write(&staged, text)?;
    std::fs::rename(staged, path)
}

pub fn pick(root: &Path) -> std::io::Result<std::process::ExitCode> {
    let mut text = std::fs::read_to_string(root.join(plan::FILE))?;
    let mut all = open_lines(&text);
    let mut here = 0;
    let mut chosen = 0;

    match all.is_empty() {
        true => {
            println!("{}: nothing is open", plan::FILE);
            return Ok(std::process::ExitCode::SUCCESS);
        }
        false => {}
    }
    let raw = Raw::enter()?;

    loop {
        let Some(open) = all.get(here).cloned() else { break };

        draw(root, &open, here, &all, chosen)?;
        let answer: Option<(Vec<usize>, String)> = match key()? {
            Key::Option(n) => open.options.get(n).map(|name| (vec![open.at], name.clone())),
            Key::All => match key()? {
                Key::Option(n) => open.options.get(n).map(|name| {
                    (all.iter().filter(|other| other.word == open.word).map(|other| other.at).collect(), name.clone())
                }),
                _ => None,
            },
            Key::Type => {
                let typed = raw.cooked(|| {
                    print!("\r\nname for {}: ", open.word);
                    let _ = std::io::stdout().flush();
                    let mut line = String::new();
                    let _ = std::io::stdin().read_line(&mut line);
                    line.trim().to_string()
                })?;

                match crate::vocabulary::is_identifier(&typed) {
                    true => Some((vec![open.at], typed)),
                    false => None,
                }
            }
            Key::Keep => Some((vec![open.at], KEEP.to_string())),
            Key::Next => {
                here = (here + 1).min(all.len().saturating_sub(1));
                None
            }
            Key::Back => {
                here = here.saturating_sub(1);
                None
            }
            Key::Quit => break,
            Key::Other => None,
        };

        if let Some((lines, right)) = answer {
            for at in &lines {
                text = answered(&text, *at, &right);
            }
            write(root, &text)?;
            chosen += lines.len();
            all = open_lines(&text);
            here = here.min(all.len().saturating_sub(1));
            match all.is_empty() {
                true => break,
                false => {}
            }
        }
    }
    drop(raw);
    println!("{chosen} answered, {} still open in {}", all.len(), plan::FILE);

    Ok(std::process::ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::{answered, open_lines};

    const PLAN: &str = "# head\n[*]\n\n[crates/a/src/lib.rs]\nwide field of Size = width    # applied\nsaid fn = ? contents | output    # 12: pub fn said() {}\n\n[crates/b/src/lib.rs]\nheld field of Hand = ? state | guard    # 3: held: u8,\nsaid fn = -    # 9: fn said() {}\n";

    #[test]
    fn only_the_open_lines_come_back_sorted_by_word_with_their_options_and_line() {
        let open = open_lines(PLAN);

        assert_eq!(open.iter().map(|one| one.key.as_str()).collect::<Vec<_>>(), ["held field of Hand", "said fn"]);
        assert_eq!(open[1].options, ["contents", "output"]);
        assert_eq!(open[1].file, "crates/a/src/lib.rs");
        assert_eq!(open[1].line, Some(12));
    }

    #[test]
    fn an_answer_rewrites_the_right_side_of_its_line_and_nothing_else() {
        let at = open_lines(PLAN)[1].at;
        let text = answered(PLAN, at, "contents");

        assert!(text.contains("said fn = contents    # 12: pub fn said() {}\n"));
        assert!(text.contains("held field of Hand = ? state | guard"));
        assert!(text.contains("said fn = -    # 9: fn said() {}"));
        assert_eq!(open_lines(&text).len(), 1);
    }
}
