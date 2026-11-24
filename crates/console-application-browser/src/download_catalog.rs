//! Search indexes over completed download directories. The files and their
//! sidecars are the durable records. A scan constructs a fresh snapshot.
//! Gregorian eras start on March 1, placing leap day at the end of the year.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError};
use std::time::Duration;

use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use console_repository::RelativePath;

use rustix::fs::inotify::{self, CreateFlags, WatchFlags};

use crate::downloads::{self, Id};

#[derive(Debug)]
pub(crate) struct Record {
  pub(crate) id: Id,
  pub(crate) label: String,
  pub(crate) site: Option<String>,
  pub(crate) date: Option<String>,
  saved_at: Option<i64>,
  filename: OsString,
}

impl Record {
  fn read(root: &Path, id: Id) -> io::Result<Option<Self>> {
    let item = {
      let optional_item = downloads::stored(root, &id)?;

      match optional_item {
        Some(matched_item) => matched_item,
        ::core::option::Option::None => return Ok(None),
      }
    };

    let filename = item.filename;
    let mut label = String::new();

    for chunk in filename.as_encoded_bytes().utf8_chunks() {
      label.push_str(chunk.valid());

      match chunk.invalid().first() {
        Some(_) => label.push('�'),
        None => {}
      }
    }

    let display_label = label.replace(char::is_control, "�");

    Ok(Some(Self {
      id,
      label: display_label,
      filename,
      site: item.site,
      date: item.saved_at.and_then(|explicit_argument_0| {
        let Ok(explicit_value) = utc_date(explicit_argument_0);
        explicit_value
      }),
      saved_at: item.saved_at,
    }))
  }

  #[must_use]
  fn search_keys(&self) -> core::result::Result<BTreeSet<String>, console_core_never::Never> {
    let fields = [
      Some(self.label.as_str()),
      self.site.as_deref(),
      self.date.as_deref(),
    ];
    let words = fields.into_iter().flatten().flat_map(|field| {
      field
        .split_whitespace()
        .chain(field.split(|ch: char| !ch.is_alphanumeric()))
    });
    let site_suffixes = self.site.iter().flat_map(|site| {
      site
        .match_indices('.')
        .filter_map(|(at, _)| site.get(at.saturating_add(1)..))
    });
    let Ok(id_text) = self.id.as_str();

    Ok(
      words
        .chain(site_suffixes)
        .chain([id_text])
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect(),
    )
  }
}

#[must_use]
fn utc_date(seconds: i64) -> core::result::Result<Option<String>, console_core_never::Never> {
  let shifted = seconds.div_euclid(86_400).saturating_add(719_468);
  let era = shifted.div_euclid(146_097);
  let day_in_era = shifted.rem_euclid(146_097);
  let year_in_era = day_in_era
    .wrapping_sub(day_in_era.wrapping_div(1_460))
    .wrapping_add(day_in_era.wrapping_div(36_524))
    .wrapping_sub(day_in_era.wrapping_div(146_096))
    .wrapping_div(365);
  let year_start = year_in_era
    .wrapping_mul(365)
    .wrapping_add(year_in_era.wrapping_div(4))
    .wrapping_sub(year_in_era.wrapping_div(100));
  let day_in_year = day_in_era.wrapping_sub(year_start);
  let march_month = day_in_year
    .wrapping_mul(5)
    .wrapping_add(2)
    .wrapping_div(153);
  let month_start = march_month
    .wrapping_mul(153)
    .wrapping_add(2)
    .wrapping_div(5);
  let day = day_in_year.wrapping_sub(month_start).wrapping_add(1);
  let month = march_month.wrapping_add(2).wrapping_rem(12).wrapping_add(1);
  let year = era
    .wrapping_mul(400)
    .wrapping_add(year_in_era)
    .wrapping_add(i64::from(month <= 2));

  Ok(
    (0..=9999)
      .contains(&year)
      .then(|| format!("{year:04}-{month:02}-{day:02}")),
  )
}

pub(crate) struct Catalog {
  pub(crate) root: PathBuf,
  recent: Vec<Record>,
  by_id: HashMap<Id, u64>,
  by_term: BTreeMap<String, Vec<u64>>,
}

#[derive(Default)]
pub(crate) enum Matches {
  #[default]
  All,
  Found(Vec<u64>),
}

