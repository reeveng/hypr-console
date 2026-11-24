//! Software Store data and actions for the shared catalogue and dialogs.

use std::cmp::Ordering;
use std::convert::Infallible;
use std::io;

use console_core_color::palette::{self, Wearing};
use console_core_external_programs::Installed;
use console_core_geometry::Size;
use console_core_number_conversion::{checked_index, fitted, toward_zero_u8};
use console_core_shapes::Pixels;
use console_panel::canvas::Interaction;
use console_panel::storefront::{
  self, Catalogue, DetailActions, Dialog, DialogBody, Entry, EntryLayout, Feature, Introduction,
  Link, Message, Search, Sidebar, Topic,
};

use crate::catalogue::{self, Category, Package};
use crate::{
  Action, LaunchStatus, Overlay, Page, ProfileAvailability, ProfileStatus, SearchStatus, Shop,
};

pub type Frame = storefront::Frame<Action>;

const PALETTE: &str = include_str!("../../../files/usr/local/lib/console/palette.sh");
const FEATURE: &[u8] = include_bytes!("../assets/obs-feature.png");

pub struct Theme {
  colors: Wearing,
  art: Pixels,
}

impl Theme {
  pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
    let colors = match Wearing::worn() {
      Ok(colors) => colors,
      Err(fault) => {
        console_core_logging::debug!("store theme: {fault}");
        let Ok(fallback) = palette::read(PALETTE);
        Wearing::out_of(&fallback)?
      }
    };
    let mut artwork = console_core_png_files::decoded(FEATURE)?;

    shade_feature(&mut artwork.bytes, artwork.size)?;

    let Ok(art) = artwork.into_strided();
    Ok(Self { colors, art })
  }
}

fn shade_feature(bytes: &mut [u8], size: Size<u32>) -> io::Result<()> {
  let stride = checked_index(size.width).map_err(io::Error::other)?;
  let (mut pixels, tail) = bytes.as_chunks_mut::<4>();

  match bool::then_some(size.width > 0 && tail.is_empty(), ()) {
    Some(()) => {}
    None => {
      return Err(io::Error::other(
        "feature image has an incomplete pixel or no columns",
      ));
    }
  }

  let columns = f64::from(size.width);
  let brightness = |x| 0.68 + f64::from(x) / columns;
  let shaded = match (0..size.width).find(|x| brightness(*x) >= 1.0) {
    Some(full_brightness) => full_brightness,
    None => size.width,
  };

  for _row in 0..size.height {
    let (row_pixels, rest) = pixels
      .split_at_mut_checked(stride)
      .ok_or_else(|| io::Error::other("feature image has an incomplete row"))?;
    pixels = rest;

    for (x, [red, green, blue, _alpha]) in (0..shaded).zip(row_pixels) {
      let light = brightness(x);

      for channel in [red, green, blue] {
        let Ok(shaded_channel) = toward_zero_u8(f64::from(*channel) * light);
        *channel = shaded_channel;
      }
    }
  }

  Ok(())
}

const PAGES: [Link<'static, Action>; 2] = [
  Link {
    label: "Browse",
    action: Action::Page(Page::Browse),
  },
  Link {
    label: "Installed",
    action: Action::Page(Page::Installed),
  },
];

