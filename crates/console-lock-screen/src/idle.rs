//! Inactivity preferences shared by Settings and the idle service.

use console_battery::PowerSource;
use console_core_never::Never;

use crate::LockScreen;

pub mod policy;

pub const SERVICE: &str = "console-idle.service";
const DELAY_SETTING: &str = "lock-screen-idle-delay";
const SUSPEND_SETTING: &str = "lock-screen-on-suspend";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delay(Option<u32>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidDelay;

impl Default for Delay {
  fn default() -> Self {
    Self(Some(300))
  }
}

impl Delay {
  pub const MAX_SECONDS: u32 = u32::MAX / 1000;
  pub const NEVER: Self = Self(None);
  pub const PRESETS: [(Self, &'static str); 6] = [
    (Self(Some(30)), "30 seconds"),
    (Self(Some(60)), "1 minute"),
    (Self(Some(120)), "2 minutes"),
    (Self(Some(300)), "5 minutes"),
    (Self(Some(600)), "10 minutes"),
    (Self::NEVER, "Never"),
  ];

  #[must_use]
  fn from_seconds(seconds: u32) -> core::result::Result<Option<Self>, Never> {
    Ok(
      (1..=Self::MAX_SECONDS)
        .contains(&seconds)
        .then_some(Self(Some(seconds))),
    )
  }

  #[must_use]
  pub fn seconds(self) -> Result<Option<u32>, Never> {
    Ok(self.0)
  }

  pub fn milliseconds(self) -> Result<Option<u32>, Never> {
    Ok(self.0.map(|seconds| seconds.saturating_mul(1000)))
  }

  #[must_use]
  pub fn custom(text: &str) -> Result<Self, InvalidDelay> {
    let trimmed = text.trim();
    let (amount, factor) = match trimmed.strip_suffix('h') {
      Some(hours) => (hours, 3600),
      None => match trimmed.strip_suffix('m') {
        Some(minutes) => (minutes, 60),
        None => match trimmed.strip_suffix('s') {
          Some(seconds) => (seconds, 1),
          None => (trimmed, 1),
        },
      },
    };
    let amount_2 = amount.trim_end().parse::<u32>().map_err(|_| InvalidDelay)?;
    let seconds = amount_2.checked_mul(factor).ok_or(InvalidDelay)?;
    let Ok(delay) = Self::from_seconds(seconds);

    delay.ok_or(InvalidDelay)
  }

  #[must_use]
  fn stored(text: &str) -> core::result::Result<Option<Self>, Never> {
    match text {
      "never" => Ok(Some(Self::NEVER)),
      _ => {
        let seconds = match text.parse::<u32>() {
          Ok(seconds) => seconds,
          Err(_invalid_seconds) => return Ok(None),
        };

        Self::from_seconds(seconds)
      }
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Profile {
  General,
  OnBattery,
  PluggedIn,
}

impl From<PowerSource> for Profile {
  fn from(source: PowerSource) -> Self {
    match source {
      PowerSource::WithoutBattery => Self::General,
      PowerSource::OnBattery => Self::OnBattery,
      PowerSource::PluggedIn => Self::PluggedIn,
    }
  }
}

impl Profile {
  #[must_use]
  fn key(self) -> core::result::Result<&'static str, Never> {
    Ok(match self {
      Self::General => DELAY_SETTING,
      Self::OnBattery => "lock-screen-idle-delay-on-battery",
      Self::PluggedIn => "lock-screen-idle-delay-plugged-in",
    })
  }
}

#[must_use]
fn stored(profile: Profile) -> core::result::Result<Option<Delay>, Never> {
  let Ok(key) = profile.key();
  let Ok(value) = console_defaults::setting(key);

  match value.as_deref() {
    Some(text) => Delay::stored(text),
    None => Ok(None),
  }
}

#[must_use]
pub fn delay(profile: Profile) -> Result<Delay, Never> {
  let Ok(preferred) = stored(profile);
  let configured = preferred.or_else(|| {
    let Ok(general) = stored(Profile::General);
    general
  });

  Ok(match configured {
    Some(configured_delay) => configured_delay,
    None => Delay::default(),
  })
}

pub fn choose_delay(profile: Profile, delay: Delay) -> Result<(), Never> {
  let Ok(seconds) = delay.seconds();
  let value = seconds.map_or_else(|| "never".to_owned(), |timeout| timeout.to_string());
  console_defaults::set(console_defaults::Setting {
    key: ({
      let Ok(explicit_value) = profile.key();
      explicit_value
    }),
    value: &value,
  })
}

pub fn on_suspend() -> Result<LockScreen, Never> {
  let Ok(value) = console_defaults::setting(SUSPEND_SETTING);
  crate::read_switch(SUSPEND_SETTING, value.as_deref())
}

pub fn choose_on_suspend(chosen: LockScreen) -> Result<(), Never> {
  crate::set_switch(SUSPEND_SETTING, chosen)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
  Dim,
  Blank,
  Suspend,
}

impl Action {
  pub const ALL: [Self; 3] = [Self::Dim, Self::Blank, Self::Suspend];

  fn preference(self) -> Result<(&'static str, Delay), Never> {
    Ok(match self {
      Self::Dim => ("screen-dim-idle-delay", Delay(Some(120))),
      Self::Blank => ("screen-blank-idle-delay", Delay(Some(300))),
      Self::Suspend => ("system-suspend-idle-delay", Delay(Some(600))),
    })
  }

  pub fn delay(self) -> Result<Delay, Never> {
    let Ok((key, default)) = self.preference();
    let Ok(value) = console_defaults::setting(key);

    Ok(match value.as_deref() {
      Some(text) => {
        let Ok(found) = Delay::stored(text);

        match found {
          Some(saved) => saved,
          None => default,
        }
      }
      None => default,
    })
  }

  pub fn choose(self, delay: Delay) -> Result<(), Never> {
    let Ok((key, _default)) = self.preference();
    let Ok(seconds) = delay.seconds();
    let value = seconds.map_or_else(|| "never".to_owned(), |timeout| timeout.to_string());
    console_defaults::set(console_defaults::Setting { key, value: &value })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn accepts_custom_seconds_minutes_and_hours() -> Result<(), InvalidDelay> {
    for (input, seconds) in [("47", 47), ("90s", 90), (" 13 m ", 780), ("2h", 7200)] {
      let parsed = Delay::custom(input)?;
      assert_eq!(parsed.seconds(), Ok(Some(seconds)));
    }

    assert_eq!(
      Delay::custom(&Delay::MAX_SECONDS.to_string()),
      Ok(Delay(Some(Delay::MAX_SECONDS)))
    );

    for input in ["", "never", "0", "-1", "1.5m", "4294968", "99999999h"] {
      assert_eq!(Delay::custom(input), Err(InvalidDelay), "{input}");
    }

    Ok(())
  }

  #[test]
  fn existing_saved_delays_keep_their_meaning() {
    for (delay, _) in Delay::PRESETS {
      let Ok(timeout) = delay.seconds();
      let written = timeout.map_or_else(|| "never".to_owned(), |seconds| seconds.to_string());
      assert_eq!(
        ({
          let Ok(explicit_value) = Delay::stored(&written);
          explicit_value
        }),
        Some(delay)
      );
    }
  }
}