impl Catalog {
  fn scan(root: &Path) -> io::Result<Self> {
    let mut recent = Vec::new();

    let dir = fs::read_dir(root)?;

    for entry in dir {
      let item = entry?;
      let name = item.file_name();

      let id = match name.to_str().map(Id::parse) {
        Some(Ok(id)) => id,
        Some(Err(console_browser_downloads::InvalidId)) | None => continue,
      };

      match Record::read(root, id) {
        Ok(Some(record)) => recent.push(record),
        Ok(None) => {}
        Err(fault) => {
          console_core_logging::debug!("reading download {}: {fault}", item.path().display())
        }
      }
    }

    recent.sort_unstable_by(|a, b| b.saved_at.cmp(&a.saved_at).then_with(|| a.id.cmp(&b.id)));
    let mut by_id = HashMap::with_capacity(recent.len());
    let mut by_term = BTreeMap::<String, Vec<u64>>::new();

    for (position, record) in recent.iter().enumerate() {
      let Ok(row) = fixed_index(position);
      by_id.insert(record.id.clone(), row);

      let Ok(search_keys) = record.search_keys();

      for term in search_keys {
        by_term.entry(term).or_default().push(row);
      }
    }

    Ok(Self {
      root: root.to_owned(),
      recent,
      by_id,
      by_term,
    })
  }

  #[must_use]
  pub(crate) fn search(
    &self,
    query: &str,
  ) -> core::result::Result<Matches, console_core_never::Never> {
    let mut terms = query.split_whitespace().map(str::to_lowercase);

    let first = {
      let optional_first = terms.next();

      match optional_first {
        Some(matched_first) => matched_first,
        ::core::option::Option::None => return Ok(Matches::All),
      }
    };

    let Ok(mut found) = self.with_prefix(first);

    for term in terms {
      match { ::core::primitive::bool::then_some(found.is_empty(), ()) } {
        ::core::option::Option::Some(()) => {
          break;
        }
        ::core::option::Option::None => {}
      }

      let Ok(other) = self.with_prefix(term);
      found.retain(|row| other.contains(row));
    }

    Ok(Matches::Found(found.into_iter().collect()))
  }

  #[must_use]
  fn with_prefix(
    &self,
    prefix: String,
  ) -> core::result::Result<BTreeSet<u64>, console_core_never::Never> {
    Ok(
      self
        .by_term
        .range(prefix.clone()..)
        .take_while(|(term, _)| term.starts_with(&prefix))
        .flat_map(|(_, rows)| rows.iter().copied())
        .collect(),
    )
  }

  #[must_use]
  pub(crate) fn count(
    &self,
    matches: &Matches,
  ) -> core::result::Result<u64, console_core_never::Never> {
    fixed_index(match matches {
      Matches::All => self.recent.len(),
      Matches::Found(rows) => rows.len(),
    })
  }

  #[must_use]
  pub(crate) fn row(
    &self,
    matches: &Matches,
    position: u64,
  ) -> core::result::Result<Option<&Record>, console_core_never::Never> {
    let Ok(slot) = index(position);
    Ok(match matches {
      Matches::All => self.recent.get(slot),
      Matches::Found(rows) => rows.get(slot).and_then(|row| {
        let Ok(source) = index(*row);
        self.recent.get(source)
      }),
    })
  }

  #[must_use]
  pub(crate) fn position(
    &self,
    matches: &Matches,
    id: &Id,
  ) -> core::result::Result<Option<u64>, console_core_never::Never> {
    let index = match ::core::option::Option::<&_>::copied(self.by_id.get(id)) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    Ok(match matches {
      Matches::All => Some(index),
      Matches::Found(rows) => match rows.binary_search(&index) {
        Ok(value) => {
          let Ok(position) = fixed_index(value);
          Some(position)
        }
        ::core::result::Result::Err(_insertion_position) => ::core::option::Option::None,
      },
    })
  }

