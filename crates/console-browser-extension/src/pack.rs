//! A zip file, written by hand, because that is what an add-on is.
//!
//! A browser installs an add-on from one file with the pieces of it inside,
//! and that file is a zip. Nothing here compresses anything: an entry is
//! stored whole, which is a legal zip, is what a browser reads either way, and
//! is a hundred lines rather than a dependency that pulls a compressor onto a
//! handheld to save forty kilobytes on a file that is read once at startup.
//!
//! The pieces of the format are laid out in the order the format lays them
//! out, and the numbers in it are little-endian, which is the whole of what
//! there is to know.


use console_core_checksums::crc32;
use console_core_never::Never;
use console_core_number_conversion::fitted;

struct Entry {
    name: String,
    crc: u32,
    size: u32,
    at: u32,
}

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;

pub fn zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, Never> {
    let mut out = Vec::new();
    let mut entries = Vec::new();

    for (name, body) in files {
        let Ok(crc) = crc32::of(body);

        let Ok(size) = fitted(body.len());
        let Ok(at) = fitted(out.len());
        let entry = Entry { name: name.clone(), crc, size, at };
        let Ok(()) = four(&mut out, LOCAL);
        let Ok(()) = two(&mut out, 20);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(named) = fitted(entry.name.len());
        let Ok(()) = four(&mut out, entry.crc);
        let Ok(()) = four(&mut out, entry.size);
        let Ok(()) = four(&mut out, entry.size);
        let Ok(()) = two(&mut out, named);
        let Ok(()) = two(&mut out, 0);
        out.extend_from_slice(entry.name.as_bytes());
        out.extend_from_slice(body);
        entries.push(entry);
    }

    let Ok(directory) = fitted::<_, u32>(out.len());

    for entry in &entries {
        let Ok(()) = four(&mut out, CENTRAL);
        let Ok(()) = two(&mut out, 20);
        let Ok(()) = two(&mut out, 20);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(named) = fitted(entry.name.len());
        let Ok(()) = four(&mut out, entry.crc);
        let Ok(()) = four(&mut out, entry.size);
        let Ok(()) = four(&mut out, entry.size);
        let Ok(()) = two(&mut out, named);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = four(&mut out, 0);
        let Ok(()) = four(&mut out, entry.at);
        out.extend_from_slice(entry.name.as_bytes());
    }

    let Ok(written) = fitted::<_, u32>(out.len());
    let listed = written.saturating_sub(directory);

    let Ok(()) = four(&mut out, END);
    let Ok(()) = two(&mut out, 0);
    let Ok(()) = two(&mut out, 0);
    let Ok(many) = fitted(entries.len());
    let Ok(()) = two(&mut out, many);
    let Ok(()) = two(&mut out, many);
    let Ok(()) = four(&mut out, listed);
    let Ok(()) = four(&mut out, directory);
    let Ok(()) = two(&mut out, 0);
    Ok(out)
}

fn two(out: &mut Vec<u8>, said: u16) -> Result<(), Never> {
    out.extend_from_slice(&said.to_le_bytes());

    Ok(())
}

fn four(out: &mut Vec<u8>, said: u32) -> Result<(), Never> {
    out.extend_from_slice(&said.to_le_bytes());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_core_number_conversion::index;
    use std::error::Error;

    fn named(body: &str) -> Result<Vec<(String, Vec<u8>)>, Never> {
        Ok(vec![
            ("manifest.json".to_string(), body.as_bytes().to_vec()),
            ("pad.js".to_string(), b"// nothing".to_vec()),
        ])
    }

    fn four(made: &[u8], at: u32) -> Result<Option<[u8; 4]>, Never> {
        let Ok(at) = index(at);

        Ok(made.get(at..).and_then(<[u8]>::first_chunk::<4>).copied())
    }

    fn end(made: &[u8]) -> Result<u32, Box<dyn Error>> {
        let length = u32::try_from(made.len())?;
        let end = length.checked_sub(22).ok_or("shorter than its own end")?;

        Ok(end)
    }

    #[test]
    fn an_archive_begins_as_an_archive_and_ends_as_one() -> Result<(), Box<dyn Error>> {
        let Ok(files) = named("{}");
        let Ok(made) = zip(&files);
        let end = end(&made)?;

        assert_eq!(four(&made, 0), Ok(Some(LOCAL.to_le_bytes())));
        assert_eq!(four(&made, end), Ok(Some(END.to_le_bytes())));

        Ok(())
    }

    #[test]
    fn every_file_is_named_in_it_twice() {
        let Ok(files) = named("{}");
        let Ok(made) = zip(&files);

        for name in ["manifest.json", "pad.js"] {
            assert_eq!(made.windows(name.len()).filter(|window| *window == name.as_bytes()).count(), 2, "{name}");
        }
    }

    #[test]
    fn the_list_at_the_end_says_where_the_list_is() -> Result<(), Box<dyn Error>> {
        let Ok(files) = named("{}");
        let Ok(made) = zip(&files);
        let end = end(&made)?;
        let Ok(from) = index(end);
        let tail = made.get(from..).ok_or("no end")?;
        let many = tail.get(10..).and_then(<[u8]>::first_chunk::<2>).copied().map(u16::from_le_bytes);
        let Ok(listed) = four(tail, 12);
        let listed = listed.map(u32::from_le_bytes).ok_or("how long the list is")?;
        let Ok(at) = four(tail, 16);
        let at = at.map(u32::from_le_bytes).ok_or("where the list is")?;

        assert_eq!(many, Some(2));
        assert_eq!(at.checked_add(listed), Some(end));
        assert_eq!(four(&made, at), Ok(Some(CENTRAL.to_le_bytes())));

        Ok(())
    }

    #[test]
    fn what_goes_in_is_what_comes_out() {
        let Ok(files) = named("{\"name\": \"Console\"}");
        let Ok(made) = zip(&files);
        let held = b"{\"name\": \"Console\"}";

        assert!(made.windows(held.len()).any(|window| window == held));
    }

    #[test]
    fn nothing_at_all_is_still_an_archive() {
        let Ok(made) = zip(&[]);

        assert_eq!(made.len(), 22);
        assert_eq!(four(&made, 0), Ok(Some(END.to_le_bytes())));
    }
}
