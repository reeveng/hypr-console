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
//! worse than one no one can check.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use std::path::{Path, PathBuf};

const THE_FRONT_OF_THE_LIST: u32 = 0;

const NOTHING_TO_CHOOSE_FROM: u64 = 0;

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
  order: Vec<u32>,
  at: u32,
  ordered: Order,
  over: Over,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SavedQueue {
  pub songs: Vec<PathBuf>,
  pub order: Vec<u32>,
  pub ordered: Order,
  pub over: Over,
}

impl Playlist {
  pub fn of(songs: Vec<PathBuf>) -> Result<Playlist, Never> {
    let Ok(many) = fitted::<_, u32>(songs.len());
    let Ok(order) = listed(many);

    Ok(Playlist {
      songs,
      order,
      at: 0,
      ordered: Order::AsListed,
      over: Over::default(),
    })
  }

  pub fn opened(songs: Vec<PathBuf>, song: &Path) -> Result<Playlist, Never> {
    let held = Playlist::of(songs)?;
    let Ok(at) = held.position_of(song);

    Ok(Playlist { at, ..held })
  }

  #[must_use]
  pub(crate) fn checkpoint(&self) -> Result<SavedQueue, Never> {
    Ok(SavedQueue {
      songs: self.songs.clone(),
      order: self.order.clone(),
      ordered: self.ordered,
      over: self.over,
    })
  }

  #[must_use]
  pub(crate) fn restored(saved: SavedQueue, was: &Path) -> Result<Option<Self>, Never> {
    let mut slots = Vec::with_capacity(saved.songs.len());
    let mut songs = Vec::new();

    for song in saved.songs {
      match song
        .is_absolute()
        .then_some(())
        .and_then(|()| match song.metadata() {
          Ok(info) => info.is_file().then_some(()),
          Err(_unavailable) => None,
        }) {
        Some(()) => {
          let Ok(position) = fitted::<_, u32>(songs.len());
          slots.push(Some(position));
          songs.push(song);
        }
        None => slots.push(None),
      }
    }

    match { ::core::primitive::bool::then_some(songs.is_empty(), ()) } {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let mut seen = std::collections::BTreeSet::new();
    let mut order = Vec::new();

    for old in saved.order {
      let Ok(position) = index(old);
      let new = match slots.get(position) {
        Some(Some(new)) => *new,
        Some(None) | None => continue,
      };

      match seen.insert(new).then_some(()) {
        Some(()) => order.push(new),
        None => {}
      }
    }

    match { ::core::primitive::bool::then_some(order.len() != songs.len(), ()) } {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let mut playlist = Self {
      songs,
      order,
      at: 0,
      ordered: saved.ordered,
      over: saved.over,
    };
    let Ok(at) = playlist.position_of(was);
    playlist.at = at;

    Ok(Some(playlist))
  }

  pub fn song(&self) -> Result<Option<&Path>, Never> {
    let Ok(at) = index(self.at);

    Ok(
      self
        .order
        .get(at)
        .and_then(|held| {
          let Ok(held_2) = index(*held);

          self.songs.get(held_2)
        })
        .map(PathBuf::as_path),
    )
  }

  pub fn ordering(&self) -> Result<Order, Never> {
    Ok(self.ordered)
  }

  pub fn repeating(&self) -> Result<Over, Never> {
    Ok(self.over)
  }

  pub fn onward(&mut self) -> Result<Moved, Never> {
    match ::core::primitive::bool::then_some(self.order.is_empty(), ()) {
      ::core::option::Option::Some(()) => return Ok(Moved::No),
      ::core::option::Option::None => {}
    }

    let Ok(many) = fitted::<_, u32>(self.order.len());
    let last = many.saturating_sub(1);

    match ::core::primitive::bool::then_some(self.at < last, ()) {
      ::core::option::Option::Some(()) => {
        self.at = self.at.saturating_add(1);

        Ok(Moved::Yes)
      }
      ::core::option::Option::None => self.round_to(0),
    }
  }

  pub fn back(&mut self) -> Result<Moved, Never> {
    match ::core::primitive::bool::then_some(self.order.is_empty(), ()) {
      ::core::option::Option::Some(()) => return Ok(Moved::No),
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(self.at > 0, ()) {
      ::core::option::Option::Some(()) => {
        self.at = self.at.saturating_sub(1);

        Ok(Moved::Yes)
      }
      ::core::option::Option::None => {
        let Ok(many) = fitted::<_, u32>(self.order.len());

        self.round_to(many.saturating_sub(1))
      }
    }
  }

  pub fn finished(&mut self) -> Result<Moved, Never> {
    match ::core::primitive::bool::then_some(self.order.is_empty(), ()) {
      ::core::option::Option::Some(()) => return Ok(Moved::No),
      ::core::option::Option::None => {}
    }

    match self.over {
      Over::Again => Ok(Moved::Yes),
      Over::Round | Over::On => self.onward(),
    }
  }

  pub fn shuffling(&mut self, order: Order, seed: u64) -> Result<(), Never> {
    let Ok(playing) = self.song();
    let held = playing.map(Path::to_path_buf);

    let Ok(many) = fitted::<_, u32>(self.songs.len());
    let Ok(ordered) = match order {
      Order::AsListed => listed(many),
      Order::Any => shuffled(many, seed),
    };

    self.order = ordered;
    self.ordered = order;

    let Ok(at) = match held {
      Some(song) => self.position_of(&song),
      None => Ok(0),
    };

    self.at = at;

    Ok(())
  }

  pub fn repeat(&mut self, over: Over) -> Result<(), Never> {
    self.over = over;

    Ok(())
  }

  fn round_to(&mut self, at: u32) -> Result<Moved, Never> {
    match self.over {
      Over::Round | Over::Again => {
        self.at = at;

        Ok(Moved::Yes)
      }
      Over::On => Ok(Moved::No),
    }
  }

  fn position_of(&self, song: &Path) -> Result<u32, Never> {
    let found = (0..).zip(&self.order).find(|(_, held)| {
      let Ok(held_2) = index(**held);

      self.songs.get(held_2).map(PathBuf::as_path) == Some(song)
    });

    Ok(match found {
      Some((at, _)) => at,
      None => THE_FRONT_OF_THE_LIST,
    })
  }
}

fn listed(songs: u32) -> Result<Vec<u32>, Never> {
  Ok((0..songs).collect())
}

fn shuffled(songs: u32, seed: u64) -> Result<Vec<u32>, Never> {
  let Ok(mut order) = listed(songs);
  let Ok(mut rolling) = Rolling::from(seed);

  for at in (1..songs).rev() {
    let Ok(other) = rolling.under(at.saturating_add(1));
    let Ok(here) = index(at);
    let Ok(there) = index(other);

    match order.split_at_mut_checked(here) {
      Some((before, remaining)) => match (before.get_mut(there), remaining.first_mut()) {
        (Some(other_song), Some(this_song)) => std::mem::swap(other_song, this_song),
        (None, _) | (_, None) => {}
      },
      None => {}
    }
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

  fn under(&mut self, bound: u32) -> Result<u32, Never> {
    let Ok(rolled) = self.rolled();
    let held = u64::from(bound);
    let under = match rolled.checked_rem(held) {
      Some(under) => under,
      None => NOTHING_TO_CHOOSE_FROM,
    };

    fitted::<u64, u32>(under)
  }
}
