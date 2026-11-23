//! The default applications on the Configuration tab.

use console_core_never::Never;
use console_default_applications::editors::{self, Application, Support};
use console_panel::page::{Aside, Handler, NOW, Row, Showing, YET};

use crate::rows::configuration;

pub struct Kind {
  pub says: &'static str,
  pub mime: &'static str,
  pub and: &'static [&'static str],
  pub editor: Option<editors::Kind>,
}

impl Kind {
  pub fn every(&self) -> Result<impl Iterator<Item = &'static str> + use<>, Never> {
    Ok(std::iter::once(self.mime).chain(self.and.iter().copied()))
  }

  pub fn support(&self, app: &Application) -> Result<Support, Never> {
    let Ok(support) = match self.editor {
      Some(kind) => app.edits(kind, self.mime),
      None => app.opens(self.mime),
    };

    Ok(support)
  }
}

const PICTURES: Kind = Kind {
  says: "View Pictures",
  mime: "image/png",
  and: &[
    "image/jpeg",
    "image/webp",
    "image/gif",
    "image/avif",
    "image/heif",
    "image/tiff",
  ],
  editor: None,
};

const WEB_PAGES: Kind = Kind {
  says: "View Web Pages",
  mime: "text/html",
  and: &["application/xhtml+xml"],
  editor: None,
};

const TEXT: Kind = Kind {
  says: "View Plain Text",
  mime: "text/plain",
  and: &[],
  editor: None,
};

pub const KINDS: [Kind; 10] = [
  Kind {
    says: "Links",
    mime: "x-scheme-handler/https",
    and: &["x-scheme-handler/http"],
    editor: None,
  },
  PICTURES,
  Kind {
    says: "Edit Pictures",
    editor: Some(editors::Kind::Pictures),
    ..PICTURES
  },
  WEB_PAGES,
  Kind {
    says: "Edit Web Pages",
    editor: Some(editors::Kind::WebPages),
    ..WEB_PAGES
  },
  TEXT,
  Kind {
    says: "Edit Text and Code",
    editor: Some(editors::Kind::Text),
    ..TEXT
  },
  Kind {
    says: "Video",
    mime: "video/mp4",
    and: &[
      "video/matroska",
      "video/x-matroska",
      "video/webm",
      "video/quicktime",
      "video/vnd.avi",
      "video/ogg",
    ],
    editor: None,
  },
  Kind {
    says: "Music",
    mime: "audio/mpeg",
    and: &[
      "audio/flac",
      "audio/ogg",
      "audio/x-opus+ogg",
      "audio/x-vorbis+ogg",
      "audio/x-flac+ogg",
      "audio/mp4",
      "audio/aac",
      "audio/vnd.wave",
    ],
    editor: None,
  },
  Kind {
    says: "Folders",
    mime: "inode/directory",
    and: &[],
    editor: None,
  },
];

pub fn defaults_rows(
  applications: &[Application],
  now: &dyn Fn(&Kind) -> Result<String, Never>,
  open: impl Fn(u32) -> Result<Handler, Never>,
) -> Result<Vec<Row>, Never> {
  Ok(
    KINDS
      .iter()
      .enumerate()
      .map(|(at, kind)| {
        let Ok(opening) = now(kind);
        let Ok(said) = in_effect(applications, &opening);
        let Ok(at_2) = console_core_number_conversion::fitted(at);
        let Ok(opens) = open(at_2);
        let Ok(row) = Row::new(kind.says, Aside(&said), opens);
        let Ok(opens_2) = row.opening();

        opens_2
      })
      .collect(),
  )
}

pub fn meanwhile_rows(open: impl Fn(u32) -> Result<Handler, Never>) -> Result<Vec<Row>, Never> {
  Ok(
    KINDS
      .iter()
      .enumerate()
      .map(|(at, kind)| {
        let Ok(at_2) = console_core_number_conversion::fitted(at);
        let Ok(opens) = open(at_2);
        let Ok(row) = Row::new(kind.says, Aside(YET), opens);
        let Ok(opens_2) = row.opening();

        opens_2
      })
      .collect(),
  )
}

fn in_effect(applications: &[Application], id: &str) -> Result<String, Never> {
  let found = applications
    .iter()
    .find(|application| application.id == id)
    .map(|one| one.says.clone());

  Ok(match found {
    Some(says) => says,
    None => String::new(),
  })
}