  pub(crate) fn file(&self, id: &Id) -> io::Result<PathBuf> {
    let record = self
      .by_id
      .get(id)
      .and_then(|row| {
        let Ok(slot) = index(*row);
        self.recent.get(slot)
      })
      .ok_or_else(|| {
        io::Error::new(
          io::ErrorKind::NotFound,
          "Download is no longer in the catalog",
        )
      })?;
    let key = RelativePath::new(id.as_ref())
      .map_err(|fault| io::Error::new(io::ErrorKind::InvalidData, fault))?;
    let name = RelativePath::new(Path::new(&record.filename))
      .map_err(|fault| io::Error::new(io::ErrorKind::InvalidData, fault))?;
    let directory = self.root.join(key);
    let folder = directory.symlink_metadata()?;

    match bool::then_some(folder.is_dir(), ()) {
      Some(()) => {}
      None => {
        return Err(io::Error::new(
          io::ErrorKind::NotFound,
          "Download has moved or been removed",
        ));
      }
    }

    let path = directory.join(name);
    let file = path.symlink_metadata()?;

    match bool::then_some(file.is_file(), ()) {
      Some(()) => Ok(path),
      None => Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Download has moved or been removed",
      )),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileAction {
  Reveal,
  Trash,
}

impl FileAction {
  fn apply(self, root: &Path, id: &Id) -> io::Result<()> {
    match self {
      Self::Reveal => downloads::reveal(root, id),
      Self::Trash => downloads::trash(root, id),
    }
  }
}

enum Work {
  Refresh,
  File { id: Id, action: FileAction },
}

pub(crate) enum Update {
  Catalog(io::Result<Catalog>),
  File {
    id: Id,
    action: FileAction,
    result: io::Result<()>,
  },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WatchChange {
  Pending,
  Quiet,
}

pub(crate) struct Scanner {
  queue: SyncSender<Work>,
  results: Receiver<Update>,
  ready: UnixDatagram,
  watch: Option<File>,
}

impl Scanner {
  pub(crate) fn new(root: PathBuf) -> io::Result<Self> {
    let (request, queued) = mpsc::sync_channel(2);
    let (send, results) = mpsc::sync_channel(1);
    let (ready, completed) = UnixDatagram::pair()?;
    ready.set_nonblocking(true)?;
    let watch = match Self::watch(&root) {
      Ok(watch) => Some(watch),
      Err(fault) => {
        console_core_logging::debug!("watching browser downloads: {fault}");
        None
      }
    };

    let worker = std::thread::Builder::new()
      .name("downloads-catalog".to_owned())
      .spawn(move || {
        let Ok(()) = scan_work(&root, queued, send, completed);
      })?;
    let Ok(()) = console_program_lifetime::threads::let_go(worker);

    Ok(Self {
      queue: request,
      results,
      ready,
      watch,
    })
  }

  fn watch(root: &Path) -> io::Result<File> {
    let fd = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK)?;
    inotify::add_watch(
      &fd,
      root,
      WatchFlags::CREATE | WatchFlags::DELETE | WatchFlags::MOVE,
    )?;
    Ok(File::from(fd))
  }

  #[must_use]
  pub(crate) fn descriptors(
    &self,
  ) -> core::result::Result<impl Iterator<Item = BorrowedFd<'_>>, console_core_never::Never> {
    Ok(std::iter::once(self.ready.as_fd()).chain(self.watch.iter().map(AsFd::as_fd)))
  }

  pub(crate) fn refresh(&self) -> io::Result<()> {
    match self.queue.try_send(Work::Refresh) {
      Ok(()) => Ok(()),
      Err(TrySendError::Full(_)) => Err(io::Error::from(io::ErrorKind::WouldBlock)),
      Err(TrySendError::Disconnected(_)) => Err(io::Error::other("The catalog worker stopped")),
    }
  }

  pub(crate) fn file_action(&self, id: Id, action: FileAction) -> io::Result<()> {
    match self.queue.try_send(Work::File { id, action }) {
      Ok(()) => Ok(()),
      Err(TrySendError::Full(_)) => Err(io::Error::from(io::ErrorKind::WouldBlock)),
      Err(TrySendError::Disconnected(_)) => Err(io::Error::other("The catalog worker stopped")),
    }
  }

  pub(crate) fn changed(&mut self) -> Result<WatchChange, Never> {
    let watch = match self.watch.as_ref() {
      Some(watch) => watch,
      None => return Ok(WatchChange::Quiet),
    };

    Ok(match rustix::io::read(watch, &mut [0; 8192]) {
      Ok(0) | Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => WatchChange::Quiet,
      Ok(_events) => WatchChange::Pending,
      Err(fault) => {
        console_core_logging::debug!("reading the downloads folder watch: {fault}");
        self.watch = None;
        WatchChange::Quiet
      }
    })
  }

  #[must_use]
  pub(crate) fn result(&self) -> core::result::Result<Option<Update>, console_core_never::Never> {
    Ok(match self.ready.recv(&mut [0]) {
      Ok(_notification) => match self.results.try_recv() {
        Ok(update) => Some(update),
        Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
      },
      Err(fault) => {
        match ::core::primitive::bool::then_some(fault.kind() == io::ErrorKind::WouldBlock, ()) {
          ::core::option::Option::Some(()) => None,
          ::core::option::Option::None => Some(Update::Catalog(Err(fault))),
        }
      }
    })
  }
}

