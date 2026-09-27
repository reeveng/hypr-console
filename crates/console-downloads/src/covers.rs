//! A result's picture, fetched to a file beside where it will be kept.
//!
//! What a site sends back with a refusal is a page about the refusal, and
//! sometimes a picture of one: a placeholder a CDN serves with its 404. Kept,
//! that is a cover nobody chose, drawn beside a row as though it were the
//! thing found. So a status curl calls a failure is no picture at all, which is
//! `--fail`, and the body that came with it is never written.

use std::path::Path;

use console_core_external_programs::Program;
use console_core_never::Never;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fetched {
    Arrived,
    Failed,
}

pub fn fetched(from: &str, into: &Path) -> Result<Fetched, Never> {
    let Ok(mut curl) = Program::Curl.command();

    let done = curl
        .args(["--silent", "--fail", "--location", "--max-time", "20", "--output"])
        .arg(into)
        .arg(from)
        .status();

    Ok(match done.is_ok_and(|how| how.success()) {
        true => Fetched::Arrived,
        false => {
            let _ = std::fs::remove_file(into);

            Fetched::Failed
        },
    })
}
