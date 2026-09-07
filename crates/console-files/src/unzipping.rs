//! What an archive is unpacked into, and when the folder inside it is one
//! folder too many.
//!
//! An archive off a mod site is one of two shapes and the person pressing Unzip
//! did not choose which. Some hold the files themselves; some hold a single
//! folder holding the files, so unpacked as they arrive they leave a folder
//! inside a folder. That is untidy anywhere else and it is the whole thing
//! here: The Sims reads a script mod from its Mods folder or one folder under
//! it and nowhere deeper, so a wrapper nobody asked for is the difference
//! between a mod that runs and a mod that is silently not there.
//!
//! So one folder is made, named for the archive, and a lone folder inside it is
//! lifted away. Unpacking into the folder being stood in was the other way, and
//! it is the one that cannot be taken back: forty files spread across Downloads
//! with nothing to say which of them arrived a moment ago.
//!
//! The kinds are the three the mod sites ship rather than everything `7z` can
//! read. A `.tar.gz` unpacked once is a `.tar`, and a row that answers a press
//! with another archive is a row that lied about what it does.

use console_core_never::Never;

use crate::places::Is;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packed {
    AnArchive,
    NotOne,
}

pub const KINDS: &[&str] = &[
    "application/zip",
    "application/x-zip-compressed",
    "application/x-7z-compressed",
    "application/vnd.rar",
    "application/x-rar",
    "application/x-rar-compressed",
];

pub const TRIES: usize = 64;

pub fn packed(kind: &str) -> Result<Packed, Never> {
    Ok(match KINDS.contains(&kind) {
        true => Packed::AnArchive,
        false => Packed::NotOne,
    })
}

pub fn named_for(archive: &str) -> Result<String, Never> {
    let stem = match archive.rsplit_once('.') {
        Some((stem, _)) => stem,
        None => archive,
    };
    let named = stem.trim();

    Ok(match named.is_empty() {
        true => archive.trim().to_string(),
        false => named.to_string(),
    })
}

pub fn beside(name: &str, taken: impl Fn(&str) -> bool) -> Result<Option<String>, Never> {
    match taken(name) {
        false => return Ok(Some(name.to_string())),
        true => {},
    }

    for number in 2..=TRIES {
        let tried = format!("{name} {number}");

        match taken(&tried) {
            false => return Ok(Some(tried)),
            true => {},
        }
    }

    Ok(None)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lift {
    TheFolderInside(String),
    Nothing,
}

pub fn lifting(inside: &[(String, Is)]) -> Result<Lift, Never> {
    let alone = inside.len() == 1;

    let Some((name, is)) = inside.first() else { return Ok(Lift::Nothing) };

    Ok(match (alone, is) {
        (true, Is::AFolder) => Lift::TheFolderInside(name.clone()),
        (true, Is::AFile) | (false, Is::AFolder) | (false, Is::AFile) => Lift::Nothing,
    })
}

pub fn while_unpacking(archive: &str) -> Result<String, Never> {
    Ok(format!(".unzipping {archive}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(said: &str) -> Packed {
        let Ok(packed) = packed(said);

        packed
    }

    fn named(archive: &str) -> String {
        let Ok(named) = named_for(archive);

        named
    }

    fn free(name: &str, taken: &[&str]) -> Option<String> {
        let taken: Vec<String> = taken.iter().map(|name| name.to_string()).collect();

        let Ok(beside) = beside(name, |tried| taken.iter().any(|held| held == tried));

        beside
    }

    fn inside(names: &[(&str, Is)]) -> Lift {
        let names: Vec<(String, Is)> =
            names.iter().map(|(name, is)| (name.to_string(), *is)).collect();

        let Ok(lift) = lifting(&names);

        lift
    }

    #[test]
    fn the_kinds_a_mod_site_ships_are_unzipped_and_a_photograph_is_not() {
        assert_eq!(kind("application/zip"), Packed::AnArchive);
        assert_eq!(kind("application/vnd.rar"), Packed::AnArchive);
        assert_eq!(kind("application/x-7z-compressed"), Packed::AnArchive);
        assert_eq!(kind("image/jpeg"), Packed::NotOne);
        assert_eq!(kind("inode/directory"), Packed::NotOne);
        assert_eq!(kind(""), Packed::NotOne);
    }

    #[test]
    fn a_thing_that_would_come_out_of_this_as_another_archive_is_not_offered() {
        assert_eq!(kind("application/x-compressed-tar"), Packed::NotOne);
        assert_eq!(kind("application/gzip"), Packed::NotOne);
    }

    #[test]
    fn the_folder_is_named_for_the_archive_without_what_it_is() {
        assert_eq!(named("WickedWhims.zip"), "WickedWhims");
        assert_eq!(named("MC Command Center v3.rar"), "MC Command Center v3");
        assert_eq!(named("hair.7z"), "hair");
    }

    #[test]
    fn a_name_that_is_all_extension_keeps_itself() {
        assert_eq!(named(".zip"), ".zip");
        assert_eq!(named("nodots"), "nodots");
    }

    #[test]
    fn the_second_unzip_of_one_archive_stands_beside_the_first() {
        assert_eq!(free("hair", &[]), Some("hair".to_string()));
        assert_eq!(free("hair", &["hair"]), Some("hair 2".to_string()));
        assert_eq!(free("hair", &["hair", "hair 2"]), Some("hair 3".to_string()));
    }

    #[test]
    fn a_folder_that_has_been_unzipped_too_many_times_is_answered_with_nothing() {
        let taken: Vec<String> = std::iter::once("hair".to_string())
            .chain((2..=TRIES).map(|number| format!("hair {number}")))
            .collect();

        let Ok(beside) = beside("hair", |tried| taken.iter().any(|held| held == tried));

        assert_eq!(beside, None);
    }

    #[test]
    fn one_folder_alone_inside_is_the_wrapper_and_is_lifted_away() {
        assert_eq!(
            inside(&[("WickedWhims", Is::AFolder)]),
            Lift::TheFolderInside("WickedWhims".to_string())
        );
    }

    #[test]
    fn anything_else_inside_is_what_the_archive_meant_to_hold() {
        assert_eq!(inside(&[("mod.package", Is::AFile)]), Lift::Nothing);
        assert_eq!(
            inside(&[("mod.package", Is::AFile), ("mod.ts4script", Is::AFile)]),
            Lift::Nothing
        );
        assert_eq!(inside(&[("hair", Is::AFolder), ("eyes", Is::AFolder)]), Lift::Nothing);
        assert_eq!(inside(&[]), Lift::Nothing);
    }

    #[test]
    fn what_is_being_unpacked_into_is_hidden_while_it_is_being_unpacked_into() {
        let Ok(while_) = while_unpacking("hair.zip");

        assert!(while_.starts_with('.'));
        assert!(while_.contains("hair.zip"));
    }
}
