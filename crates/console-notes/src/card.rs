//! The notes, drawn.
//!
//! One page, opening on the list: New Note at the top and every note under
//! it, newest first. New Note asks for a title and then for the note, both as
//! the panel's own question, so the keyboard comes up and Return keeps what
//! was typed; the file is written once both are answered, and B on either
//! question leaves nothing behind. A note opened is its body under the way
//! back, which carries its title. Y on a note shows it in the files or selects
//! it to be deleted, as it does on a song or a picture.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use console_core_internal_programs::InternalProgram;
use console_core_arguments::{Command, Operands};
use console_core_never::Never;
use console_panel::card::{Card, Door};
use console_panel::page::{Answer, Aside, Handler, Page, Row, Rows, Showing, shown_or_selected};

use crate::folder::{self, Draft, Exists, Note};

pub const WHO: &str = "notes";

const DOOR: &str = "notes";

const TITLE: &str = "Notes";

const NEW_NOTE: &str = "New Note";

const ASKING_TITLE: &str = "Title";

const ASKING_BODY: &str = "Note";

const NO_HOME: &str = "There is no home folder to keep notes in";

const FIRST_ROW_UNDER_THE_WAY_BACK: u32 = 1;

const NEW_NOTE_ROW: u32 = 0;

type Open = Arc<Mutex<Option<PathBuf>>>;

pub const COMMAND: Command = Command {
    name: "notes",
    about: "notes, each a title and a body",
    flags: &[],
    operands: Operands::None,
};

pub fn door(_argv: &[String]) -> Result<Door, Never> {
    Door::closing(DOOR)
}

pub fn card(_argv: &[String]) -> Result<Card, Never> {
    let Ok(home) = console_core_places::home();
    let kept = home.map(|home| home.join(folder::FOLDER));
    let open: Open = Arc::new(Mutex::new(None));

    Card::new(Arc::new(move || {
        let Ok(page) = page(&open, kept.as_deref());

        vec![page]
    }))
}

fn page(open: &Open, kept: Option<&Path>) -> Result<Page, Never> {
    let drawing = Arc::clone(open);
    let backing = Arc::clone(open);
    let kept = kept.map(Path::to_path_buf);
    let Ok(rows) = Rows::computed(move || {
        let Ok(rows) = rows(&drawing, kept.as_deref());

        rows
    });
    let Ok(page) = Page::new(TITLE, rows);
    let Ok(page) = page.trashing_selected();

    page.on_back(move |showing| {
        let Ok(was) = opened(&backing, None);

        match was {
            Some(_note) => {
                showing.replace(NEW_NOTE_ROW);

                false
            }
            None => true,
        }
    })
}

fn opened(open: &Open, now: Option<PathBuf>) -> Result<Option<PathBuf>, Never> {
    Ok(match open.lock() {
        Ok(mut held) => std::mem::replace(&mut *held, now),
        Err(_the_lock_is_poisoned) => None,
    })
}

fn now(open: &Open) -> Result<Option<PathBuf>, Never> {
    Ok(match open.lock() {
        Ok(held) => held.clone(),
        Err(_the_lock_is_poisoned) => None,
    })
}

fn rows(open: &Open, kept: Option<&Path>) -> Result<Vec<Row>, Never> {
    let kept = match kept {
        Some(kept) => kept,
        None => {
            let Ok(row) = Row::placeholder(NO_HOME);

            return Ok(vec![row]);
        }
    };

    let Ok(now) = now(open);

    match now {
        Some(note) => note_rows(open, &note),
        None => list_rows(open, kept),
    }
}

fn list_rows(open: &Open, kept: &Path) -> Result<Vec<Row>, Never> {
    let opening = Arc::clone(open);
    let kept_for_asking = kept.to_path_buf();
    let Ok(asks) = Handler::and_stay(move |showing| {
        let Ok(answer) = asked_title(&opening, &kept_for_asking);

        showing.ask_aloud(ASKING_TITLE, answer);
    });
    let Ok(new) = Row::new(NEW_NOTE, Aside(""), asks);
    let mut rows = vec![new];

    match folder::listed(kept) {
        Ok(notes) => {
            for note in &notes {
                let Ok(row) = note_row(open, note);

                rows.push(row);
            }
        }
        Err(fault) => {
            let Ok(row) = Row::placeholder(&fault.to_string());

            rows.push(row);
        }
    }

    Ok(rows)
}

fn asked_title(open: &Open, kept: &Path) -> Result<Answer, Never> {
    let open = Arc::clone(open);
    let kept = kept.to_path_buf();

    Ok(Arc::new(move |showing: &dyn Showing, word: &str| {
        let Ok(title) = console_core_file_names::a_name(word);

        let title = match title {
            Some(title) => title,
            None => return,
        };

        let Ok(at) = folder::path(&kept, &title);
        let Ok(exists) = folder::exists(&at);

        match exists {
            Exists::Yes => showing.note(&format!("There is already a note called {title}")),
            Exists::No => {
                let Ok(answer) = asked_body(&open, &at, &title);

                showing.ask_aloud(ASKING_BODY, answer);
            }
        }
    }))
}

