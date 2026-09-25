//! The pool's words turned back into *ask again*.
//!
//! A bar module does not want the line a source said, it wants to know that
//! its reading is stale. `watching_layers` was that shape before the pool
//! existed -- a channel of nothing, one word per reason to look -- and four
//! programs were written against it, so this is the same channel with the pool
//! behind it. Moving one over is a change of one call.
//!
//! **What is worth asking after stays outside the pool.** `console-events`
//! relays a line as a line and never parses one, and the bar keeps different
//! lines from the wallpaper -- so the deciding happens here, on the near side
//! of the socket, and anything that wants to disagree asks
//! [`crate::subscriber`] itself the way `console-wallpaper` does.
//!
//! **[`about`] cannot be called without saying what the lines mean, and it
//! could.** It used to turn every line on a topic into *ask again* and ask
//! no one what the lines were, which is the one thing a subscriber must not be
//! handed: a reading answered by asking its own source is heard by the source
//! as a change, and the bar read the volume forty-seven times a second for as
//! long as it was up. The filter is an argument now rather than a thing a
//! caller might remember, and a watch that truly wants every line says
//! [`anything`] out loud. [`layers`] is the same call with the one filter the
//! compositor's own words already settle.
//!
//! **Getting into the pool is a word.** What these carry is *ask again* rather
//! than an answer, so a gap in one is a reading quietly out of date with
//! nothing on the way to correct it -- which is what every watch this replaced
//! meant by saying something the moment it connected. `bar-door` is the one
//! with no tick underneath it and so the one that would stay wrong.

use std::sync::mpsc::Sender;

use console_core_never::Never;
use console_program_contract::Topic;
use console_program_lifetime::threads;

use crate::bus;
use crate::sources::{ADAPTER, BLUEZ, NOTIFICATIONS, OURS, SCANNED};
use crate::subscription::{self, Received, Subscriber};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worth {
    Querying,
    Ignoring,
}

pub type Worthwhile = fn(&str) -> Result<Worth, Never>;

pub fn anything(_every_line_is_a_reason: &str) -> Result<Worth, Never> {
    Ok(Worth::Querying)
}

pub fn layers(say: Sender<()>) -> Result<(), Never> {
    let Ok(subscriber) = subscription::connect(&[Topic::Compositor]);

    saying(subscriber, surfaces, say)
}

pub fn about(topic: &Topic, worth: Worthwhile, say: Sender<()>) -> Result<(), Never> {
    let Ok(subscriber) = subscription::connect(std::slice::from_ref(topic));

    saying(subscriber, worth, say)
}

pub fn sound(line: &str) -> Result<Worth, Never> {
    let mut said = line.split_whitespace().skip_while(|word| *word != "on");

    Ok(match said.nth(1) {
        Some("sink" | "server") => Worth::Querying,
        Some(_not_what_the_reading_comes_from) => Worth::Ignoring,
        None => Worth::Querying,
    })
}

pub fn notifications(line: &str) -> Result<Worth, Never> {
    let Ok(said) = bus::message(line);

    let said = match said {
        Some(said) => said,
        None => return Ok(Worth::Ignoring),
    };

    Ok(match (said.interface, said.member) {
        (OURS | NOTIFICATIONS, _something_happened) => Worth::Querying,
        (_someone_elses_conversation, _member) => Worth::Ignoring,
    })
}

pub fn bluetooth(line: &str) -> Result<Worth, Never> {
    let owner = format!("The name {BLUEZ} ");
    let member = line
        .split_whitespace()
        .nth(1)
        .and_then(|named| named.rsplit('.').next());

    Ok(match (line.starts_with(&owner), member) {
        (true, Some(_) | None) => Worth::Querying,
        (false, Some("PropertiesChanged")) => {
            let Ok(worth) = naming(line, &["'Powered'", "'Connected'"]);

            worth
        }
        (false, Some("InterfacesAdded" | "InterfacesRemoved")) => {
            let Ok(worth) = naming(line, &[&format!("'{ADAPTER}'")]);

            worth
        }
        (false, Some(_) | None) => Worth::Ignoring,
    })
}

