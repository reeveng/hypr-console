//! What the desktop has said, drawn.
//!
//!     notices-panel
//!     notices-panel Earlier
//!
//! The bell on the right of the bar opens it, and tapping the bell again puts
//! it away, which is how every other icon along there works.
//!
//! What is here is the asking of mako. What each tab holds once it has been
//! asked is `console_notices::rows`, where it can be read without a mako to
//! ask.

use std::sync::Arc;

use console_notices::reading::{self, Notice};
use console_notices::rows::{Chosen, TABS, earlier_rows, gone_rows, one_rows, waiting_rows};
use console_panel::actor::{self, Addr, Answer};
use console_panel::page::{Does, Page, Row, Rows, Showing, Watch};
use console_panel::running::said;
use console_panel::{chooser, panel};

/// mako, asked about itself.
fn makoctl(argv: &[&str]) -> String {
    said(&[&["makoctl"], argv].concat())
}

/// What is on the screen now.
fn waiting() -> Vec<Notice> {
    reading::read(&makoctl(&["list", "-j"]))
}

/// What has been let go of. mako keeps the last few, and its configuration
/// says how many.
fn earlier() -> Vec<Notice> {
    reading::read(&makoctl(&["history", "-j"]))
}

// ------------------------------------------------------------------ looking

/// What the Waiting tab is looking at: the list, or one notification whole.
///
/// The one thing this panel holds between a drawing and the next, because the
/// pages are asked for again on every redraw. B unwinds it: out of the
/// notification, then out of the panel.
#[derive(Clone, Copy)]
enum Onto {
    List,
    One(u32),
}

/// The whole of what this panel holds, and the only thing that owns it.
///
/// One field, because one field is all there is: everything else on the two
/// tabs is asked of mako at the moment it is drawn. It is a machine rather
/// than a value behind a lock because the thumb writes it on the main thread
/// and the tab is read on another, and a state with one owner cannot be half
/// way through being written when the reader arrives.
struct Looking {
    onto: Onto,
}

/// Everything that can happen to it, and nothing else.
enum Msg {
    /// Look at something else.
    Look(Onto),
    /// What is it looking at.
    At(Answer<Onto>),
}

impl actor::Machine for Looking {
    type Msg = Msg;

    fn step(self, message: Msg) -> Self {
        match message {
            Msg::Look(onto) => Looking { onto },
            Msg::At(answer) => {
                let _ = answer.say(self.onto);
                self
            },
        }
    }
}

/// Where the panel reaches it. Cloned into every closure that used to be
/// handed the lock.
type Held = Addr<Msg>;

/// What it is looking at, asked of the owner.
///
/// The list, if the owner has gone: this panel is on its way out by then, and
/// the list is the tab as it opens rather than a page hanging on a
/// notification that no longer exists.
fn looking_at(held: &Held) -> Onto {
    match held.ask(Msg::At) {
        Ok(onto) => onto,
        Err(_) => Onto::List,
    }
}

/// Look at something else, and stand on a given row of it.
///
/// Said rather than asked, and the redraw underneath it is what reads the
/// answer. The two cannot cross: the message goes down the mailbox before
/// `replace` is called, and the question the redraw asks goes down the same
/// mailbox behind it.
fn look(held: &Held, onto: Onto, showing: &dyn Showing, row: usize) {
    let _ = held.tell(Msg::Look(onto));
    showing.replace(row);
}

/// Back to the list, standing at the top of it.
///
/// Not on the row it was opened from, which is what the settings do. There the
/// list is the same list on the way back; here it may be one row shorter,
/// because the ordinary reason to open a notification is to be done with it.
/// The top of a list of two or three is never far from wherever you were, and
/// a remembered row that no longer exists is a panel with nothing highlighted.
fn went_up(held: &Held, showing: &dyn Showing) {
    look(held, Onto::List, showing, 0);
}

/// The way back up, for the rows that build themselves.
fn back_up(held: &Held) -> Chosen {
    let held = held.clone();
    Arc::new(move |showing: &dyn Showing| went_up(&held, showing))
}

/// Where the highlight lands on a notification's own page: past the way back,
/// on the words it was opened to read.
const DEEPER: usize = 1;

// -------------------------------------------------------------- what it does

/// Open one notification, whole.
fn open(held: &Held, id: u32) -> Does {
    let held = held.clone();
    Does::and_stay(move |showing| look(&held, Onto::One(id), showing, DEEPER))
}

