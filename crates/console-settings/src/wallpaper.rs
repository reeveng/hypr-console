//! The wallpaper tab: which picture is up, and where a new one comes from.
//!
//! Two things a person wants from a wallpaper and cannot otherwise have here.
//! To stop it changing, because they have found one they like and the weather
//! keeps taking it away. And to add one of their own, which on a machine with
//! no file manager and no terminal means putting a file in a directory and
//! being told what happened to it.
//!
//! Adding is the part worth explaining. `wallpaper-render` does the work, and what
//! it does is not copying: a picture is decoded, brought into this palette, cut to
//! the shape of this screen, and written out as something that rests and then
//! stirs. So the row does not say "copy" and it does not say "import", it says
//! what is actually going to happen to her picture.
//!
//! Reading what is on the machine is one half and knowing what to draw from it
//! is the other, and only the second is here, so the tab can be asked what it
//! would show without a machine to ask.

use std::collections::HashSet;

use console_core_never::Never;
use console_panel::page::{Aside, Handler, NOW, Row};
use console_wallpaper::rules::{Category, Rule};

use crate::rows::Chosen;

pub const SETS: &str = "Wallpaper sets";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
  pub name: String,
  pub says: String,
  pub by: String,
}

impl Offered {
  pub fn of(name: &str) -> Result<Self, Never> {
    let says = name
      .split(['-', '_'])
      .filter(|word| !word.is_empty())
      .map(|word| {
        let mut letters = word.chars();

        match letters.next() {
          Some(first) => first.to_uppercase().chain(letters).collect::<String>(),
          None => String::new(),
        }
      })
      .collect::<Vec<_>>()
      .join(" ");

    Ok(Offered {
      name: name.to_string(),
      says,
      by: String::new(),
    })
  }
}

pub struct Found<'a> {
  pub pictures: &'a [Offered],
  pub scenes: &'a [Offered],
  pub selection: ActiveWallpaper<'a>,
  pub up: &'a str,
  pub dropped: u32,
}

#[derive(Clone, Copy)]
pub enum ActiveWallpaper<'a> {
  Following,
  Still,
  Live(&'a str),
}

