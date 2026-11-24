//! A small desktop-first catalogue alongside Nixpkgs search.

use console_package_manager::user::Installable;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Category {
  #[default]
  All,
  Media,
  Creative,
  Everyday,
  Developer,
}

impl Category {
  pub const EVERY: [Self; 5] = [
    Self::All,
    Self::Media,
    Self::Creative,
    Self::Everyday,
    Self::Developer,
  ];

  #[must_use]
  pub fn title(self) -> core::result::Result<&'static str, core::convert::Infallible> {
    Ok(match self {
      Self::All => "All apps",
      Self::Media => "Media",
      Self::Creative => "Creative",
      Self::Everyday => "Everyday",
      Self::Developer => "Developer",
    })
  }

  #[must_use]
  pub fn mark(self) -> core::result::Result<&'static str, core::convert::Infallible> {
    Ok(match self {
      Self::All => "◇",
      Self::Media => "▶",
      Self::Creative => "✦",
      Self::Everyday => "▦",
      Self::Developer => "›_",
    })
  }
}

#[derive(Clone, Copy)]
pub struct Featured {
  pub attribute: &'static str,
  pub title: &'static str,
  pub description: &'static str,
  pub category: Category,
}

impl Featured {
  #[must_use]
  fn package(self) -> core::result::Result<Package, core::convert::Infallible> {
    let Ok(installable) = Installable::nixpkgs(self.attribute);
    Ok(Package {
      installable,
      title: self.title.to_owned(),
      description: self.description.to_owned(),
      category: self.category,
      version: String::new(),
    })
  }
}

const FEATURED: &[Featured] = &[
  Featured {
    attribute: "obs-studio",
    title: "OBS Studio",
    description: "Record your screen, mix video sources, and stream live.",
    category: Category::Media,
  },
  Featured {
    attribute: "vlc",
    title: "VLC",
    description: "Play videos, music, and discs in almost any format.",
    category: Category::Media,
  },
  Featured {
    attribute: "krita",
    title: "Krita",
    description: "Paint, sketch, and animate with digital brushes.",
    category: Category::Creative,
  },
  Featured {
    attribute: "firefox",
    title: "Firefox",
    description: "Browse the web with a privacy-focused browser.",
    category: Category::Everyday,
  },
  Featured {
    attribute: "audacity",
    title: "Audacity",
    description: "Record and edit audio, from voice notes to podcasts.",
    category: Category::Media,
  },
  Featured {
    attribute: "inkscape",
    title: "Inkscape",
    description: "Draw and edit vector artwork for any screen size.",
    category: Category::Creative,
  },
  Featured {
    attribute: "libreoffice",
    title: "LibreOffice",
    description: "Write documents, build spreadsheets, and make presentations.",
    category: Category::Everyday,
  },
  Featured {
    attribute: "gimp",
    title: "GIMP",
    description: "Retouch photos and edit images with layers.",
    category: Category::Creative,
  },
  Featured {
    attribute: "thunderbird",
    title: "Thunderbird",
    description: "Manage your email and calendars in one place.",
    category: Category::Everyday,
  },
  Featured {
    attribute: "git",
    title: "Git",
    description: "Track file changes and collaborate on code.",
    category: Category::Developer,
  },
  Featured {
    attribute: "ripgrep",
    title: "ripgrep",
    description: "Search the contents of files from your terminal.",
    category: Category::Developer,
  },
  Featured {
    attribute: "jq",
    title: "jq",
    description: "Read and transform JSON data in your terminal.",
    category: Category::Developer,
  },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
  pub installable: Installable,
  pub title: String,
  pub description: String,
  pub category: Category,
  pub version: String,
}

#[must_use]
pub fn featured(
  category: Category,
) -> core::result::Result<Vec<Package>, core::convert::Infallible> {
  Ok(
    FEATURED
      .iter()
      .filter(|item| category == Category::All || item.category == category)
      .map(|item| {
        let Ok(explicit_value) = item.package();
        explicit_value
      })
      .collect(),
  )
}

#[must_use]
pub fn suggested(query: &str) -> core::result::Result<Vec<Package>, core::convert::Infallible> {
  let terms: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
  Ok(
    ({
      let Ok(explicit_value) = featured(Category::All);
      explicit_value
    })
    .into_iter()
    .filter(|item| {
      let searchable = format!(
        "{} {} {}",
        item.installable.attribute, item.title, item.description
      )
      .to_lowercase();
      terms.iter().all(|word| searchable.contains(word))
    })
    .collect(),
  )
}

#[must_use]
pub fn package(attribute: &str) -> core::result::Result<Package, core::convert::Infallible> {
  Ok(
    match FEATURED.iter().find(|row| row.attribute == attribute) {
      Some(row) => {
        let Ok(explicit_value) = row.package();
        explicit_value
      }
      None => {
        let Ok(installable) = Installable::nixpkgs(attribute);
        Package {
          installable,
          title: attribute.to_owned(),
          description: String::new(),
          category: Category::All,
          version: String::new(),
        }
      }
    },
  )
}
