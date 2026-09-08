//! What `busctl monitor` prints, read once.
//!
//! Two watches take their words from a bus monitor -- the bell and the player
//! -- and both decide the same way, on the interface a message is on and the
//! member it names. Everything else the monitor prints is the message being
//! spelled out underneath: a type, a sender, a body in braces, a blank line.
//!
//! The pool relays a line and never reads one, and that is still true. This is
//! the reading offered to whoever asked, kept in the crate that runs the
//! program, because the alternative is the shape of somebody else's output
//! spelled once per subscriber and drifting the day one of them is wrong about
//! it. What the fields *mean* is still the subscriber's: a member that matters
//! to the bell means nothing to the player.
//!
//! A line that carries only one of the two is not a message being announced,
//! and neither is a line that carries neither, so both come back as nothing.

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Message<'a> {
    pub interface: &'a str,
    pub member: &'a str,
}

pub fn message(line: &str) -> Result<Option<Message<'_>>, Never> {
    let Ok(interface) = after(line, "Interface=");
    let Ok(member) = after(line, "Member=");

    Ok(match (interface, member) {
        (Some(interface), Some(member)) => Some(Message { interface, member }),
        (Some(_), None) | (None, Some(_)) | (None, None) => None,
    })
}

fn after<'a>(line: &'a str, key: &str) -> Result<Option<&'a str>, Never> {
    let said = match line.split_once(key) {
        Some((_before, said)) => said,
        None => return Ok(None),
    };

    Ok(said.split_whitespace().next())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_is_the_interface_it_is_on_and_the_member_it_names() {
        let line = concat!(
            "  Sender=:1.92 Destination=org.freedesktop.Notifications ",
            "Path=/org/freedesktop/Notifications ",
            "Interface=org.freedesktop.Notifications  Member=Notify"
        );
        let Ok(said) = message(line);

        assert_eq!(
            said,
            Some(Message {
                interface: "org.freedesktop.Notifications",
                member: "Notify",
            })
        );
    }

    #[test]
    fn what_the_monitor_prints_around_a_message_is_not_one() {
        for line in [
            "‣ Type=method_call  Endian=l  Flags=0  Version=1 Cookie=2",
            "  MESSAGE \"ss\" {",
            "          STRING \"org.mpris.MediaPlayer2.Player\";",
            "  };",
            "",
        ] {
            let Ok(said) = message(line);

            assert_eq!(said, None, "{line}");
        }
    }

    #[test]
    fn a_reply_names_no_interface_and_is_not_a_message_being_announced() {
        let line = "  Sender=:1.65 Destination=:1.92 Member=Something";
        let Ok(said) = message(line);

        assert_eq!(said, None);
    }
}
