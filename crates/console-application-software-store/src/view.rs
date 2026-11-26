//! Software Store data and actions for the shared catalogue and dialogs.

use std::cmp::Ordering;
use std::convert::Infallible;

use console_core_color::palette::Wearing;
use console_core_external_programs::Installed;
use console_core_geometry::Size;
use console_core_number_conversion::fitted;
use console_panel::storefront::{
  self, Catalogue, DetailActions, Dialog, DialogBody, Entry, Introduction, Link, Message, Search,
};
use console_panel::ui::canvas::Interaction;
use console_panel::ui::theme;

use crate::catalogue::{Category, Package, Source};
use crate::{
  Action, LaunchStatus, Overlay, Page, ProfileAvailability, ProfileStatus, SearchStatus, Shop,
};

pub type Frame = storefront::Frame<Action>;

pub struct Theme {
  colors: Wearing,
}

impl Theme {
  pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
    let colors = theme::current()?;
    Ok(Self { colors })
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PageView {
  Browse,
  Installed,
  Search,
}

impl PageView {
  pub(super) fn current(shop: &Shop) -> Result<Self, Infallible> {
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
  let Ok(hover) = shop.hover.target();
  let interaction = Interaction {
    focus: shop.focus,
    hover: hover.map(|(_, action)| action),
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
  let details: Vec<_> = rows
    .iter()
    .map(|item| {
      let Ok(source) = item.source.label();
      let Ok(name) = item.source.name();
      let description = match item.description.as_str() {
        "" => name,
        description => description,
      };

      match item.version.as_str() {
        "" => format!("{source} · {description}"),
        version => format!("{source} · {version} · {description}"),
      }
    })
    .collect();
  let status = match page {
    PageView::Search => {
      let Ok(status) = search_subtitle(shop, &rows);
      status
    }
    PageView::Browse | PageView::Installed => String::new(),
  };
  let Ok(search) = search(shop);
  let Ok(filters) = filters();
  let Ok(introduction) = introduction(shop, page, &rows, &filters);
  let Ok(problem) = search_notice(shop, page);
  let advisory = (shop.nix_available == Installed::No).then_some(Message {
    title: "Nixpkgs search is unavailable",
    detail: Some("Install Nix to enable the Nixpkgs catalogue alongside your other sources."),
  });
  let Ok(selected) = shop.page.index();
  let Ok(scroll) = shop.scroll_position();
  let view = Catalogue {
    header: console_panel::strip::Header {
      title: "Console Store",
      tabs: &["Browse", "Installed"],
      selected,
      select: Action::SelectTab,
      previous: Action::PreviousTab,
      next: Action::NextTab,
      close: Action::Close,
    },
    search,
    notice: None,
    status: &status,
    advisory,
    problem,
    introduction,
    scroll,
  };
  let Ok(frame) = storefront::catalogue(
    &view,
    room,
    theme.colors,
    interaction,
    rows.iter().zip(&details).enumerate(),
    |(index, (item, detail))| {
      let Ok(position) = fitted::<_, u32>(index);
      let Ok(entry) = entry(shop, item, detail, Action::Details(position));
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
    clear: Action::ClearSearch,
    shortcut: "/",
    placeholder: "Search for an app or package",
    before_caret: before,
    after_caret: after,
    preedit: shop.preedit.as_ref(),
  })
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
        retry: Some(Action::RetrySearch),
      }),
  )
}

