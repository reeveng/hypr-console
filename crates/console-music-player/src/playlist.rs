//! The order songs play in, and where in that order we are.
//!
//! Every fault this desktop has watched in kew came out of one arrangement: two
//! lists that had to agree, and nothing that made them. Filling the library
//! into the playing list left the unshuffled list it restores from empty, so
//! pressing shuffle off copied nothing over the library. The restore then freed
//! every node of the playing list under the thread that answers the bus, the
//! song playing among them, which read as a crash or as a dead queue depending
//! on when the other thread looked. And a toggle marked the next song as
//! needing working out, so the press arriving before the working out was
//! dropped, and a button that does nothing on a handheld reads as a button that
//! is broken.
//!
//! There is one list here and it is never reordered. `songs` is the library in
//! the order it was read, `order` is a permutation of positions into it, and
//! `at` is where in that permutation we are. Shuffling builds a new permutation
//! and unshuffling builds the identity one; neither moves a song and neither
//! frees one. The song playing cannot be lost by a toggle, because the toggle
//! ends by finding where that song went, and the press after a toggle moves
//! because nothing was left over to work out.
//!
//! A press of next and a song reaching its own end are different questions, and
//! they are asked separately. [`Playlist::onward`] is the button, and always
//! leaves the song it was on; [`Playlist::finished`] is the song ending, and is
//! the only one [`Over::Again`] means anything to. Answering both down one path
//! is how repeating a track comes to eat the button.
//!
//! The shuffle is seeded so a test can press it. What seeds it is the clock, at
//! the call site: a player that shuffled the same way every morning would be
//! worse than one nobody can check.

use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Order {
    Any,
    #[default]
    AsListed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Over {
    On,
    Again,
    #[default]
    Round,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    Yes,
    No,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Playlist {
    songs: Vec<PathBuf>,
    order: Vec<usize>,
    at: usize,
    ordered: Order,
    over: Over,
}

impl Playlist {
    pub fn of(songs: Vec<PathBuf>) -> Result<Playlist, Never> {
        let Ok(order) = listed(songs.len());

        Ok(Playlist { songs, order, at: 0, ordered: Order::AsListed, over: Over::default() })
    }

    pub fn opened(songs: Vec<PathBuf>, song: &Path) -> Result<Playlist, Never> {
        let held = Playlist::of(songs)?;
        let Ok(at) = held.holding(song);

        Ok(Playlist { at, ..held })
    }

    pub fn song(&self) -> Result<Option<&Path>, Never> {
        Ok(self
            .order
            .get(self.at)
            .and_then(|held| self.songs.get(*held))
            .map(PathBuf::as_path))
    }

    pub fn ordering(&self) -> Result<Order, Never> {
        Ok(self.ordered)
    }

    pub fn repeating(&self) -> Result<Over, Never> {
        Ok(self.over)
    }

    pub fn onward(&mut self) -> Result<Moved, Never> {
        match self.order.is_empty() {
            true => return Ok(Moved::No),
            false => {},
        }

        let last = self.order.len().saturating_sub(1);

        match self.at < last {
            true => {
                self.at = self.at.saturating_add(1);

                Ok(Moved::Yes)
            },
            false => self.round_to(0),
        }
    }

    pub fn back(&mut self) -> Result<Moved, Never> {
        match self.order.is_empty() {
            true => return Ok(Moved::No),
            false => {},
        }

        match self.at > 0 {
            true => {
                self.at = self.at.saturating_sub(1);

                Ok(Moved::Yes)
            },
            false => self.round_to(self.order.len().saturating_sub(1)),
        }
    }

    pub fn finished(&mut self) -> Result<Moved, Never> {
        match self.order.is_empty() {
            true => return Ok(Moved::No),
            false => {},
        }

        match self.over {
            Over::Again => Ok(Moved::Yes),
            Over::Round | Over::On => self.onward(),
        }
    }

    pub fn shuffling(&mut self, order: Order, seed: u64) -> Result<(), Never> {
        let Ok(playing) = self.song();
        let held = playing.map(Path::to_path_buf);

        let Ok(ordered) = match order {
            Order::AsListed => listed(self.songs.len()),
            Order::Any => shuffled(self.songs.len(), seed),
        };

        self.order = ordered;
        self.ordered = order;

        let Ok(at) = match held {
            Some(song) => self.holding(&song),
            None => Ok(0),
        };

        self.at = at;

        Ok(())
    }

    pub fn repeat(&mut self, over: Over) -> Result<(), Never> {
        self.over = over;

        Ok(())
    }

    fn round_to(&mut self, at: usize) -> Result<Moved, Never> {
        match self.over {
            Over::Round | Over::Again => {
                self.at = at;

                Ok(Moved::Yes)
            },
            Over::On => Ok(Moved::No),
        }
    }

    fn holding(&self, song: &Path) -> Result<usize, Never> {
        Ok(self
            .order
            .iter()
            .position(|held| self.songs.get(*held).map(PathBuf::as_path) == Some(song))
            .unwrap_or_default())
    }
}

fn listed(songs: usize) -> Result<Vec<usize>, Never> {
    Ok((0..songs).collect())
}

fn shuffled(songs: usize, seed: u64) -> Result<Vec<usize>, Never> {
    let Ok(mut order) = listed(songs);
    let Ok(mut rolling) = Rolling::from(seed);

    let mut at = songs;

    while at > 1 {
        at = at.saturating_sub(1);

        let Ok(other) = rolling.under(at.saturating_add(1));

        order.swap(at, other);
    }

    Ok(order)
}

struct Rolling(u64);

impl Rolling {
    fn from(seed: u64) -> Result<Rolling, Never> {
        Ok(Rolling(seed | 1))
    }

    fn rolled(&mut self) -> Result<u64, Never> {
        let mut held = self.0;

        held ^= held.wrapping_shr(12);
        held ^= held.wrapping_shl(25);
        held ^= held.wrapping_shr(27);
        self.0 = held;

        Ok(held.wrapping_mul(0x2545_F491_4F6C_DD1D))
    }

    fn under(&mut self, bound: usize) -> Result<usize, Never> {
        let Ok(rolled) = self.rolled();
        let Ok(held) = fitted::<usize, u64>(bound);
        let under = rolled.checked_rem(held).unwrap_or_default();

        fitted::<u64, usize>(under)
    }
}
