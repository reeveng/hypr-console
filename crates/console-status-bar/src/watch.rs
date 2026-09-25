//! What wakes a reading up.
//!
//! Each of these has something that says when it changed, and every one of
//! them is asked through `console-events` rather than opened here: the sound is
//! told by pipewire, the network by NetworkManager, Bluetooth by bluetoothd on
//! the system bus, the battery by the kernel when a supply changed, and every
//! one of them by the compositor when a panel opens over it. The battery was
//! the last to open its own `udevadm monitor`, on the argument that no other
//! program would ask about this machine's supplies; the pool holds a source
//! only while somebody listens, so a topic with one listener costs what the
//! bar's own process cost, and the bar stops being the one program with a
//! subscription nobody else can see.
//!
//! **A reading its source tells about is never read on a clock.** Bluetooth
//! was read every ten seconds by two `bluetoothctl`s with nothing telling it,
//! and the sound and the network every ten seconds beside a source that
//! already did. That was the net for a machine with no pool, and the pool's
//! own reconnection is the better one: getting in again is said as *ask
//! again*, so a pool that was down leaves a reading stale for as long as it
//! is down rather than for ever.
//!
//! Two keep a clock, each for something its source does not say. The battery's
//! uevent is raised when the firmware notifies, which it does for the cable
//! and need not do for a percent -- on the laptop this was written on the
//! charge fell two points in two minutes with the monitor silent -- and the
//! last of what `dwindling` watches for stops the machine. The network's
//! `nmcli monitor` says a device connected and never how strong the signal
//! is; the bus does say that, for every access point in range, which on a busy
//! street is a reading a second to learn about one of them.
//!
//! **A reading whose own asking is heard as a change never stops.** `pactl
//! subscribe` says a client appeared, changed, and went away; reading the
//! volume with `wpctl` *is* a client appearing, changing and going away. Handed
//! the line whole, this woke, asked, and was woken by its own asking: measured
//! on the device it stood at forty-seven readings a second with no one
//! touching the machine, and the sound server answering those connections was
//! most of what the desktop did while it was idle. The bell had the same shape
//! and only its settle kept it off the same cliff, back when what it read was
//! a program it had to run: `busctl monitor` showed that asking as traffic on
//! the bus, which is what it was woken by. It reads a file now and the shape
//! is gone, and the filtering below is what would have saved it either way.
//!
//! So no source's line is a reason to look until someone says it is. Each watch
//! carries what its own reading comes from, decided on this side of the socket the
//! way `console-wallpaper` decides what is worth waking for, and the two that want
//! every line say `anything` rather than say nothing. Getting that wrong in the
//! careful direction is a reading late until the next change, where a line no
//! one filtered costs the machine.
//!
//! **The bar keeps a wider list of compositor lines than anything else does.**
//! `console_onscreen::worth_asking_after` answers the question the doors ask --
//! did a layer open or close -- and says no to a workspace changing and to a
//! window opening, because neither moves a surface. Both move the bar: the
//! workspaces along the left are a row of what there is and which one you are
//! on, and on this desktop a window opening is a workspace appearing. So
//! [`surface_worth_asking_after`] is the bar's own reading of the same lines,
//! which is what `again`'s head says a subscriber that disagrees should write
//! rather than widening the one the wallpaper is also standing on.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use console_compositor::events::CompositorEvent;
use console_events::again::{self, Worth, Worthwhile, about, anything, layers};
use console_core_never::Never;
use console_program_contract::Topic;

use crate::reading::StatusItem;

pub const BELL: Duration = Duration::from_secs(10);

pub fn tick(item: StatusItem) -> Result<Option<Duration>, Never> {
    Ok(match item {
        StatusItem::Battery => Some(Duration::from_secs(30)),
        StatusItem::Network => Some(Duration::from_secs(60)),
        StatusItem::Bluetooth | StatusItem::Sound => None,
    })
}

fn change_source(item: StatusItem) -> Result<(Topic, Worthwhile), Never> {
    Ok(match item {
        StatusItem::Battery => (Topic::Battery, anything),
        StatusItem::Bluetooth => (Topic::Bluetooth, again::bluetooth),
        StatusItem::Network => (Topic::Network, anything),
        StatusItem::Sound => (Topic::Sound, again::sound),
    })
}

pub fn subscribe(item: StatusItem, say: Sender<()>) -> Result<(), Never> {
    let Ok((topic, worth)) = change_source(item);

    about(&topic, worth, say)
}

pub fn watching(item: StatusItem) -> Result<Receiver<()>, Never> {
    let (say, heard) = channel();
    let Ok(()) = layers(say.clone());
    let Ok(()) = subscribe(item, say);

    Ok(heard)
}

pub fn telling_notifications(say: Sender<()>) -> Result<(), Never> {
    about(&Topic::Notifications, again::notifications, say)
}

pub fn watching_notifications() -> Result<Receiver<()>, Never> {
    let (say, heard) = channel();
    let Ok(()) = layers(say.clone());
    let Ok(()) = telling_notifications(say);

    Ok(heard)
}

pub fn surface_worth_asking_after(line: &str) -> Result<Worth, Never> {
    let Ok(stirred) = console_compositor::events::read(line);

    Ok(match stirred {
        CompositorEvent::LayerOpened
        | CompositorEvent::LayerClosed
        | CompositorEvent::WorkspaceChanged
        | CompositorEvent::WindowOpened(_)
        | CompositorEvent::WindowClosed(_)
        | CompositorEvent::ScreenFocused => Worth::Querying,
        CompositorEvent::WindowRenamed(_)
        | CompositorEvent::WindowMoved
        | CompositorEvent::WindowFloated
        | CompositorEvent::WindowPinned
        | CompositorEvent::WindowFilled
        | CompositorEvent::ConfigurationReloaded
        | CompositorEvent::Ignored => Worth::Ignoring,
    })
}

pub fn telling_surfaces(say: Sender<()>) -> Result<(), Never> {
    about(&Topic::Compositor, surface_worth_asking_after, say)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY: [StatusItem; 4] =
        [StatusItem::Battery, StatusItem::Bluetooth, StatusItem::Network, StatusItem::Sound];

    #[test]
    fn every_reading_is_told_by_the_pool_rather_than_by_a_program_of_its_own() {
        let told: Vec<Topic> = EVERY
            .iter()
            .map(|item| {
                let Ok((topic, _worth)) = change_source(*item);

                topic
            })
            .collect();

        assert_eq!(told, [Topic::Battery, Topic::Bluetooth, Topic::Network, Topic::Sound]);
    }

    #[test]
    fn a_reading_its_source_tells_about_every_change_is_never_read_on_a_clock() {
        for item in [StatusItem::Bluetooth, StatusItem::Sound] {
            let Ok(every) = tick(item);

            assert_eq!(every, None, "{item:?} is still polled");
        }
    }

    #[test]
    fn what_no_source_says_is_read_on_a_long_clock_and_never_a_short_one() {
        for item in [StatusItem::Battery, StatusItem::Network] {
            let Ok(every) = tick(item);

            match every {
                Some(every) => assert!(every >= Duration::from_secs(30), "{item:?} every {every:?}"),
                None => panic!("{item:?} has something no source says and nothing to read it"),
            }
        }
    }
}