fn asked_body(open: &Open, at: &Path, title: &str) -> Result<Answer, Never> {
    let open = Arc::clone(open);
    let at = at.to_path_buf();
    let title = title.to_string();

    Ok(Arc::new(move |showing: &dyn Showing, body: &str| {
        match folder::written(&at, Draft { title: &title, body }) {
            Ok(()) => {
                let Ok(_was) = opened(&open, Some(at.clone()));

                showing.replace(FIRST_ROW_UNDER_THE_WAY_BACK);
            }
            Err(fault) => showing.note(&fault.to_string()),
        }
    }))
}

fn note_row(open: &Open, note: &Note) -> Result<Row, Never> {
    let opening = Arc::clone(open);
    let path = note.path.clone();
    let Ok(opens) = Handler::and_stay(move |showing| {
        let Ok(_was) = opened(&opening, Some(path.clone()));

        showing.replace(FIRST_ROW_UNDER_THE_WAY_BACK);
    });
    let shown = note.path.to_string_lossy().to_string();
    let Ok(offering) = shown_or_selected(&note.title, &note.path, move |showing| {
        let Ok(files) = InternalProgram::Files.at();

        showing.leave_running(vec![files.to_string_lossy().to_string(), shown.clone()]);
    });
    let Ok(row) = Row::new(&note.title, Aside(""), opens);
    let Ok(row) = row.opening();
    let Ok(row) = row.selectable(&note.path.to_string_lossy());

    row.offering(offering)
}

fn note_rows(open: &Open, note: &Path) -> Result<Vec<Row>, Never> {
    let backing = Arc::clone(open);
    let title = match note.file_stem() {
        Some(stem) => stem.to_string_lossy().to_string(),
        None => TITLE.to_string(),
    };
    let Ok(back) = Row::back(&title, move |showing| {
        let Ok(_was) = opened(&backing, None);

        showing.replace(NEW_NOTE_ROW);
    });
    let mut rows = vec![back];

    match folder::read(note) {
        Ok(lines) => {
            for line in &lines {
                let Ok(row) = Row::text(line, Aside(""));

                rows.push(row);
            }
        }
        Err(fault) => {
            let Ok(row) = Row::placeholder(&fault.to_string());

            rows.push(row);
        }
    }

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_panel::page::Nowhere;

    type Failure = Box<dyn std::error::Error>;

    fn said(open: &Open, kept: &Path) -> Result<Vec<String>, Never> {
        let Ok(rows) = rows(open, Some(kept));

        Ok(rows.into_iter().map(|row| row.says).collect())
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Pressed {
        Yes,
        NothingThere,
    }

    fn pressed(open: &Open, kept: &Path, says: &str) -> Result<Pressed, Never> {
        let Ok(rows) = rows(open, Some(kept));
        let does = rows.into_iter().find(|row| row.says == says).and_then(|row| row.does);

        Ok(match does {
            Some(Handler::Call(act)) => {
                let _stays = act(&Nowhere);

                Pressed::Yes
            }
            Some(Handler::Run(_)) | None => Pressed::NothingThere,
        })
    }

    #[test]
    fn a_note_answered_with_a_title_and_a_body_is_written_and_opened() -> Result<(), Failure> {
        let kept = console_core_temporary_directories::fresh("notes-card-made")?;
        let open: Open = Arc::new(Mutex::new(None));
        let Ok(at) = folder::path(&kept, "Groceries");
        let Ok(answered) = asked_body(&open, &at, "Groceries");

        answered(&Nowhere, "milk and eggs");

        let kept_as = std::fs::read_to_string(&at)?;

        assert_eq!(kept_as, "# Groceries\n\nmilk and eggs\n");

        let Ok(shown) = said(&open, &kept);

        assert_eq!(shown.get(1..).map(<[String]>::to_vec), Some(vec!["milk and eggs".to_string()]), "{shown:?}");

        Ok(())
    }

    #[test]
    fn a_title_that_would_be_a_path_writes_nothing() -> Result<(), Failure> {
        let kept = console_core_temporary_directories::fresh("notes-card-refused")?;
        let open: Open = Arc::new(Mutex::new(None));
        let Ok(answered) = asked_title(&open, &kept);

        answered(&Nowhere, "../Groceries");
        answered(&Nowhere, "  ");

        let written = std::fs::read_dir(&kept)?;

        assert_eq!(written.count(), 0);

        Ok(())
    }

    #[test]
    fn the_list_opens_a_note_and_the_way_back_returns_to_it() -> Result<(), Failure> {
        let kept = console_core_temporary_directories::fresh("notes-card-list")?;
        let open: Open = Arc::new(Mutex::new(None));
        let Ok(at) = folder::path(&kept, "Groceries");

        folder::written(&at, Draft { title: "Groceries", body: "milk" })?;

        assert_eq!(said(&open, &kept), Ok(vec![NEW_NOTE.to_string(), "Groceries".to_string()]));

        assert_eq!(pressed(&open, &kept, "Groceries"), Ok(Pressed::Yes));

        let Ok(inside) = said(&open, &kept);

        assert_eq!(inside.get(1..).map(<[String]>::to_vec), Some(vec!["milk".to_string()]), "{inside:?}");

        let back = inside.first().cloned().ok_or("an opened note has no way back")?;

        assert_eq!(pressed(&open, &kept, &back), Ok(Pressed::Yes));

        assert_eq!(said(&open, &kept), Ok(vec![NEW_NOTE.to_string(), "Groceries".to_string()]));

        Ok(())
    }
}