pub fn choice_rows(
  kind: &'static Kind,
  applications: &[Application],
  now: &dyn Fn(&Kind) -> Result<String, Never>,
  back: impl Fn(&dyn Showing) + Send + Sync + 'static,
  use_: impl Fn(&Kind, &Application) -> Result<Handler, Never>,
) -> Result<Vec<Row>, Never> {
  let Ok(configuration) = configuration();
  let Ok(way_back) = Row::back(&configuration, back);
  let Ok(naming) = Row::naming(kind.says, Aside(""));
  let mut rows = vec![way_back, naming];
  let Ok(default) = now(kind);
  let mut opening: Vec<&Application> = applications
    .iter()
    .filter(|application| kind.support(application) == Ok(Support::Yes))
    .collect();
  opening.sort_by_key(|application| application.says.to_lowercase());

  match ::core::primitive::bool::then_some(opening.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(row) = Row::placeholder(match kind.editor {
        Some(_) => "No editor installed for these files",
        None => "Nothing here opens these",
      });

      rows.push(row);

      return Ok(rows);
    }
    ::core::option::Option::None => {}
  }

  for application in opening {
    let Ok(uses) = use_(kind, application);
    let Ok(row) = Row::new(
      &application.says,
      Aside(
        match ::core::primitive::bool::then_some(application.id == default, ()) {
          ::core::option::Option::Some(()) => NOW,
          ::core::option::Option::None => "",
        },
      ),
      uses,
    );

    rows.push(row);
  }

  Ok(rows)
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use super::*;
  use console_panel::page::{Active, Heading};

  const A_ROW: &str = "a row the list was to have";

  fn nothing(_: &Kind, _: &Application) -> Result<Handler, Never> {
    Handler::and_stay(|_| ())
  }

  fn opens(_: u32) -> Result<Handler, Never> {
    Handler::and_stay(|_| ())
  }

  fn applications() -> Result<Vec<Application>, Never> {
    Ok(
      [
        (
          "librewolf.desktop",
          "Name=LibreWolf\nMimeType=x-scheme-handler/https;image/png;text/html;\n",
        ),
        (
          "chromium.desktop",
          "Name=Chromium\nMimeType=x-scheme-handler/https;\n",
        ),
        (
          "org.kde.kate.desktop",
          "Name=Kate\nCategories=TextEditor;\nMimeType=text/plain;\nExec=kate %U\n",
        ),
      ]
      .into_iter()
      .filter_map(|(id, fields)| {
        let held = format!("[Desktop Entry]\nType=Application\n{fields}");
        let Ok(app) = Application::read(Path::new(id), &held);

        app
      })
      .collect(),
    )
  }

  fn choices(kind: &'static Kind, set: &str) -> Result<Vec<Row>, Never> {
    let Ok(applications) = applications();

    choice_rows(
      kind,
      &applications,
      &|_| Ok(set.to_string()),
      |_| (),
      nothing,
    )
  }

  const LINKS_FIRST: &str = "the kinds start with links";

  fn links(set: &str) -> Result<Option<Vec<Row>>, Never> {
    Ok(KINDS.first().map(|links| {
      let Ok(rows) = choices(links, set);

      rows
    }))
  }

  fn defaults(now: &dyn Fn(&Kind) -> Result<String, Never>) -> Result<Vec<Row>, Never> {
    let Ok(applications) = applications();

    defaults_rows(&applications, now, opens)
  }

  fn says(rows: &[Row]) -> Result<Vec<&str>, Never> {
    Ok(rows.iter().map(|row| row.says.as_str()).collect())
  }

  #[test]
  fn a_desktop_file_gives_its_name_and_what_it_opens() -> Result<(), &'static str> {
    let Ok(read) = Application::read(
      Path::new("librewolf.desktop"),
      "[Desktop Entry]\nType=Application\nName=LibreWolf\n\
                 MimeType=text/html;image/png;\n",
    );
    let read_2 = read.ok_or("an application")?;

    assert_eq!(read_2.says, "LibreWolf");
    assert_eq!(read_2.opens("image/png"), Ok(Support::Yes));
    assert_eq!(read_2.opens("video/mp4"), Ok(Support::No));

    Ok(())
  }

  #[test]
  fn only_the_first_group_of_a_desktop_file_is_read() -> Result<(), &'static str> {
    let Ok(read) = Application::read(
      Path::new("librewolf.desktop"),
      "[Desktop Entry]\nType=Application\nName=LibreWolf\n\
             [Desktop Effect new-private-window]\nName=New Private Window\n",
    );
    let read_2 = read.ok_or("an application")?;

    assert_eq!(read_2.says, "LibreWolf");

    Ok(())
  }

  #[test]
  fn a_file_that_asks_not_to_be_shown_is_not_offered() {
    let hidden = "[Desktop Entry]\nType=Application\nName=A helper\nNoDisplay=true\n";

    assert_eq!(
      Application::read(Path::new("helper.desktop"), hidden),
      Ok(None)
    );
  }

  #[test]
  fn a_file_that_is_not_a_program_is_not_offered() {
    assert_eq!(
      Application::read(
        Path::new("a.desktop"),
        "[Desktop Entry]\nType=Link\nName=A site\n"
      ),
      Ok(None)
    );
    assert_eq!(
      Application::read(Path::new("b.desktop"), "[Desktop Entry]\nName=Untitled\n"),
      Ok(None)
    );
    assert_eq!(
      Application::read(
        Path::new("c.desktop"),
        "[Desktop Entry]\nType=Application\nName=\n"
      ),
      Ok(None)
    );
  }

  #[test]
  fn the_program_in_effect_is_the_one_marked() -> Result<(), &'static str> {
    let Ok(rows) = links("chromium.desktop");
    let rows_2 = rows.ok_or(LINKS_FIRST)?;
    let chromium = rows_2
      .iter()
      .find(|row| row.says == "Chromium")
      .ok_or(A_ROW)?;
    let librewolf = rows_2
      .iter()
      .find(|row| row.says == "LibreWolf")
      .ok_or(A_ROW)?;

    assert_eq!(chromium.now(), Ok(Active::Yes));
    assert_eq!(librewolf.now(), Ok(Active::No));

    Ok(())
  }

  #[test]
  fn the_console_browser_can_be_chosen_for_links_and_web_pages() -> Result<(), &'static str> {
    let Ok(browser) = Application::read(
      Path::new("console-browser-window.desktop"),
      include_str!("../../../files/usr/share/applications/console-browser-window.desktop"),
    );
    let browser_2 = browser.ok_or("missing Console Browser desktop entry")?;
    let links = KINDS.first().ok_or(LINKS_FIRST)?;

    for kind in [links, &WEB_PAGES] {
      assert_eq!(kind.support(&browser_2), Ok(Support::Yes));
      let Ok(types) = kind.every();

      for mime in types {
        assert_eq!(browser_2.opens(mime), Ok(Support::Yes), "{mime}");
      }
    }

    let Ok(schemes) = links.every();
    assert_eq!(
      schemes.collect::<Vec<_>>(),
      ["x-scheme-handler/https", "x-scheme-handler/http"]
    );
    Ok(())
  }

  #[test]
  fn every_kind_is_a_row_of_the_tab_in_the_order_it_is_written_down() {
    let Ok(rows) = defaults(&|_| Ok(String::new()));

    assert_eq!(
      says(&rows),
      Ok(vec![
        "Links",
        "View Pictures",
        "Edit Pictures",
        "View Web Pages",
        "Edit Web Pages",
        "View Plain Text",
        "Edit Text and Code",
        "Video",
        "Music",
        "Folders"
      ])
    );
  }

  #[test]
  fn html_has_separate_viewer_and_editor_choices() -> Result<(), &'static str> {
    let view = KINDS
      .iter()
      .find(|kind| kind.says == "View Web Pages")
      .ok_or(A_ROW)?;
    let edit = KINDS
      .iter()
      .find(|kind| kind.says == "Edit Web Pages")
      .ok_or(A_ROW)?;
    let Ok(viewing) = choices(view, "librewolf.desktop");
    let Ok(editing) = choices(edit, "org.kde.kate.desktop");

    assert_eq!(
      viewing.get(2).map(|row| row.says.as_str()),
      Some("LibreWolf")
    );
    assert_eq!(editing.get(2).map(|row| row.says.as_str()), Some("Kate"));
    assert_eq!(viewing.len(), 3);
    assert_eq!(editing.len(), 3);

    Ok(())
  }

  #[test]
  fn every_setting_says_that_it_opens_onto_something() {
    let Ok(rows) = defaults(&|_| Ok(String::new()));

    assert!(
      rows.iter().all(|row| row.opens),
      "a setting that does not say it opens"
    );
  }

  #[test]
  fn a_setting_reads_as_the_name_of_what_it_is_set_to() -> Result<(), &'static str> {
    let Ok(rows) = defaults(&|_| Ok("librewolf.desktop".to_string()));
    let first = rows.first().ok_or(A_ROW)?;

    assert_eq!(first.aside, "LibreWolf");

    Ok(())
  }

  #[test]
  fn a_setting_pointed_at_a_program_that_is_gone_says_nothing() -> Result<(), &'static str> {
    let Ok(rows) = defaults(&|_| Ok("dolphin.desktop".to_string()));
    let first = rows.first().ok_or(A_ROW)?;

    assert_eq!(first.aside, "");

    Ok(())
  }

  #[test]
  fn a_list_under_a_setting_is_the_way_back_and_then_what_it_is_about() -> Result<(), &'static str>
  {
    let Ok(rows) = links("");
    let rows_2 = rows.ok_or(LINKS_FIRST)?;
    let Ok(configuration) = configuration();
    let back = rows_2.first().ok_or(A_ROW)?;
    let named = rows_2.get(1).ok_or(A_ROW)?;

    assert!(
      back.says.ends_with(&configuration),
      "{:?} is not the way back",
      back.says
    );
    assert_eq!(named.says, "Links");
    assert_eq!(
      named.heading(),
      Ok(Heading::Yes),
      "the kind is read rather than chosen"
    );

    Ok(())
  }

  #[test]
  fn a_kind_nothing_opens_says_so_on_its_own_list() -> Result<(), &'static str> {
    let music = KINDS
      .iter()
      .find(|kind| kind.says == "Music")
      .ok_or(A_ROW)?;
    let Ok(rows) = choices(music, "");
    let Ok(said) = says(&rows);

    assert_eq!(
      said.get(1..),
      Some(["Music", "Nothing here opens these"].as_slice())
    );

    Ok(())
  }
}
