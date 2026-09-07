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


use console_core_never::Never;
use console_core_number_conversion::fitted;

struct Entry {
    name: String,
    crc: u32,
    size: usize,
    at: usize,
}

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;

pub fn zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, Never> {
    let mut out = Vec::new();
    let mut entries = Vec::new();

    for (name, body) in files {
        let Ok(crc) = crc32(body);

        let entry = Entry { name: name.clone(), crc, size: body.len(), at: out.len() };
        let Ok(()) = four(&mut out, LOCAL);
        let Ok(()) = two(&mut out, 20);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(size) = fitted(entry.size);
        let Ok(named) = fitted(entry.name.len());
        let Ok(()) = four(&mut out, entry.crc);
        let Ok(()) = four(&mut out, size);
        let Ok(()) = four(&mut out, size);
        let Ok(()) = two(&mut out, named);
        let Ok(()) = two(&mut out, 0);
        out.extend_from_slice(entry.name.as_bytes());
        out.extend_from_slice(body);
        entries.push(entry);
    }

    let directory = out.len();

    for entry in &entries {
        let Ok(()) = four(&mut out, CENTRAL);
        let Ok(()) = two(&mut out, 20);
        let Ok(()) = two(&mut out, 20);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(size) = fitted(entry.size);
        let Ok(named) = fitted(entry.name.len());
        let Ok(at) = fitted(entry.at);
        let Ok(()) = four(&mut out, entry.crc);
        let Ok(()) = four(&mut out, size);
        let Ok(()) = four(&mut out, size);
        let Ok(()) = two(&mut out, named);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = two(&mut out, 0);
        let Ok(()) = four(&mut out, 0);
        let Ok(()) = four(&mut out, at);
        out.extend_from_slice(entry.name.as_bytes());
    }

    let listed = out.len().saturating_sub(directory);

    let Ok(()) = four(&mut out, END);
    let Ok(()) = two(&mut out, 0);
    let Ok(()) = two(&mut out, 0);
    let Ok(many) = fitted(entries.len());
    let Ok(listed) = fitted(listed);
    let Ok(directory) = fitted(directory);
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

pub fn crc32(bytes: &[u8]) -> Result<u32, Never> {
    let mut crc = 0xFFFF_FFFFu32;

    for byte in bytes {
        crc ^= u32::from(*byte);

        for _ in 0..8 {
            let odd = crc & 1 == 1;
            crc = crc.wrapping_shr(1);

            crc = match odd {
                true => crc ^ 0xEDB8_8320,
                false => crc,
            };
        }
    }

    Ok(!crc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crc32(body: &[u8]) -> u32 {
        let Ok(check) = super::crc32(body);

        check
    }

    fn zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
        let Ok(made) = super::zip(files);

        made
    }

    fn named(body: &str) -> Vec<(String, Vec<u8>)> {
        vec![
            ("manifest.json".to_string(), body.as_bytes().to_vec()),
            ("pad.js".to_string(), b"// nothing".to_vec()),
        ]
    }

    #[test]
    fn the_check_is_the_one_everybody_elses_is() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn an_archive_begins_as_an_archive_and_ends_as_one() {
        let made = zip(&named("{}"));
        assert_eq!(&made[..4], &LOCAL.to_le_bytes());
        assert_eq!(&made[made.len() - 22..made.len() - 18], &END.to_le_bytes());
    }

    #[test]
    fn every_file_is_named_in_it_twice() {
        let made = zip(&named("{}"));
        for name in ["manifest.json", "pad.js"] {
            let times = made.windows(name.len()).filter(|window| *window == name.as_bytes()).count();
            assert_eq!(times, 2, "{name}");
        }
    }

    #[test]
    fn the_list_at_the_end_says_where_the_list_is() {
        let made = zip(&named("{}"));
        let end = made.len() - 22;
        let many = u16::from_le_bytes([made[end + 10], made[end + 11]]);
        let listed = u32::from_le_bytes(made[end + 12..end + 16].try_into().expect("four")) as usize;
        let at = u32::from_le_bytes(made[end + 16..end + 20].try_into().expect("four")) as usize;
        assert_eq!(many, 2);
        assert_eq!(at + listed, end);
        assert_eq!(&made[at..at + 4], &CENTRAL.to_le_bytes());
    }

    #[test]
    fn what_goes_in_is_what_comes_out() {
        let made = zip(&named("{\"name\": \"Console\"}"));
        let held = b"{\"name\": \"Console\"}";
        assert!(made.windows(held.len()).any(|window| window == held));
    }

    #[test]
    fn nothing_at_all_is_still_an_archive() {
        let made = zip(&[]);
        assert_eq!(made.len(), 22);
        assert_eq!(&made[..4], &END.to_le_bytes());
    }
}