pub enum Choice<'a> {
  Picture(&'a str),
  Scene(&'a str),
}

pub fn wallpaper_rows(
  found: &Found<'_>,
  follow: impl Fn(bool) -> Handler,
  show: impl Fn(Choice<'_>) -> Handler,
  sets: Handler,
  take: Handler,
  find: Handler,
) -> Result<Vec<Row>, Never> {
  let &Found {
    pictures,
    scenes,
    selection,
    up,
    dropped,
  } = found;
  let (weather_status, enable_following) = match selection {
    ActiveWallpaper::Following => (NOW, false),
    ActiveWallpaper::Still | ActiveWallpaper::Live(_) => ("", true),
  };
  let Ok(weather) = Row::new(
    "Follow time and weather",
    Aside(weather_status),
    follow(enable_following),
  );
  let Ok(sets_2) = Row::new(SETS, Aside(""), sets);
  let Ok(sets_3) = sets_2.opening();
  let mut rows = vec![weather, sets_3];

  for offered in scenes {
    let aside = match matches!(selection, ActiveWallpaper::Live(name) if name == offered.name)
      .then_some(())
    {
      Some(()) => NOW,
      None => "Live",
    };
    let Ok(row) = Row::new(
      &offered.says,
      Aside(aside),
      show(Choice::Scene(&offered.name)),
    );
    rows.push(row);
  }

  for picture in pictures {
    let Ok(row) = Row::new(
      &picture.says,
      Aside(
        match ::core::primitive::bool::then_some(
          !matches!(selection, ActiveWallpaper::Live(_)) && picture.name == up,
          (),
        ) {
          ::core::option::Option::Some(()) => NOW,
          ::core::option::Option::None => picture.by.as_str(),
        },
      ),
      show(Choice::Picture(&picture.name)),
    );

    rows.push(row);
  }

  match ::core::primitive::bool::then_some(pictures.is_empty() && scenes.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(row) = Row::placeholder("There are no pictures on this machine");

      rows.push(row);
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(dropped > 0, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(row) = match dropped {
        1 => Row::new("Add the picture in Pictures/Wallpapers", Aside(""), take),
        many => Row::new(
          &format!("Add the {many} pictures in Pictures/Wallpapers"),
          Aside(""),
          take,
        ),
      };

      rows.push(row);
    }
    ::core::option::Option::None => {}
  }

  let Ok(finding) = Row::new("Find a picture in the files", Aside(""), find);

  rows.push(finding);

  Ok(rows)
}

#[derive(Debug, Clone, Copy)]
pub enum Edit {
  Automatic,
  Condition(Category, &'static str),
  Reset,
}

#[derive(Clone, Copy)]
pub enum RuleSource {
  Default,
  Customized,
}

pub fn rule_rows(
  picture: &str,
  rule: &Rule,
  source: RuleSource,
  back: impl Chosen,
  edit: impl Fn(Edit) -> Handler,
) -> Result<Vec<Row>, Never> {
  let Ok(back_2) = Row::back(SETS, back);
  let Ok(title) = Row::naming(picture, Aside(""));
  let Ok(automatic) = Row::new(
    "Use in automatic rotation",
    Aside(
      match { ::core::primitive::bool::then_some(rule.automatic, ()) } {
        ::core::option::Option::Some(()) => NOW,
        ::core::option::Option::None => "",
      },
    ),
    edit(Edit::Automatic),
  );
  let mut rows = vec![back_2, title, automatic];

  for category in Category::EVERY {
    let Ok(tags) = rule.tags(category);
    let selected_tags: HashSet<_> = tags
      .iter()
      .map(|entry| entry.trim().to_ascii_lowercase())
      .collect();
    let Ok(category_label) = category.label();
    let Ok(heading) = Row::naming(
      category_label,
      Aside(match tags {
        [] => "Any",
        [_, ..] => "",
      }),
    );
    rows.push(heading);

    let Ok(options) = category.options();

    for tag in options {
      let Ok(label) = Offered::of(tag);
      let name = match { ::core::primitive::bool::then_some(tag == "cloud", ()) } {
        ::core::option::Option::Some(()) => "Cloudy",
        ::core::option::Option::None => &label.says,
      };
      let selected = selected_tags.contains(&tag.to_ascii_lowercase());
      let Ok(row) = Row::new(
        name,
        Aside(match ::core::primitive::bool::then_some(selected, ()) {
          ::core::option::Option::Some(()) => NOW,
          ::core::option::Option::None => "",
        }),
        edit(Edit::Condition(category, tag)),
      );
      rows.push(row);
    }
  }

  match source {
    RuleSource::Customized => {
      let Ok(reset) = Row::new("Restore default rules", Aside(""), edit(Edit::Reset));
      rows.push(reset);
    }
    RuleSource::Default => {}
  }

  Ok(rows)
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_panel::page::{Action, Active};

  const A_ROW: &str = "a row the tab was to have";

  fn nothing() -> Result<Handler, Never> {
    Handler::and_stay(|_| ())
  }

  fn set() -> Result<Vec<Offered>, Never> {
    let Ok(hers) = Offered::of("her-own-photo");

    Ok(vec![
      Offered {
        name: "star-ride".to_string(),
        says: "Star Ride".to_string(),
        by: "Abi Toads".to_string(),
      },
      hers,
    ])
  }

  fn rows_of(found: &Found<'_>) -> Result<Vec<Row>, Never> {
    let Ok(choose) = nothing();
    let Ok(follow) = nothing();

    wallpaper_rows(
      found,
      |_| {
        let Ok(does) = nothing();

        does
      },
      |_| {
        let Ok(does) = nothing();

        does
      },
      choose.clone(),
      choose,
      follow,
    )
  }

  fn following(dropped: u32) -> Result<Vec<Row>, Never> {
    let Ok(set) = set();

    rows_of(&Found {
      pictures: &set,
      scenes: &[],
      selection: ActiveWallpaper::Following,
      up: "star-ride",
      dropped,
    })
  }

  fn pinned(up: &str) -> Result<Vec<Row>, Never> {
    let Ok(set) = set();

    rows_of(&Found {
      pictures: &set,
      scenes: &[],
      selection: ActiveWallpaper::Still,
      up,
      dropped: 0,
    })
  }

  fn says(rows: &[Row]) -> Result<Vec<&str>, Never> {
    Ok(rows.iter().map(|row| row.says.as_str()).collect())
  }

  #[test]
  fn a_picture_of_hers_is_named_after_its_own_file() {
    for (file, named) in [
      ("her-own-photo", "Her Own Photo"),
      ("a_quiet_lake", "A Quiet Lake"),
      ("sunset", "Sunset"),
    ] {
      let Ok(offered) = Offered::of(file);

      assert_eq!(offered.says, named);
    }
  }

  #[test]
  fn following_the_weather_is_marked_when_it_is_what_is_happening() -> Result<(), &'static str> {
    let Ok(followed) = following(0);
    let Ok(pinned) = pinned("star-ride");
    let followed_2 = followed.first().ok_or(A_ROW)?;
    let pinned_2 = pinned.first().ok_or(A_ROW)?;

    assert_eq!(followed_2.now(), Ok(Active::Yes));
    assert_eq!(pinned_2.now(), Ok(Active::No));

    Ok(())
  }

  #[test]
  fn the_picture_on_the_screen_is_marked_however_it_was_chosen() -> Result<(), &'static str> {
    let Ok(followed) = following(0);
    let Ok(followed_says) = says(&followed);
    let up = followed.get(2).ok_or(A_ROW)?;

    assert_eq!(up.now(), Ok(Active::Yes), "{followed_says:?}");

    let Ok(pinned) = pinned("her-own-photo");
    let Ok(pinned_says) = says(&pinned);
    let up_2 = pinned.get(3).ok_or(A_ROW)?;

    assert_eq!(up_2.now(), Ok(Active::Yes), "{pinned_says:?}");

    Ok(())
  }

  #[test]
  fn a_live_scene_with_the_same_name_as_its_fallback_is_marked_by_itself()
  -> Result<(), &'static str> {
    let Ok(pictures) = set();
    let Ok(scene) = Offered::of("star-ride");
    let Ok(rows) = rows_of(&Found {
      pictures: &pictures,
      scenes: &[scene],
      selection: ActiveWallpaper::Live("star-ride"),
      up: "star-ride",
      dropped: 0,
    });
    let live = rows.get(2).ok_or(A_ROW)?;
    let fallback = rows.get(3).ok_or(A_ROW)?;

    assert_eq!(live.now(), Ok(Active::Yes));
    assert_eq!(fallback.now(), Ok(Active::No));

    Ok(())
  }

  #[test]
  fn a_picture_that_is_not_up_says_who_drew_it() -> Result<(), &'static str> {
    let Ok(set) = set();
    let Ok(rows) = rows_of(&Found {
      pictures: &set,
      scenes: &[],
      selection: ActiveWallpaper::Following,
      up: "her-own-photo",
      dropped: 0,
    });
    let other = rows.get(2).ok_or(A_ROW)?;

    assert_eq!(other.aside, "Abi Toads");

    Ok(())
  }

  #[test]
  fn an_empty_drop_is_not_offered_and_leaves_the_way_in_that_always_works()
  -> Result<(), &'static str> {
    let Ok(empty) = following(0);
    let Ok(said) = says(&empty);
    let last = empty.last().ok_or(A_ROW)?;

    assert!(!said.iter().any(|says| says.contains("Pictures/Wallpapers")));
    assert_eq!(last.says, "Find a picture in the files");
    assert!(last.does.is_some(), "every row on this tab can be chosen");

    Ok(())
  }

  #[test]
  fn every_row_on_the_tab_does_something() {
    for dropped in [0, 1, 4] {
      let Ok(rows) = following(dropped);

      for row in rows {
        assert_eq!(row.acts(), Ok(Action::Yes), "{:?} does nothing", row.says);
      }
    }
  }

  #[test]
  fn a_drop_with_something_in_it_offers_to_take_it_and_says_how_much() {
    let Ok(single) = following(1);
    let Ok(several) = following(4);
    let Ok(one) = says(&single);
    let Ok(four) = says(&several);

    assert!(one.contains(&"Add the picture in Pictures/Wallpapers"));
    assert!(four.contains(&"Add the 4 pictures in Pictures/Wallpapers"));
  }

  #[test]
  fn a_machine_with_no_pictures_says_so() {
    let Ok(bare) = rows_of(&Found {
      pictures: &[],
      scenes: &[],
      selection: ActiveWallpaper::Following,
      up: "",
      dropped: 0,
    });
    let Ok(said) = says(&bare);

    assert!(said.contains(&"There are no pictures on this machine"));
  }
}