fn search_subtitle(shop: &Shop, rows: &[Package]) -> Result<String, Infallible> {
  let Ok(status) = shop.searching();

  match status {
    SearchStatus::Searching => return Ok("Searching package sources…".to_owned()),
    SearchStatus::Settled => {}
  }

  match (&shop.search_problem, rows) {
    (Some(_), []) => return Ok(String::new()),
    (Some(_), [_, ..]) => return Ok("Showing results from available sources.".to_owned()),
    (None, _) => {}
  }

  let Ok(visible) = fitted::<_, u32>(rows.len());
  let Ok(total) = shop.match_count();
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
) -> Result<Introduction<'a, Action>, Infallible> {
  match (page, rows) {
    (PageView::Browse, _) => Ok(Introduction::Featured {
      title: "Featured apps",
      filters,
      selected: Action::Category(shop.category),
    }),
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
    (PageView::Search, SearchStatus::Settled, _, Some(_)) => {
      return Ok(Introduction::None);
    }
    (PageView::Search, SearchStatus::Settled, _, None) => (
      "No packages found",
      "Try a shorter name or a word like video or paint.",
      None,
    ),
    (PageView::Installed | PageView::Browse, _, _, _) => {
      match (shop.profile_status, shop.nix_available, profile) {
        (ProfileStatus::Reading(_), _, _) => (
          "Reading installed packages",
          "The store is loading your library.",
          None,
        ),
        (_, Installed::Yes, ProfileAvailability::Unavailable) => (
          "Installed packages are unavailable",
          "Try refreshing the list.",
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
  detail: &'a str,
  action: Action,
) -> Result<Entry<'a, Action>, Infallible> {
  let Ok(local) = shop.local(item);
  let Ok(category) = crate::catalogue::category(item);
  let Ok(icon) = category.mark();
  let indicator = match local {
    Some(_) => "✓",
    None => console_panel::marks::INTO,
  };
  Ok(Entry {
    title: &item.title,
    detail,
    icon,
    art: None,
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
      let Ok(source) = item.source.label();
      let Ok(name) = item.source.name();
      text = match item.version.as_str() {
        "" => format!("{source} · {name}"),
        version => format!("{source} · {name} · {version}"),
      };
      let Ok(actions) = package_actions(shop, item);
      DialogBody::Details {
        title: &item.title,
        subtitle: &text,
        description: &item.description,
        actions,
      }
    }
    Overlay::Remove(item) => {
      text = format!("Remove {}?", item.package.title);
      DialogBody::Confirmation {
        title: &text,
        description: match &item.package.source {
          Source::Pacman(_) | Source::Aur(_) => {
            "Removes this package from the system. Administrator approval is required."
          }
          Source::Mise { .. } => {
            "Removes this installed version. Any project using it may need to install it again."
          }
          Source::Flatpak(_) => "Removes this app from the selected Flatpak installation.",
          Source::Nix(_) => "Removes this package from your Console Store profile.",
        },
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
  };
  let view = Dialog {
    back: Action::Back,
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
  let Ok(availability) = shop.can_change(&item.source);

  Ok(match (installed, availability) {
    (Some(local), permission) => {
      let Ok(launch) = local.launch_label();
      let primary = launch
        .filter(|_| shop.launching == LaunchStatus::Idle)
        .map(|label| Link {
          label,
          action: Action::Open,
        });
      let Ok(removable) = local.removable();
      let secondary = (permission == ProfileAvailability::Ready
        && removable == crate::packages::Removal::Optional)
        .then_some(Link {
          label: "Remove",
          action: Action::Remove,
        });
      DetailActions::PrimaryOrStatus {
        primary,
        status: match removable {
          crate::packages::Removal::Optional => "Installed",
          crate::packages::Removal::Required => "Installed · Required by Console",
        },
        secondary,
      }
    }
    (None, ProfileAvailability::Ready) => DetailActions::WithHint {
      primary: Link {
        label: "Install",
        action: Action::Install,
      },
      hint: match &item.source {
        Source::Nix(_) => "Installs for your user",
        Source::Pacman(_) => "Installs on the system with pacman",
        Source::Aur(_) => "Opens a terminal to review and install with yay",
        Source::Flatpak(_) => "Installs through the selected Flatpak remote",
        Source::Mise { .. } => "Installs the latest version with mise",
      },
    },
    (None, ProfileAvailability::Unavailable) => {
      DetailActions::Message(match (&item.source, shop.nix_available) {
        (Source::Nix(_), Installed::No) => "Install Nix to use this catalogue.",
        _ => "Install is unavailable while the store is busy or this source needs attention.",
      })
    }
  })
}
