//! Decode the pictures a list wants, once, into the one file it reads.
//!
//!     panel-pictures --side 32 /usr/share/icons/.../firefox.svg /usr/share/pixmaps/x.png
//!
//! Off the panel, like `files-thumbnails` and for the same reason: this is the work
//! that was making the menu slow to appear, and doing it where the panel draws
//! is doing it in the one place where nothing else can happen. The panel that
//! asks for it is already on the screen and goes on answering buttons; what
//! this is for is the next opening.
//!
//! The side is what the caller draws at, and it is half of what the picture is
//! called: the rows want one size and the home screen wants another, and the
//! same file at two sizes is two pictures. A run with no `--side` is the rows',
//! which is the size everything in this store was before there were two.
//!
//! What is asked for is decoded again whether or not the store already has it,
//! and everything else in the store whose file still exists is kept. So a
//! package that adds an application costs one rebuild of the pictures that
//! application's list wanted, and one that removes it leaves nothing behind.
//!
//! Every picture asked for is decoded at once, one per core, through
//! `console_concurrency`: a list of sixty is sixty SVGs rasterised, none of them
//! reading another. The store is still written once, after the last of them.
//!
//! `console_panel::pictures` is the file's shape, who reads it and why.


use console_core_geometry::Size;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use console_panel::pictures::{self, Picture};
use console_panel::strip::PICTURE;

fn main() -> ExitCode {
    let said: Vec<String> = std::env::args().skip(1).collect();
    let Ok(asked) = parse_arguments(&said);

    let (side, wanted) = match asked {
        Some((side, wanted)) => (side, wanted),
        None => {
            eprintln!("usage: panel-pictures [--side PIXELS] FILE...");

            return ExitCode::FAILURE;
        }
    };

    let Ok(mut made) = load_store();

    let every = console_concurrency::map(&wanted, |of| decode(of, side));

    let every = match every {
        Ok(every) => every,
        Err(fault) => {
            eprintln!("panel-pictures: {fault}");

            return ExitCode::FAILURE;
        }
    };

    for (of, drawn) in wanted.iter().zip(every) {
        let Ok(named) = pictures::key(of, side);
        let Ok(drawn) = drawn;

        match drawn {
            Some(picture) => {
                made.insert(named.clone(), Picture { of: named, ..picture });
            },
            None => {
                made.remove(&named);
            },
        }
    }

    let pictures: Vec<Picture> = made.into_values().collect();

    let Ok(written) = save_store(&pictures);

    match written {
        Written::Yes => ExitCode::SUCCESS,
        Written::No => ExitCode::FAILURE,
    }
}

fn parse_arguments(said: &[String]) -> Result<Option<(pictures::Side, Vec<String>)>, Never> {
    let Ok(rows) = fitted::<i32, u32>(PICTURE);
    let read = console_core_iteration::iterate((pictures::Side(rows), Vec::new(), said.iter()), |(side, mut wanted, mut words)| {
        Ok(match words.next() {
            Some(word) => match word.as_str() == pictures::SIDE {
                true => match words.next().map(|said| said.parse()) {
                    Some(Ok(said)) => Step::Again((pictures::Side(said), wanted, words)),
                    Some(Err(_not_a_size)) => Step::Halt(None),
                    None => Step::Halt(None),
                },
                false => {
                    wanted.push(word.clone());

                    Step::Again((side, wanted, words))
                }
            },
            None => Step::Halt(Some((side, wanted))),
        })
    });

    Ok(match read {
        Ok(Some((side, wanted))) => match wanted.is_empty() {
            true => None,
            false => Some((side, wanted)),
        },
        Ok(None) => None,
        Err(_endless) => None,
    })
}

fn load_store() -> Result<BTreeMap<String, Picture>, Never> {
    let Ok(store) = pictures::store();

    let store = match store {
        Some(store) => store,
        None => return Ok(BTreeMap::new()),
    };

    let bytes = match std::fs::read(store) {
        Ok(bytes) => bytes,
        Err(_unreadable) => return Ok(BTreeMap::new()),
    };

    let index = match pictures::read(&bytes) {
        Ok(Some(index)) => index,
        Ok(None) | Err(_) => return Ok(BTreeMap::new()),
    };

    Ok(index
        .into_iter()
        .filter(|(named, _)| match pictures::parse_key(named) {
            Ok(Some((of, _))) => std::path::Path::new(of).exists(),
            Ok(None) | Err(_) => false,
        })
        .filter_map(|(of, found)| {
            let Ok(held) = found.in_store(&bytes);

            let held = match held {
                Some(held) => held,
                None => return None,
            };

            let pixels = held.to_vec();
            Some((
                of.clone(),
                Picture { of, width: found.width, height: found.height, stride: found.stride, pixels },
            ))
        })
        .collect())
}

fn decode(of: &str, side: pictures::Side) -> Result<Option<Picture>, Never> {
    let read = console_pictures::decoded(Path::new(of), Size { width: side.0, height: side.0 });

    let held = match read {
        Ok(Some(held)) => held,
        Ok(None) => return Ok(None),
        Err(why) => {
            eprintln!("panel-pictures: {of}: {why}");

            return Ok(None);
        }
    };

    Ok(Some(Picture {
        of: of.to_string(),
        width: held.width,
        height: held.height,
        stride: held.stride,
        pixels: held.bytes.to_vec(),
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Written {
    Yes,
    No,
}

fn save_store(pictures: &[Picture]) -> Result<Written, Never> {
    let Ok(at) = pictures::store();

    let at = match at {
        Some(at) => at,
        None => return Ok(Written::No),
    };

    let above = match at.parent() {
        Some(above) => above,
        None => return Ok(Written::No),
    };

    match std::fs::create_dir_all(above) {
        Ok(_) => {},
        Err(fault) => {
            eprintln!("panel-pictures: {}: {fault}", above.display());

            return Ok(Written::No);
        }
    }

    let Ok(said) = pictures::serialize(pictures);

    Ok(match console_core_atomic_writes::whole(&at, &said) {
        Ok(()) => Written::Yes,

        Err(fault) => {
            eprintln!("panel-pictures: {}: {fault}", at.display());

            Written::No
        }
    })
}
