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

/// One file, once it is in the archive.
struct Entry {
    name: String,
    crc: u32,
    size: usize,
    at: usize,
}

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;

/// Everything named, in one file a browser will open.
pub fn zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut entries = Vec::new();

    for (name, body) in files {
        let entry = Entry { name: name.clone(), crc: crc32(body), size: body.len(), at: out.len() };
        four(&mut out, LOCAL);
        two(&mut out, 20); // the version that reads a stored entry
        two(&mut out, 0); // no flags
        two(&mut out, 0); // stored
        two(&mut out, 0); // the time, which nothing here has an opinion about
        two(&mut out, 0); // and the date, for the same reason
        four(&mut out, entry.crc);
        four(&mut out, entry.size as u32);
        four(&mut out, entry.size as u32);
        two(&mut out, entry.name.len() as u16);
        two(&mut out, 0); // nothing extra
        out.extend_from_slice(entry.name.as_bytes());
        out.extend_from_slice(body);
        entries.push(entry);
    }

    let directory = out.len();
    for entry in &entries {
        four(&mut out, CENTRAL);
        two(&mut out, 20); // written by
        two(&mut out, 20); // and readable by
        two(&mut out, 0);
        two(&mut out, 0);
        two(&mut out, 0);
        two(&mut out, 0);
        four(&mut out, entry.crc);
        four(&mut out, entry.size as u32);
        four(&mut out, entry.size as u32);
        two(&mut out, entry.name.len() as u16);
        two(&mut out, 0); // nothing extra
        two(&mut out, 0); // and nothing to say about it
        two(&mut out, 0); // one disk, this one
        two(&mut out, 0);
        four(&mut out, 0);
        four(&mut out, entry.at as u32);
        out.extend_from_slice(entry.name.as_bytes());
    }
    let listed = out.len() - directory;

    four(&mut out, END);
    two(&mut out, 0);
    two(&mut out, 0);
    two(&mut out, entries.len() as u16);
    two(&mut out, entries.len() as u16);
    four(&mut out, listed as u32);
    four(&mut out, directory as u32);
    two(&mut out, 0); // and nothing to say about the whole of it
    out
}

fn two(out: &mut Vec<u8>, said: u16) {
    out.extend_from_slice(&said.to_le_bytes());
}

fn four(out: &mut Vec<u8>, said: u32) {
    out.extend_from_slice(&said.to_le_bytes());
}

/// The check a zip carries for every entry.
///
/// Written out rather than taken from a crate, for the same reason as the
/// archive around it: it is nine lines, and it is the only arithmetic here.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let odd = crc & 1 == 1;
            crc >>= 1;
            if odd {
                crc ^= 0xEDB8_8320;
            }
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(body: &str) -> Vec<(String, Vec<u8>)> {
        vec![
            ("manifest.json".to_string(), body.as_bytes().to_vec()),
            ("pad.js".to_string(), b"// nothing".to_vec()),
        ]
    }

    /// The one number in the format that is not a length, against the vector
    /// every implementation of it is checked with.
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

    /// Twice: once beside the file itself and once in the list at the end,
    /// which is the list a browser reads to find out what is in there.
    #[test]
    fn every_file_is_named_in_it_twice() {
        let made = zip(&named("{}"));
        for name in ["manifest.json", "pad.js"] {
            let times = made.windows(name.len()).filter(|window| *window == name.as_bytes()).count();
            assert_eq!(times, 2, "{name}");
        }
    }

    /// The list at the end says where the list begins and how long it is, and
    /// a reader that cannot find it reads an archive with nothing in it.
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

    /// A file's bytes are in there whole, because nothing here compresses.
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