pub fn scanned(line: &str) -> Result<Worth, Never> {
    naming(line, &[SCANNED])
}

fn naming(line: &str, shown: &[&str]) -> Result<Worth, Never> {
    Ok(match shown.iter().any(|word| line.contains(word)) {
        true => Worth::Querying,
        false => Worth::Ignoring,
    })
}

fn surfaces(line: &str) -> Result<Worth, Never> {
    let Ok(worth) = console_onscreen::worth_asking_after(line);

    Ok(match worth {
        console_onscreen::Worth::Querying => Worth::Querying,
        console_onscreen::Worth::Ignoring => Worth::Ignoring,
    })
}

fn saying(subscriber: Subscriber, worth: Worthwhile, say: Sender<()>) -> Result<(), Never> {
    let Ok(()) = threads::let_go(std::thread::spawn(move || {
        let Ok(received) = subscriber.received();

        for event in received.iter() {
            let asking = match &event {
                Received::Connected => Worth::Querying,
                Received::Event(change) => {
                    let Ok(asking) = worth(&change.text);

                    asking
                }
            };

            match asking {
                Worth::Querying => {
                    match say.send(()) {
                        Ok(()) => {},
                        Err(_no_one_is_listening) => return,
                    }
                }
                Worth::Ignoring => {},
            }
        }
    }));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sound_is_read_again_when_a_sink_or_the_server_changed() {
        let Ok(sink) = sound("Event 'change' on sink #49");
        let Ok(server) = sound("Event 'change' on server");
        let Ok(gone) = sound("Event 'remove' on sink #49");

        assert_eq!(sink, Worth::Querying);
        assert_eq!(server, Worth::Querying);
        assert_eq!(gone, Worth::Querying);
    }

    #[test]
    fn asking_what_the_volume_is_does_not_ask_what_the_volume_is() {
        for line in [
            "Event 'new' on client #312779",
            "Event 'change' on client #312779",
            "Event 'remove' on client #312779",
        ] {
            let Ok(worth) = sound(line);

            assert_eq!(worth, Worth::Ignoring, "{line}");
        }
    }

    #[test]
    fn a_stream_starting_is_not_the_volume_changing() {
        let Ok(worth) = sound("Event 'new' on sink-input #74");

        assert_eq!(worth, Worth::Ignoring);
    }

    #[test]
    fn a_line_pactl_has_never_printed_is_read_again_rather_than_dropped() {
        let Ok(worth) = sound("Subscribed.");

        assert_eq!(worth, Worth::Querying);
    }

    #[test]
    fn the_wifi_is_read_again_when_a_scan_finished_and_not_when_a_neighbour_is_seen_again() {
        let Ok(scan) = scanned(
            "/org/freedesktop/NetworkManager/Devices/4: org.freedesktop.DBus.Properties.PropertiesChanged ('org.freedesktop.NetworkManager.Device.Wireless', {'LastScan': <int64 144930545>}, @as [])",
        );
        let Ok(seen) = scanned(
            "/org/freedesktop/NetworkManager/AccessPoint/1082: org.freedesktop.DBus.Properties.PropertiesChanged ('org.freedesktop.NetworkManager.AccessPoint', {'Strength': <byte 0x45>, 'LastSeen': <144925>}, @as [])",
        );

        assert_eq!(scan, Worth::Querying);
        assert_eq!(seen, Worth::Ignoring);
    }

    #[test]
    fn bluetooth_is_read_again_when_it_is_switched_or_something_connects() {
        for line in [
            concat!(
                "/org/bluez/hci0: org.freedesktop.DBus.Properties.PropertiesChanged ",
                "('org.bluez.Adapter1', {'Powered': <false>, 'PowerState': <'off'>}, @as [])"
            ),
            concat!(
                "/org/bluez/hci0/dev_AC_80_0A_12_34_56: org.freedesktop.DBus.Properties.PropertiesChanged ",
                "('org.bluez.Device1', {'Connected': <true>}, @as [])"
            ),
            concat!(
                "/: org.freedesktop.DBus.ObjectManager.InterfacesRemoved ",
                "(objectpath '/org/bluez/hci0', ['org.freedesktop.DBus.Properties', 'org.bluez.Adapter1'])"
            ),
            "The name org.bluez is owned by :1.7",
            "The name org.bluez does not have an owner",
        ] {
            let Ok(worth) = bluetooth(line);

            assert_eq!(worth, Worth::Querying, "{line}");
        }
    }

    #[test]
    fn a_scan_going_on_is_not_bluetooth_changing() {
        for line in [
            "Monitoring signals from all objects owned by org.bluez",
            concat!(
                "/org/bluez/hci0/dev_5C_E7_1D_AA_BB_CC: org.freedesktop.DBus.Properties.PropertiesChanged ",
                "('org.bluez.Device1', {'RSSI': <int16 -71>}, @as [])"
            ),
            concat!(
                "/org/bluez/hci0/dev_AC_80_0A_12_34_56: org.freedesktop.DBus.Properties.PropertiesChanged ",
                "('org.bluez.Device1', {'ServicesResolved': <true>}, @as [])"
            ),
            concat!(
                "/org/bluez/hci0: org.freedesktop.DBus.Properties.PropertiesChanged ",
                "('org.bluez.Adapter1', {'Discovering': <true>}, @as [])"
            ),
            concat!(
                "/: org.freedesktop.DBus.ObjectManager.InterfacesAdded ",
                "(objectpath '/org/bluez/hci0/dev_5C_E7_1D_AA_BB_CC', ",
                "{'org.bluez.Device1': {'Address': <'5C:E7:1D:AA:BB:CC'>, 'Connected': <false>}})"
            ),
        ] {
            let Ok(worth) = bluetooth(line);

            assert_eq!(worth, Worth::Ignoring, "{line}");
        }
    }

    #[test]
    fn a_notification_arriving_or_going_rings_the_bell() {
        let arrived = concat!(
            "  Sender=:1.92 Destination=org.freedesktop.Notifications ",
            "Path=/org/freedesktop/Notifications ",
            "Interface=org.freedesktop.Notifications  Member=Notify"
        );
        let closed = concat!(
            "  Sender=:1.65 Path=/org/freedesktop/Notifications ",
            "Interface=org.freedesktop.Notifications  Member=NotificationClosed"
        );
        let Ok(arrived) = notifications(arrived);
        let Ok(closed) = notifications(closed);

        assert_eq!(arrived, Worth::Querying);
        assert_eq!(closed, Worth::Querying);
    }

    #[test]
    fn the_mode_changing_rings_the_bell() {
        let set = concat!(
            "  Sender=:1.92 Destination=org.freedesktop.Notifications ",
            "Path=/org/freedesktop/Notifications Interface=console.Notifications  Member=Quieten"
        );
        let Ok(set) = notifications(set);

        assert_eq!(set, Worth::Querying);
    }

    #[test]
    fn the_bus_talking_about_connections_is_not_a_notification() {
        for line in [
            concat!(
                "  Sender=:1.92 Destination=org.freedesktop.DBus ",
                "Path=/org/freedesktop/DBus Interface=org.freedesktop.DBus  Member=Hello"
            ),
            concat!(
                "  Sender=org.freedesktop.DBus Path=/org/freedesktop/DBus ",
                "Interface=org.freedesktop.DBus  Member=NameOwnerChanged"
            ),
        ] {
            let Ok(worth) = notifications(line);

            assert_eq!(worth, Worth::Ignoring, "{line}");
        }
    }

    #[test]
    fn someone_elses_conversation_on_the_bus_is_not_a_notification() {
        let mpris = concat!(
            "  Sender=:1.92 Destination=org.mpris.MediaPlayer2.console ",
            "Path=/org/mpris/MediaPlayer2 ",
            "Interface=org.freedesktop.DBus.Properties  Member=Get"
        );
        let Ok(worth) = notifications(mpris);

        assert_eq!(worth, Worth::Ignoring);
    }

    #[test]
    fn what_carries_no_message_at_all_is_not_a_notification() {
        for line in ["‣ Type=method_call  Endian=l  Flags=0", "  };", ""] {
            let Ok(worth) = notifications(line);

            assert_eq!(worth, Worth::Ignoring, "{line}");
        }
    }
}
