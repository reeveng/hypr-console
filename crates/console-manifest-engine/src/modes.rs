//! What mode a file under `files/` is installed with, from where it goes and
//! how it begins. Nothing declares it: a path under a `bin` is run, a file
//! that opens with `#!` or an ELF header is run, and the rest is read. The
//! engine installs by this answer and the nested desktop stages by it, so it
//! is said once, here.

use console_core_never::Never;

pub fn of(live: &str, head: &[u8]) -> Result<u32, Never> {
    Ok(match live.contains("/bin/") || live.contains("/sbin/") {
        true => 0o755,
        false => match head {
            [b'#', b'!', ..] => 0o755,
            [0x7f, b'E', b'L', b'F', ..] => 0o755,
            _ => 0o644,
        },
    })
}

#[cfg(test)]
mod tests {
    fn mode_of(live: &str, head: &[u8]) -> u32 {
        let Ok(mode) = super::of(live, head);

        mode
    }

    #[test]
    fn anything_in_a_bin_directory_is_meant_to_be_run() {
        assert_eq!(mode_of("/usr/local/bin/console", b"any"), 0o755);
        assert_eq!(mode_of("/usr/sbin/thing", b""), 0o755);
    }

    #[test]
    fn a_script_and_a_compiled_program_are_both_meant_to_be_run() {
        assert_eq!(mode_of("/etc/thing", b"#!/bin/sh"), 0o755);
        assert_eq!(mode_of("/etc/thing", b"\x7fELF\x02"), 0o755);
    }

    #[test]
    fn a_compiled_program_is_read_past_its_first_two_bytes() {
        assert_eq!(mode_of("/etc/thing", b"\x7fELF"), 0o755);
        assert_eq!(mode_of("/etc/thing", b"\x7fEL"), 0o644);
    }

    #[test]
    fn everything_else_is_only_read() {
        assert_eq!(mode_of("/etc/systemd/user/console.target", b"[Uni"), 0o644);
        assert_eq!(mode_of("/home/@user@/.config/kdeglobals", b"[Col"), 0o644);
    }
}
