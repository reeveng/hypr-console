use std::{collections::BTreeSet, fmt, io, marker::PhantomData};

use console_compositor::{DispatchResult, Query, Request, quote};
use console_core_external_programs::Program;
use console_core_never::Never;
use serde::Deserialize;
use serde::de::{self, Error, IgnoredAny, MapAccess, Visitor};

use super::called;

const DEVICE_REPLY_BYTES: u32 = 1_048_576;

struct Devices {
  mice: Vec<Mouse>,
}

struct Mouse {
  name: String,
}

impl<'de> Deserialize<'de> for Devices {
  fn deserialize<D: de::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
    let mice = decoder.deserialize_map(Field {
      name: "mice",
      value: PhantomData,
    })?;

    Ok(Self { mice })
  }
}

impl<'de> Deserialize<'de> for Mouse {
  fn deserialize<D: de::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
    let name = decoder.deserialize_map(Field {
      name: "name",
      value: PhantomData,
    })?;

    Ok(Self { name })
  }
}

struct Field<T> {
  name: &'static str,
  value: PhantomData<T>,
}

impl<'de, T: Deserialize<'de>> Visitor<'de> for Field<T> {
  type Value = T;

  fn expecting(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(to, "a Hyprland object containing {}", self.name)
  }

  fn visit_map<M: MapAccess<'de>>(self, mut fields: M) -> Result<T, M::Error> {
    let mut value = None;

    for _ in 0..=DEVICE_REPLY_BYTES {
      let next_key = fields.next_key::<String>()?;

      match next_key {
        Some(key) => match (key == self.name, value.as_ref()) {
          (true, None) => {
            let decoded = fields.next_value()?;
            value = Some(decoded);
          }
          (true, Some(_)) => return Err(M::Error::duplicate_field(self.name)),
          (false, _) => {
            let _extension = fields.next_value::<IgnoredAny>()?;
          }
        },
        None => return value.ok_or_else(|| M::Error::missing_field(self.name)),
      }
    }

    Err(M::Error::custom(
      "too many fields in a Hyprland device reply",
    ))
  }
}

fn read_devices(reply: &str) -> io::Result<Devices> {
  let length = u32::try_from(reply.len()).map_err(io::Error::other)?;

  match (length <= DEVICE_REPLY_BYTES).then_some(()) {
    Some(()) => serde_json::from_str(reply).map_err(io::Error::other),
    None => Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "Hyprland device reply exceeds 1 MiB",
    )),
  }
}

fn touchpad_names(database: &str) -> Result<BTreeSet<String>, Never> {
  Ok(
    database
      .split("\n\n")
      .filter_map(|device| {
        device
          .lines()
          .any(|line| line == "E: ID_INPUT_TOUCHPAD=1")
          .then_some(())?;
        let raw = device
          .lines()
          .find_map(|line| line.strip_prefix("E: NAME="))?;
        let name = match raw
          .strip_prefix('"')
          .and_then(|quoted| quoted.strip_suffix('"'))
        {
          Some(unquoted) => unquoted,
          None => raw,
        };

        Some(name.to_lowercase().replace([' ', ',', '\n'], "-"))
      })
      .collect(),
  )
}

fn toggle_script(touchpads: &BTreeSet<String>, devices: &Devices) -> Result<String, Never> {
  let names: Vec<_> = devices
    .mice
    .iter()
    .filter(|mouse| {
      touchpads.contains(&mouse.name)
        || mouse.name.rsplit_once('-').is_some_and(|(name, suffix)| {
          !suffix.is_empty()
            && suffix.bytes().all(|byte| byte.is_ascii_digit())
            && touchpads.contains(name)
        })
    })
    .map(|mouse| {
      let Ok(quoted) = quote(&mouse.name);
      quoted
    })
    .collect();
  Ok(format!(
    r#"local pads = {{{}}}
if console_disabled_touchpads then
    for _, name in ipairs(console_disabled_touchpads) do
        hl.device({{ name = name, enabled = true }})
    end
    console_disabled_touchpads = nil
elseif #pads > 0 then
    for _, name in ipairs(pads) do
        hl.device({{ name = name, enabled = false }})
    end
    console_disabled_touchpads = pads
end"#,
    names.join(", ")
  ))
}

pub fn toggle() -> io::Result<()> {
  let database = called(
    Program::Udevadm,
    &["info", "--export-db", "--subsystem-match=input"],
  )?;
  let Ok(touchpads) = touchpad_names(&database);
  let Ok(command) = Program::Hyprctl.command();
  let reading =
    console_compositor::query_text(command, Query::Devices).map_err(io::Error::other)?;
  let devices = read_devices(&reading)?;
  let Ok(script) = toggle_script(&touchpads, &devices);
  let Ok(result) = console_compositor::request(Request::Script, &script);

  match result {
    DispatchResult::Success => Ok(()),
    DispatchResult::Failure(fault) => Err(io::Error::other(fault)),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_touchpads_classified_by_udev_are_toggled() -> io::Result<()> {
    let Ok(touchpads) = touchpad_names(concat!(
      "E: ID_INPUT_TOUCHPAD=1\nE: NAME=\"Apple, Trackpad\"\n\n",
      "E: ID_INPUT_MOUSE=1\nE: NAME=\"TrackPoint\"\n\n",
      "E: ID_INPUT_TOUCHSCREEN=1\nE: NAME=\"Touch Display\"\n",
    ));
    let devices = read_devices(
      r#"{"mice":[{"name":"apple--trackpad-2"},{"name":"trackpoint"}],"touch":[{"name":"touch-display"}]}"#,
    )?;
    let Ok(script) = toggle_script(&touchpads, &devices);

    assert_eq!(
      script.lines().next(),
      Some(r#"local pads = {"apple--trackpad-2"}"#)
    );

    Ok(())
  }
}