const HELP: [Topic<'static>; 3] = [
  Topic {
    title: "Find an app",
    detail: "Search by name or what you want to do. Select a result to see its details.",
  },
  Topic {
    title: "Install and open",
    detail: "Select Install, then look under Installed to open or remove the app.",
  },
  Topic {
    title: "Keyboard",
    detail: "Ctrl+K searches. Tab moves between controls. Enter activates. Esc goes back.",
  },
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum PageView {
  Browse,
  Installed,
  Search,
}

impl PageView {
  fn current(shop: &Shop) -> Result<Self, Infallible> {
    let Ok(query) = shop.query();
    Ok(match query.trim() {
      "" => match shop.page {
        Page::Browse => Self::Browse,
        Page::Installed => Self::Installed,
      },
      _ => Self::Search,
    })
  }
}

#[must_use]
pub fn draw(shop: &Shop, room: Size<u32>, theme: &Theme) -> Result<Frame, Infallible> {
  let interaction = Interaction {
    focus: shop.focus,
    hover: shop.hover,
  };

  match &shop.overlay {
    Some(overlay) => draw_dialog(shop, room, theme.colors, interaction, overlay),
    None => draw_catalogue(shop, room, theme, interaction),
  }
}

fn draw_catalogue(
  shop: &Shop,
  room: Size<u32>,
  theme: &Theme,
  interaction: Interaction<Action>,
) -> Result<Frame, Infallible> {
  let Ok(page) = PageView::current(shop);
  let Ok(rows) = shop.rows();
  let Ok((title, subtitle)) = heading(shop, page, &rows);
  let Ok(search) = search(shop);
  let Ok(filters) = filters();
  let Ok(obs) = catalogue::package("obs-studio");
  let Ok(introduction) = introduction(shop, page, &rows, &filters, &obs, &theme.art);
  let Ok(notice) = store_notice(shop);
  let Ok(problem) = search_notice(shop, page);
  let advisory = (shop.nix_available == Installed::No).then_some(Message {
    title: "Nix is needed to install packages",
    detail: Some("Install Nix and reopen the store to search the full catalogue."),
  });
  let sidebar = Sidebar {
    title: "Software Store",
    narrow_title: "Software\nStore",
    mark: "◇",
    pages: &PAGES,
    selected: (page != PageView::Search).then_some(Action::Page(shop.page)),
    footer: Link {
      label: "Help",
      action: Action::Help,
    },
  };
  let layout = match page {
    PageView::Browse => EntryLayout::Tiles,
    PageView::Installed | PageView::Search => EntryLayout::List,
  };
  let view = Catalogue {
    sidebar,
    search,
    close: Link {
      label: "×",
      action: Action::Close,
    },
    notice,
    title,
    subtitle: &subtitle,
    advisory,
    problem,
    introduction,
    layout,
    scroll: shop.scroll,
  };
  let show_banner = page == PageView::Browse && shop.category == Category::All;
  let entries = rows
    .iter()
    .enumerate()
    .filter(|(_, item)| !show_banner || item.installable.attribute != "obs-studio");
  let Ok(frame) = storefront::catalogue(
    &view,
    room,
    theme.colors,
    interaction,
    entries,
    |(index, item)| {
      let Ok(position) = fitted::<_, u32>(index);
      let Ok(entry) = entry(shop, item, Action::Details(position));
      entry
    },
  );
  Ok(frame)
}

fn search(shop: &Shop) -> Result<Search<'_, Action>, Infallible> {
  let Ok(text) = shop.query();
  let Ok(before) = shop.query.before_caret();
  let after = match text.get(before.len()..) {
    Some(after) => after,
    None => "",
  };
  Ok(Search {
    edit: Action::Search,
    clear: Link {
      label: "Clear",
      action: Action::ClearSearch,
    },
    shortcut: "Ctrl+K",
    placeholder: "Search for an app or package",
    before_caret: before,
    after_caret: after,
    preedit: &shop.preedit,
  })
}

fn store_notice(shop: &Shop) -> Result<Option<storefront::Notice<'_, Action>>, Infallible> {
  Ok(shop.notice.as_ref().map(|notice| storefront::Notice {
    message: Message {
      title: &notice.title,
      detail: notice.detail.as_deref(),
    },
    details: notice.error.as_ref().map(|_| Link {
      label: "Details",
      action: Action::ErrorDetails,
    }),
    reply: (shop.profile_status != ProfileStatus::Changing).then_some(Link {
      label: "Close",
      action: Action::Dismiss,
    }),
  }))
}

fn search_notice(
  shop: &Shop,
  page: PageView,
) -> Result<Option<storefront::Notice<'_, Action>>, Infallible> {
  Ok(
    shop
      .search_problem
      .as_ref()
      .filter(|_| page == PageView::Search)
      .map(|notice| storefront::Notice {
        message: Message {
          title: &notice.title,
          detail: notice.detail.as_deref(),
        },
        details: notice.error.as_ref().map(|_| Link {
          label: "Details",
          action: Action::SearchDetails,
        }),
        reply: Some(Link {
          label: "Retry",
          action: Action::RetrySearch,
        }),
      }),
  )
}