/// Take one off the screen, and go back to what is left.
///
/// Not `later`. Dismissing is one call to a daemon on the same bus and is over
/// before the next frame, and handing it off would draw the list once with the
/// notification still in it.
fn dismiss(held: &Held, id: u32) -> Does {
    let held = held.clone();
    Does::and_stay(move |showing| {
        makoctl(&["dismiss", "-n", &id.to_string()]);
        went_up(&held, showing);
    })
}

/// Take the lot off the screen.
///
/// Nothing is asked first. Everything cleared here is in Earlier a moment
/// later -- mako keeps a dismissed notification in its history the same as one
/// that ran out of seconds -- so this moves them rather than throwing them
/// away, and a question about a press that can be walked back is a question
/// somebody learns to answer without reading it.
fn clear() -> Does {
    Does::and_stay(|showing| {
        makoctl(&["dismiss", "--all"]);
        showing.refresh();
    })
}

// ---------------------------------------------------------------- the tabs

fn waiting_tab(looking: &Held) -> Vec<Row> {
    let held = waiting();

    match looking_at(looking) {
        Onto::List => waiting_rows(&held, |notice| open(looking, notice.id), clear()),
        // The one it was opened on, if mako still has it. It can go while its
        // own page is up -- a thumb on the card, or the seconds running out --
        // and a page that emptied itself would be the panel moving under
        // somebody who is reading it.
        Onto::One(id) => match held.iter().find(|notice| notice.id == id) {
            Some(notice) => one_rows(notice, &back_up(looking), dismiss(looking, id)),
            None => gone_rows(&back_up(looking)),
        },
    }
}

fn earlier_tab() -> Vec<Row> {
    earlier_rows(&earlier())
}

/// What says a notification arrived or went.
///
/// The bus mako owns, watched for the two members that change what is on this
/// tab: `Notify`, which is one being raised, and `NotificationClosed`, which
/// is one going however it went. Both are matched on the front of the name,
/// which is what keeps this panel from answering itself: every redraw asks
/// mako for `ListNotifications` over the same bus, and a watch that counted
/// that would ask for a redraw for every redraw, forever.
fn arriving() -> Watch {
    Watch::on(
        &["stdbuf", "-oL", "busctl", "--user", "monitor", "org.freedesktop.Notifications"],
        "Member=Notif",
    )
}

fn pages(looking: &Held) -> Vec<Page> {
    let drawing = looking.clone();
    let backing = looking.clone();
    vec![
        // No `meanwhile`. Every other tab on this desktop that has one draws
        // the list it is going to have, wearing YET where a reading has not
        // come back; here the list is the answer, and there is nothing true to
        // put up before mako has given it. Something plausible would be worse
        // than nothing: drawn as the empty tab, the highlight lands on the
        // switch, and the notifications arrive underneath it a moment later
        // with the highlight left standing on whatever took that row -- which
        // is a panel that moves the thing under your thumb between the tap and
        // the first press. Both readings are one call each to a daemon on this
        // bus, which is not a wait anybody sees.
        Page::new(TABS[0], Rows::asked(move || waiting_tab(&drawing)))
            .watching(arriving())
            // B is back before it is close: out of the notification, then out
            // of the panel, which is how back means one thing everywhere here.
            .on_back(move |showing| match looking_at(&backing) {
                Onto::List => true,
                Onto::One(_) => {
                    went_up(&backing, showing);
                    false
                }
            }),
        Page::new(TABS[1], Rows::asked(earlier_tab)),
    ]
}

fn main() {
    // A tab may be named, so something on the bar can land on the thing it
    // stands for. The tab is part of which door this is: the bell tapped twice
    // puts the panel away, the same as every other icon along that edge.
    let tab = std::env::args().nth(1);

    if chooser::alone(
        &format!("notices {}", tab.clone().unwrap_or_default()),
        chooser::Again::Closes,
    ) == chooser::Alone::No
    {
        return;
    }

    let looking = actor::supervise(|| Looking { onto: Onto::List });
    let held = looking.addr.clone();
    // The guide's width. Both tabs are text to be read rather than a list of
    // things to pick -- who said it on the left, what it said on the right --
    // and that is the shape the panel draws a row in when it is given a column
    // to hold the first words at.
    panel::show(Arc::new(move || pages(&held)), 250, tab.as_deref());
    // The panel is down and the last redraw has been drawn, so nothing is
    // going to ask again. Waited for rather than dropped, so that a message
    // already in the mailbox is finished with before the process leaves.
    looking.shutdown();
}