fn scan_work(
  root: &Path,
  queued: Receiver<Work>,
  send: SyncSender<Update>,
  completed: UnixDatagram,
) -> Result<(), Never> {
  let finished = iterate((queued, send, completed), |ports| {
    let (requests, results, notices) = &ports;
    let work = match requests.recv_timeout(Duration::from_secs(60)) {
      Ok(work) => work,
      Err(RecvTimeoutError::Timeout) => return Ok(Step::Again(ports)),
      Err(RecvTimeoutError::Disconnected) => return Ok(Step::Halt(())),
    };
    let update = match work {
      Work::Refresh => Update::Catalog(Catalog::scan(root)),
      Work::File { id, action } => {
        let result = action.apply(root, &id);
        Update::File { id, action, result }
      }
    };

    match results.send(update) {
      Ok(()) => {}
      Err(_caller_closed) => return Ok(Step::Halt(())),
    }

    Ok(match notices.send(&[1]) {
      Ok(_sent) => Step::Again(ports),
      Err(_caller_closed) => Step::Halt(()),
    })
  });

  match finished {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("download catalog worker: {fault}"),
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::downloads::Staging;

  const FIRST: &str = "12345678-abcd-4321-bcde-000000000001";
  const SECOND: &str = "12345678-abcd-4321-bcde-000000000002";
  const LEGACY: &str = "12345678-abcd-4321-bcde-000000000003";

  fn fixture(path: impl AsRef<Path>, bytes: &[u8]) -> io::Result<()> {
    console_core_atomic_writes::whole_without_overwriting(path.as_ref(), bytes)
      .map_err(io::Error::other)
  }

  #[test]
  fn dates_use_utc_calendar_days_including_leap_days() {
    for (timestamp, day) in [
      (-1, "1969-12-31"),
      (0, "1970-01-01"),
      (951_782_400, "2000-02-29"),
    ] {
      assert_eq!(
        ({
          let Ok(explicit_value) = utc_date(timestamp);
          explicit_value
        })
        .as_deref(),
        Some(day)
      );
    }
  }

  fn duplicate_reports(root: &Path) -> io::Result<PathBuf> {
    let first = root.join(FIRST);
    let second = root.join(SECOND);
    let legacy = root.join(LEGACY);

    for directory in [&first, &second, &legacy] {
      fs::create_dir(directory)?;
    }

    fixture(first.join("Report.PDF"), b"first")?;
    fixture(second.join("Report.PDF"), b"second")?;
    fixture(legacy.join("older download.txt"), b"legacy")?;
    fixture(
      first.join(".download.json"),
      br#"{"name":"Report.PDF","site":"files.example.org","saved_at":951782400}"#,
    )?;
    fixture(second.join(".download.json"), b"corrupt metadata")?;
    let outside = root.join("external-file");
    fixture(&outside, b"external")?;
    std::os::unix::fs::symlink(&outside, first.join("linked-file"))?;

    Ok(first)
  }

  fn search_count(catalog: &Catalog, term: &str) -> Result<u64, Never> {
    let Ok(found) = catalog.search(term);
    catalog.count(&found)
  }

  #[test]
  fn a_rebuilt_catalog_finds_duplicate_names_and_tolerates_missing_metadata() -> io::Result<()> {
    let temporary = Staging::temporary("download-catalog-test")?;
    let root = temporary.as_ref();
    let first = duplicate_reports(root)?;
    let catalog = Catalog::scan(root)?;
    let Ok(reports) = catalog.search("report pdf");
    let Ok(count) = catalog.count(&reports);
    assert_eq!(count, 2);
    let Ok(match_on_site) = catalog.search("example.org 2000-02 rep");
    let Ok(catalog_row) = catalog.row(&match_on_site, 0);
    let found = catalog_row.ok_or_else(|| io::Error::other("missing indexed download"))?;
    assert_eq!(found.id.as_str(), Ok(FIRST));
    let Ok(catalog_position) = catalog.position(&reports, &found.id);
    let position = catalog_position.ok_or_else(|| io::Error::other("missing report position"))?;
    let Ok(found_row) = catalog.row(&reports, position);
    assert_eq!(found_row.map(|row| &row.id), Some(&found.id));
    let file = catalog.file(&found.id)?;
    let read = fs::read(file)?;
    assert_eq!(read, b"first");
    assert_eq!(search_count(&catalog, "older txt"), Ok(1));
    assert_eq!(search_count(&catalog, "linked-file"), Ok(0));
    assert_eq!(search_count(&catalog, SECOND), Ok(1));

    fs::rename(first.join("Report.PDF"), first.join("Renamed.pdf"))?;
    let stale = catalog.file(&found.id);
    assert!(matches!(stale, Err(fault) if fault.kind() == io::ErrorKind::NotFound));
    let rebuilt = Catalog::scan(root)?;
    assert_eq!(search_count(&rebuilt, "report"), Ok(1));
    assert_eq!(search_count(&rebuilt, "renamed example.org"), Ok(1));
    let rebuilt_file = rebuilt.file(&found.id)?;
    let rebuilt_bytes = fs::read(rebuilt_file)?;
    assert_eq!(rebuilt_bytes, b"first");
    let legacy_id = Id::parse(LEGACY).map_err(io::Error::other)?;
    let locate = downloads::locate(root, &legacy_id)?;
    let legacy_bytes = fs::read(locate)?;
    assert_eq!(legacy_bytes, b"legacy");
    Ok(())
  }

  #[test]
  fn a_click_still_selects_the_drawn_id_after_a_catalog_refresh()
  -> Result<(), Box<dyn std::error::Error>> {
    use console_core_color::palette;
    use console_core_fonts::TextStyle;
    use console_core_geometry::{Point, Size};
    use console_draw_painting::Frame;

    use crate::download_view::{Action, View};

    let temporary = Staging::temporary("download-catalog-click")?;
    let root = temporary.as_ref();
    let first = root.join(FIRST);
    fs::create_dir(&first)?;
    fixture(first.join("report.pdf"), b"first")?;
    fixture(first.join(".download.json"), br#"{"saved_at":1}"#)?;

    let mut view = View::default();
    let Ok(()) = view.open();
    let scan = Catalog::scan(root)?;
    let Ok(()) = view.replace(scan);
    let Ok(shipped) = palette::read(include_str!(
      "../../../files/usr/local/lib/console/palette.sh"
    ));
    let colors = palette::Wearing::out_of(&shipped)?;
    let Ok(font) = TextStyle::Footnote.font();
    const VIEWPORT: Size<u32> = Size {
      width: 640,
      height: 320,
    };
    const PIXEL_BYTES: u32 = VIEWPORT.width * VIEWPORT.height * 4;
    let frame = Frame {
      points: VIEWPORT,
      device: VIEWPORT,
    };
    let Ok(length) = index(PIXEL_BYTES);
    let mut pixels = vec![0; length.min(const { 1024 * 1024 })];
    view.paint(&mut pixels, frame, &colors, &font)?;

    let second = root.join(SECOND);
    fs::create_dir(&second)?;
    fixture(second.join("report.pdf"), b"second")?;
    fixture(second.join(".download.json"), br#"{"saved_at":2}"#)?;
    let catalog_scan = Catalog::scan(root)?;
    let Ok(()) = view.replace(catalog_scan);

    let point = Point { x: 40, y: 140 };
    let menu = Point { x: 604, y: 140 };
    let trash = Point { x: 550, y: 225 };
    let Ok(selected) = view.click(point);
    assert!(matches!(selected, Action::Open(id) if id.as_str() == Ok(FIRST)));
    let Ok(open_menu) = view.click(menu);
    assert!(matches!(open_menu, Action::Handled));
    let Ok(reveal) = view.key(console_input_key_symbols::Keysym::Return, 0);
    assert!(
      matches!(reveal, Action::File { id, action: FileAction::Reveal } if id.as_str() == Ok(FIRST))
    );
    let Ok(reopen_menu) = view.click(menu);
    assert!(matches!(reopen_menu, Action::Handled));
    let Ok(before_draw) = view.click(trash);
    assert!(matches!(before_draw, Action::Handled));
    view.paint(&mut pixels, frame, &colors, &font)?;
    let Ok(removed) = view.click(trash);
    assert!(
      matches!(removed, Action::File { id, action: FileAction::Trash } if id.as_str() == Ok(FIRST))
    );
    let Ok(new_choice) = view.click(point);
    assert!(matches!(new_choice, Action::Open(id) if id.as_str() == Ok(SECOND)));
    Ok(())
  }
}