fn heading(
  shop: &Shop,
  page: PageView,
  rows: &[Package],
) -> Result<(&'static str, String), Infallible> {
  Ok(match page {
    PageView::Browse => (
      "Browse apps",
      "Discover desktop apps and command-line tools from Nixpkgs.".to_owned(),
    ),
    PageView::Installed => (
      "Installed",
      "Apps and tools you installed with the store.".to_owned(),
    ),
    PageView::Search => {
      let Ok(subtitle) = search_subtitle(shop, rows);
      ("Search results", subtitle)
    }
  })
}

fn search_subtitle(shop: &Shop, rows: &[Package]) -> Result<String, Infallible> {
  let Ok(status) = shop.searching();

  match status {
    SearchStatus::Searching => return Ok("Searching Nixpkgs…".to_owned()),
    SearchStatus::Settled => {}
  }

  let Ok(visible) = fitted::<_, u32>(rows.len());
  let total = shop.match_count;
  Ok(match total.cmp(&visible) {
    Ordering::Greater => format!(
      "Showing {visible} of {total} packages. Use a narrower search for more specific results."
    ),
    Ordering::Equal | Ordering::Less => match (total, rows) {
      (0, []) => "Search by name or by what you want to do.".to_owned(),
      (0, _) => "Featured matches".to_owned(),
      _ => format!("{total} packages found"),
    },
  })
}

type Filters = [(Link<'static, Action>, i32); 5];

fn filters() -> Result<Filters, Infallible> {
  let [all, media, creative, everyday, developer] = Category::EVERY.map(|category| {
    let Ok(label) = category.title();
    Link {
      label,
      action: Action::Category(category),
    }
  });
  Ok([
    (all, 77),
    (media, 73),
    (creative, 91),
    (everyday, 92),
    (developer, 102),
  ])
}

fn introduction<'a>(
  shop: &Shop,
  page: PageView,
  rows: &[Package],
  filters: &'a Filters,
  obs: &'a Package,
  art: &'a Pixels,
) -> Result<Introduction<'a, Action>, Infallible> {
  match (page, rows) {
    (PageView::Browse, _) => {
      let banner = match shop.category {
        Category::All => {
          let Ok(local) = shop.local(obs);
          let label = match local {
            Some(_) => "View installed app",
            None => "View OBS Studio  →",
          };
          Some(Feature {
            art,
            eyebrow: "Screen recording & streaming",
            title: &obs.title,
            description: &obs.description,
            link: Link {
              label,
              action: Action::Featured,
            },
          })
        }
        Category::Media | Category::Creative | Category::Everyday | Category::Developer => None,
      };
      Ok(Introduction::Featured {
        banner,
        title: "Featured apps",
        filters,
        selected: Action::Category(shop.category),
      })
    }
    (PageView::Search | PageView::Installed, []) => empty(shop, page),
    (PageView::Search | PageView::Installed, _) => Ok(Introduction::None),
  }
}

fn empty(shop: &Shop, page: PageView) -> Result<Introduction<'static, Action>, Infallible> {
  let Ok(search) = shop.searching();
  let Ok(profile) = shop.availability();
  let (title, detail, link) = match (page, search, shop.nix_available, &shop.search_problem) {
    (PageView::Search, SearchStatus::Searching, _, _) => (
      "Looking for packages",
      "The first search can take a little longer.",
      None,
    ),
    (PageView::Search, SearchStatus::Settled, Installed::No, _)
    | (PageView::Search, SearchStatus::Settled, Installed::Yes, Some(_)) => (
      "No featured matches",
      "Try a different name or browse the featured apps.",
      None,
    ),
    (PageView::Search, SearchStatus::Settled, Installed::Yes, None) => (
      "No packages found",
      "Try a shorter name or a word like video or paint.",
      None,
    ),
    (PageView::Installed | PageView::Browse, _, _, _) => {
      match (shop.profile_status, shop.nix_available, profile) {
        (ProfileStatus::Reading(_), _, _) => (
          "Reading installed packages",
          "the store is loading your library.",
          None,
        ),
        (_, Installed::Yes, ProfileAvailability::Unavailable) => (
          "Installed packages are unavailable",
          "Refresh the list after checking Nix.",
          Some(Link {
            label: "Refresh",
            action: Action::Refresh,
          }),
        ),
        _ => (
          "Your shelf is empty",
          "Choose an app from Browse or Search to install it.",
          Some(Link {
            label: "Browse",
            action: Action::Page(Page::Browse),
          }),
        ),
      }
    }
  };
  Ok(Introduction::Empty {
    message: Message {
      title,
      detail: Some(detail),
    },
    link,
  })
}

