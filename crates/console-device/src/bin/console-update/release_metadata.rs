//! The fields used by the release updater are read once from a GitHub response bounded to 1 MiB.
//! Remaining fields, including release notes and author information, are consumed as ignored metadata.

use std::fmt;

use console_core_never::Never;
use serde::de::{Error, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use super::UpdateError;

pub(super) const MAX_METADATA: u32 = 1_048_576;

pub(super) enum Publication {
  ImmutableRelease,
  Ineligible,
}

pub(super) struct GitHubRelease {
  pub(super) publication: Publication,
  pub(super) tag_name: String,
  pub(super) assets: Vec<ReleaseAsset>,
}

pub(super) struct ReleaseAsset {
  pub(super) name: String,
  pub(super) state: String,
  pub(super) digest: Option<String>,
  pub(super) size: Option<u64>,
}

pub(super) fn decode(response: &[u8]) -> Result<GitHubRelease, UpdateError> {
  let bytes = u64::try_from(response.len())?;

  match (bytes <= u64::from(MAX_METADATA)).then_some(()) {
    Some(()) => {}
    None => return Err(UpdateError::from("GitHub release metadata exceeds 1 MiB")),
  }

  serde_json::from_slice(response)
    .map_err(|fault| UpdateError::from(format!("GitHub release metadata: {fault}")))
}

enum Field<T> {
  Missing,
  Present(T),
}

impl<T> Field<T> {
  fn read<'de, A>(&mut self, name: &'static str, fields: &mut A) -> Result<(), A::Error>
  where
    T: Deserialize<'de>,
    A: MapAccess<'de>,
  {
    match self {
      Self::Missing => {
        let value = fields.next_value()?;
        *self = Self::Present(value);
        Ok(())
      }
      Self::Present(_) => Err(A::Error::duplicate_field(name)),
    }
  }

  fn required<E: Error>(self, name: &'static str) -> Result<T, E> {
    match self {
      Self::Missing => Err(E::missing_field(name)),
      Self::Present(value) => Ok(value),
    }
  }
}

impl<T> Field<Option<T>> {
  fn optional(self) -> Result<Option<T>, Never> {
    Ok(match self {
      Self::Missing => None,
      Self::Present(value) => value,
    })
  }
}

impl<'de> Deserialize<'de> for GitHubRelease {
  fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
    decoder.deserialize_map(ReleaseVisitor)
  }
}

struct ReleaseVisitor;

impl<'de> Visitor<'de> for ReleaseVisitor {
  type Value = GitHubRelease;

  fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    out.write_str("a GitHub release object")
  }

  fn visit_map<A: MapAccess<'de>>(self, mut fields: A) -> Result<Self::Value, A::Error> {
    let mut immutable_field = Field::Missing;
    let mut draft_field = Field::Missing;
    let mut prerelease_field = Field::Missing;
    let mut tag = Field::Missing;
    let mut assets_field = Field::Missing;

    for _ in 0..MAX_METADATA {
      let next = fields.next_key::<String>()?;
      let name = match next {
        Some(field) => field,
        None => break,
      };

      match name.as_str() {
        "immutable" => immutable_field.read("immutable", &mut fields)?,
        "draft" => draft_field.read("draft", &mut fields)?,
        "prerelease" => prerelease_field.read("prerelease", &mut fields)?,
        "tag_name" => tag.read("tag_name", &mut fields)?,
        "assets" => assets_field.read("assets", &mut fields)?,
        _ => {
          let _ignored: IgnoredAny = fields.next_value()?;
        }
      }
    }

    let immutable = immutable_field.required("immutable")?;
    let draft = draft_field.required("draft")?;
    let prerelease = prerelease_field.required("prerelease")?;
    let tag_name = tag.required("tag_name")?;
    let assets = assets_field.required("assets")?;
    let publication = match (immutable, draft, prerelease) {
      (true, false, false) => Publication::ImmutableRelease,
      (false, _, _) | (true, true, _) | (true, false, true) => Publication::Ineligible,
    };

    Ok(GitHubRelease {
      publication,
      tag_name,
      assets,
    })
  }
}

impl<'de> Deserialize<'de> for ReleaseAsset {
  fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
    decoder.deserialize_map(AssetVisitor)
  }
}

struct AssetVisitor;

impl<'de> Visitor<'de> for AssetVisitor {
  type Value = ReleaseAsset;

  fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    out.write_str("a GitHub release asset object")
  }

  fn visit_map<A: MapAccess<'de>>(self, mut fields: A) -> Result<Self::Value, A::Error> {
    let mut name_field = Field::Missing;
    let mut state_field = Field::Missing;
    let mut digest_field = Field::Missing;
    let mut size_field = Field::Missing;

    for _ in 0..MAX_METADATA {
      let next = fields.next_key::<String>()?;
      let field = match next {
        Some(field) => field,
        None => break,
      };

      match field.as_str() {
        "name" => name_field.read("name", &mut fields)?,
        "state" => state_field.read("state", &mut fields)?,
        "digest" => digest_field.read("digest", &mut fields)?,
        "size" => size_field.read("size", &mut fields)?,
        _ => {
          let _ignored: IgnoredAny = fields.next_value()?;
        }
      }
    }

    let name = name_field.required("name")?;
    let state = state_field.required("state")?;
    let Ok(digest) = digest_field.optional();
    let Ok(size) = size_field.optional();
    Ok(ReleaseAsset {
      name,
      state,
      digest,
      size,
    })
  }
}
