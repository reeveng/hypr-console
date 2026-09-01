//! The guide, printed.
//!
//! Named by number, not by shade: 35 and 37 are whatever the terminal's palette
//! says magenta and white are, which on this machine is the pink and the quiet
//! colour every other surface uses. The dim attribute is not used anywhere
//! here. It halves whatever it is applied to, and half of a colour chosen to
//! clear 7:1 is a colour that does not.

use crate::guide::Section;

/// How wide the line under a heading is drawn.
pub const RULE: usize = 46;

/// How far the second column starts.
pub const COLUMN: usize = 22;

/// The escapes, or nothing at all where nothing is reading them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ink {
    pub bold: &'static str,
    pub quiet: &'static str,
    pub pink: &'static str,
    pub off: &'static str,
}

pub const COLOURED: Ink =
    Ink { bold: "\u{1b}[1m", quiet: "\u{1b}[37m", pink: "\u{1b}[35m", off: "\u{1b}[0m" };

pub const PLAIN: Ink = Ink { bold: "", quiet: "", pink: "", off: "" };

/// The whole guide, as something to read in a terminal.
pub fn guide(sections: &[Section], ink: Ink) -> String {
    let mut said = format!("\n{}The buttons on this device{}\n", ink.bold, ink.off);
    for section in sections.iter().filter(|section| !section.lines.is_empty()) {
        said.push_str(&format!(
            "\n{}{}{}{}\n{}{}{}\n",
            ink.pink,
            ink.bold,
            section.title,
            ink.off,
            ink.quiet,
            "\u{2500}".repeat(RULE),
            ink.off
        ));
        for line in &section.lines {
            said.push_str(&format!(
                "  {}{:<COLUMN$}{}{}\n",
                ink.bold, line.button, ink.off, line.does
            ));
        }
    }
    said.push_str(&format!(
        "\n{}  Not sure which paddle is which? Run:  console-buttons --identify{}\n\n",
        ink.quiet, ink.off
    ));
    said
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_controller::means::Table;

    use crate::guide::{Line, sections};

    #[test]
    fn a_terminal_that_is_not_one_is_given_no_escapes() {
        let said = guide(&sections(&Table::ours(), ""), PLAIN);
        assert!(!said.contains('\u{1b}'), "an escape reached something reading a file");
    }

    #[test]
    fn every_line_is_the_button_and_what_it_does() {
        let said = guide(&sections(&Table::ours(), ""), PLAIN);
        assert!(said.contains("  Touchpad              move the pointer"));
    }

    /// A guide with a heading and nothing under it is a section somebody will
    /// look for the rest of.
    #[test]
    fn a_section_with_nothing_in_it_is_not_printed() {
        let said = guide(&sections(&Table::ours(), ""), PLAIN);
        assert!(!said.contains("Shortcuts"));
    }

    #[test]
    fn a_section_with_something_in_it_is() {
        let mut every = sections(&Table::ours(), "");
        every.last_mut().expect("a section").lines.push(Line::new("Super Q", "close"));
        assert!(guide(&every, PLAIN).contains("Shortcuts"));
    }
}