fn entry<'a>(
  shop: &Shop,
  item: &'a Package,
  action: Action,
) -> Result<Entry<'a, Action>, Infallible> {
  let Ok(local) = shop.local(item);
  let Ok(icon) = item.category.mark();
  let indicator = match local {
    Some(_) => "✓",
    None => "→",
  };
  let detail = match item.description.as_str() {
    "" => &item.installable.attribute,
    description => description,
  };
  Ok(Entry {
    title: &item.title,
    detail,
    icon,
    indicator,
    action,
  })
}

fn draw_dialog(
  shop: &Shop,
  room: Size<u32>,
  colors: Wearing,
  interaction: Interaction<Action>,
  overlay: &Overlay,
) -> Result<Frame, Infallible> {
  let text;
  let body = match overlay {
    Overlay::Package(item) => {
      let subtitle = match item.version.as_str() {
        "" => item.installable.attribute.as_str(),
        version => {
          text = format!("{}  ·  Version {version}", item.installable.attribute);
          &text
        }
      };
      let Ok(actions) = package_actions(shop, item);
      DialogBody::Details {
        title: &item.title,
        subtitle,
        description: &item.description,
        actions,
      }
    }
    Overlay::Remove(item) => {
      text = format!("Remove {}?", item.package.title);
      DialogBody::Confirmation {
        title: &text,
        description: "You can install this package again from Browse or Search.",
        confirm: Link {
          label: "Remove package",
          action: Action::ConfirmRemove,
        },
        cancel: Link {
          label: "Cancel",
          action: Action::Back,
        },
      }
    }
    Overlay::Error {
      summary,
      text: report,
    } => DialogBody::Report {
      title: summary,
      report,
    },
    Overlay::Help => DialogBody::Help {
      title: "Using the store",
      topics: &HELP,
    },
  };
  let view = Dialog {
    back: Link {
      label: "Back",
      action: Action::Back,
    },
    body,
  };
  let Ok(frame) = storefront::dialog(&view, room, colors, interaction);
  Ok(frame)
}

fn package_actions(
  shop: &Shop,
  item: &Package,
) -> Result<DetailActions<'static, Action>, Infallible> {
  let Ok(installed) = shop.local(item);
  let Ok(availability) = shop.can_change();

  Ok(match (installed, availability) {
    (Some(local), permission) => {
      let Ok(launch) = local.launch_label();
      let primary = launch
        .filter(|_| shop.launching == LaunchStatus::Idle)
        .map(|label| Link {
          label,
          action: Action::Open,
        });
      let secondary = (permission == ProfileAvailability::Ready).then_some(Link {
        label: "Remove",
        action: Action::Remove,
      });
      DetailActions::PrimaryOrStatus {
        primary,
        status: "Installed",
        secondary,
      }
    }
    (None, ProfileAvailability::Ready) => DetailActions::WithHint {
      primary: Link {
        label: "Install",
        action: Action::Install,
      },
      hint: "Installs for your user",
    },
    (None, ProfileAvailability::Unavailable) => {
      DetailActions::Message(match shop.nix_available {
        Installed::Yes => {
          "Install is unavailable while the store is busy or the profile needs attention."
        }
        Installed::No => "Install Nix, then reopen the store.",
      })
    }
  })
}
