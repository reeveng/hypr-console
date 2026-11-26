//! The four readings, each as the bar draws it.
//!
//! A reading says what it is and never what it should look like. It used to
//! carry a class -- `muted`, `off`, `critical`, `urgent` -- which is a name in a
//! stylesheet spelled again in Rust, and the stylesheet had nine of them for
//! five colors because every module had invented its own word for the same
//! thing. So what comes back now is a [`Tone`]: the resting color, the quiet
//! one a reading with nothing to report wears, and the three a machine wears
//! when something is happening to it. What each of those is worth in ink is
//! decided once, where the bar is drawn.
//!
//! One of the six is not a reading at all. The two icons on the left of the bar
//! open a thing and close it again, and the stylesheet drew them a shade
//! brighter than everything beside them because a button that looks like a
//! reading is a button no one presses. That is the same kind of statement as
//! `muted` -- what this is, said so that one place can decide what it is
//! worth -- so `Pressed` lives here with the rest of them and no reading ever
//! returns it.

use crate::settings::SettingsTab;
use console_battery::{Charge, Filling};
use console_bluetooth::adapter::Power;
use console_bluetooth::device::Connection;
use console_bluetooth::objects::{self, Objects};
use console_bus::connection::Bus;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, whole_u32};
use console_panel::door::Up;
use console_panel::running::run_output;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tone {
  Plain,
  Secondary,
  Pressed,
  Low,
  Error,
  Well,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reading {
  pub icon: String,
  pub beside: Option<String>,
  pub tone: Tone,
}

impl Reading {
  pub fn new(icon: &str, tone: Tone) -> Result<Self, Never> {
    Ok(Reading {
      icon: String::from(icon),
      beside: None,
      tone,
    })
  }

  pub fn and(self, said: String) -> Result<Self, Never> {
    Ok(Reading {
      beside: Some(said),
      ..self
    })
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum StatusItem {
  Battery,
  Bluetooth,
  Network,
  Sound,
}

impl StatusItem {
  pub const fn settings_tab(self) -> Result<SettingsTab, Never> {
    Ok(match self {
      Self::Battery => SettingsTab::Battery,
      Self::Bluetooth => SettingsTab::Bluetooth,
      Self::Network => SettingsTab::Wifi,
      Self::Sound => SettingsTab::Sound,
    })
  }

  pub fn tab(self) -> Result<&'static str, Never> {
    let Ok(tab) = self.settings_tab();

    tab.english_title()
  }

  pub fn parse(word: &str) -> Result<Option<Self>, Never> {
    Ok(match word {
      "battery" => Some(StatusItem::Battery),
      "bluetooth" => Some(StatusItem::Bluetooth),
      "network" => Some(StatusItem::Network),
      "sound" => Some(StatusItem::Sound),
      _ => None,
    })
  }

  pub fn reading(self) -> Result<Reading, Never> {
    match self {
      StatusItem::Battery => {
        let said = console_battery::charge()?;

        battery(&said)
      }
      StatusItem::Bluetooth => {
        let Ok(radio) = radio();

        bluetooth(radio.as_ref())
      }
      StatusItem::Network => {
        let asked = &["-t", "-f", "TYPE,STATE,CONNECTION", "device", "status"];
        let Ok(wifi) = wifi();
        let Ok(devices) = run_output(Program::Nmcli, asked);

        network(Readings {
          devices: &devices,
          wifi: &wifi,
        })
      }
      StatusItem::Sound => {
        let Ok(level) = run_output(Program::Wpctl, &["get-volume", SINK]);

        sound(&level)
      }
    }
  }
}

pub fn worn(tone: Tone) -> Result<Option<&'static str>, Never> {
  Ok(match tone {
    Tone::Plain | Tone::Pressed => None,
    Tone::Secondary => Some("secondary"),
    Tone::Low => Some("low"),
    Tone::Error => Some("error"),
    Tone::Well => Some("well"),
  })
}

pub fn line(reading: &Reading, open: Up) -> Result<String, Never> {
  let lit = match open {
    Up::OnScreen => Some("open"),
    Up::NotThere => None,
  };
  let Ok(state) = worn(reading.tone);
  let worn: Vec<&str> = state.into_iter().chain(lit).collect();
  let class = match ::core::primitive::bool::then_some(worn.is_empty(), ()) {
    ::core::option::Option::Some(()) => String::new(),
    ::core::option::Option::None => format!(r#","class":{}"#, serde_json::Value::from(worn)),
  };
  let text = match &reading.beside {
    Some(beside) => {
      let Ok(small) = small(beside);

      format!("{} {small}", reading.icon)
    }
    None => reading.icon.clone(),
  };

  Ok(format!(
    r#"{{"text":{}{class}}}"#,
    serde_json::Value::String(text)
  ))
}

fn whole(percent: i32) -> Result<Option<u32>, Never> {
  let whole = match u32::try_from(percent) {
    Ok(whole) => whole,
    Err(_too_large) => return Ok(None),
  };

  Ok(Some(whole))
}

fn number<T: std::str::FromStr>(said: &str) -> Result<Option<T>, Never> {
  let number = match said.trim().parse::<T>() {
    Ok(number) => number,
    Err(_not_a_number) => return Ok(None),
  };

  Ok(Some(number))
}

pub fn battery(said: &str) -> Result<Reading, Never> {
  let Ok(reading) = Charge::of(said);

  let told = reading.percent.and_then(|percent| {
    let Ok(told) = whole(percent);

    told
  });

  let charge = match told {
    Some(charge) => charge,
    None => {
      let Ok(blank) = wide("");
      let Ok(reading_2) = Reading::new(NO_BATTERY, Tone::Plain);

      return reading_2.and(blank);
    }
  };

  let filling = reading.filling;
  let Ok(level) = icon_for(&LEVELS, charge);

  let icon = match filling {
    Filling::Yes => CHARGING,
    Filling::Charged => PLUGGED,
    Filling::No => level,
  };
  let tone = match (filling, charge) {
    (Filling::Yes | Filling::Charged, _) => Tone::Well,
    (_, 0..=10) => Tone::Error,
    (_, 11..=25) => Tone::Low,
    _ => Tone::Plain,
  };

  let Ok(percent) = wide(&format!("{charge}%"));
  let Ok(reading_3) = Reading::new(icon, tone);

  reading_3.and(percent)
}

fn wide(reading: &str) -> Result<String, Never> {
  let Ok(written) = fitted::<_, u32>(reading.chars().count());
  let Ok(short) = index(WIDEST.saturating_sub(written));

  Ok(format!("{reading}{}", FIGURE.repeat(short)))
}

const WIDEST: u32 = 4;

const FIGURE: &str = "\u{2007}";

fn small(text: &str) -> Result<String, Never> {
  Ok(format!(r#"<span size="x-small">{text}</span>"#))
}

pub const CHARGING: &str = "\u{f0084}";

pub const PLUGGED: &str = "\u{f06a5}";

pub const NO_BATTERY: &str = "\u{f008e}";

const LEVELS: [&str; 5] = [
  "\u{f007a}",
  "\u{f007c}",
  "\u{f007e}",
  "\u{f0080}",
  "\u{f0079}",
];

const A_WHOLE: std::num::NonZeroU32 = match std::num::NonZeroU32::new(100) {
  Some(whole) => whole,
  None => std::num::NonZeroU32::MIN,
};

const NO_ICON: &str = "";

const NOTHING_HEARD: u32 = 0;

fn icon_for(icons: &[&'static str], percent: u32) -> Result<&'static str, Never> {
  let Ok(many) = fitted::<_, u32>(icons.len());
  let last = many.saturating_sub(1);
  let Ok(at) = index(percent.min(100).saturating_mul(last) / A_WHOLE);

  Ok(match icons.get(at).copied() {
    Some(icon) => icon,
    None => NO_ICON,
  })
}

fn radio() -> Result<Option<Objects>, Never> {
  let mut bus = match Bus::system() {
    Ok(bus) => bus,
    Err(_unreachable) => return Ok(None),
  };

  let Ok(asking) = objects::managed();

  let answer = match bus.call(&asking) {
    Ok(answer) => answer,
    Err(_hung_up) => return Ok(None),
  };

  Ok(match answer.fault {
    Some(_refused) => None,
    None => {
      let Ok(read) = objects::read(&answer);

      Some(read)
    }
  })
}

pub fn bluetooth(radio: Option<&Objects>) -> Result<Reading, Never> {
  let objects = match radio {
    Some(objects) => objects,
    None => return Reading::new("\u{f00b2}", Tone::Secondary),
  };

  let Ok(power) = objects.power();
  let connected = objects
    .devices
    .iter()
    .any(|device| device.connection == Connection::Connected);

  match (power, connected) {
    (Power::Off, true | false) => Reading::new("\u{f00b2}", Tone::Secondary),
    (Power::On, false) => Reading::new("\u{f00af}", Tone::Plain),
    (Power::On, true) => Reading::new("\u{f00b1}", Tone::Plain),
  }
}

fn wifi() -> Result<String, Never> {
  run_output(
    Program::Nmcli,
    &[
      "-t",
      "-f",
      "IN-USE,SIGNAL",
      "device",
      "wifi",
      "list",
      "--rescan",
      "no",
    ],
  )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Readings<'a> {
  pub devices: &'a str,
  pub wifi: &'a str,
}

pub fn network(asked: Readings<'_>) -> Result<Reading, Never> {
  let Readings { devices, wifi } = asked;
  let connected = |kind: &str| {
    devices
      .lines()
      .filter_map(|line| {
        let mut fields = line.split(':');
        let type_ = fields.next()?;
        let state = fields.next()?;

        Some((type_, state))
      })
      .any(|(type_, state)| type_ == kind && state == "connected")
  };

  match ::core::primitive::bool::then_some(connected("wifi"), ()) {
    ::core::option::Option::Some(()) => {
      let strength = wifi
        .lines()
        .find(|line| line.starts_with('*'))
        .and_then(|line| line.split(':').nth(1))
        .and_then(|said| {
          let Ok(told) = number::<u32>(said);

          told
        });

      let strength_3 = match strength {
        Some(strength_2) => strength_2,
        None => NOTHING_HEARD,
      };

      let Ok(bars) = icon_for(&BARS, strength_3);

      return Reading::new(bars, Tone::Plain);
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(connected("ethernet"), ()) {
    ::core::option::Option::Some(()) => Reading::new("\u{f0200}", Tone::Plain),
    ::core::option::Option::None => Reading::new("\u{f05aa}", Tone::Secondary),
  }
}

const BARS: [&str; 4] = ["\u{f091f}", "\u{f0922}", "\u{f0925}", "\u{f0928}"];

pub const SILENT: &str = "\u{f075f}";

pub const SINK: &str = "@DEFAULT_AUDIO_SINK@";

fn loudness(said: &str) -> Result<Option<f64>, Never> {
  Ok(said.split_whitespace().nth(1).and_then(|said_2| {
    let Ok(told) = number::<f64>(said_2);

    told
  }))
}

pub fn level(said: &str) -> Result<Option<Reading>, Never> {
  let Ok(told) = loudness(said);

  let volume = match told {
    Some(volume) => volume,
    None => return Ok(None),
  };

  let Ok(percent) = whole_u32(volume * 100.0);
  let Ok(reading) = sound(said);
  let Ok(beside) = wide(&format!("{percent}%"));
  let Ok(level) = reading.and(beside);

  Ok(Some(level))
}

pub fn read_level() -> Result<Option<Reading>, Never> {
  let Ok(said) = run_output(Program::Wpctl, &["get-volume", SINK]);

  level(&said)
}

pub fn sound(said: &str) -> Result<Reading, Never> {
  use crate::retained::Volume;

  let Ok(told) = loudness(said);
  let volume = match told {
    Some(volume) => volume,
    None => return Volume::Unavailable.reading(),
  };

  match said.contains("[MUTED]").then_some(()) {
    Some(()) => return Volume::Silent.reading(),
    None => {}
  }

  let Ok(percent) = whole_u32(volume * 100.0);
  let icon = match percent {
    0 => Volume::Silent,
    1..=33 => Volume::Low,
    34..=66 => Volume::Medium,
    _ => Volume::High,
  };

  icon.reading()
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;

  use console_bluetooth::objects::written::{self, managed, object, set};
  use console_bluetooth::{adapter, device};
  use console_bus::messages::Truth;
  use console_bus::properties::{InterfaceName, PropertyName};

  fn plain_reading(icon: &str, tone: Tone) -> Result<Reading, Never> {
    Ok(Reading {
      icon: String::from(icon),
      beside: None,
      tone,
    })
  }

  fn around(power: Power, connections: &[Connection]) -> Result<Objects, Never> {
    let powered = match power {
      Power::On => Truth::Yes,
      Power::Off => Truth::No,
    };
    let Ok(powering) = set(PropertyName(adapter::POWERED), powered);
    let Ok(radio) = object(
      "/org/bluez/hci0",
      InterfaceName(adapter::INTERFACE),
      vec![powering],
    );
    let devices = (0_u8..).zip(connections).map(|(last, connection)| {
      let connected = match connection {
        Connection::Connected => Truth::Yes,
        Connection::NotConnected => Truth::No,
      };
      let Ok(addressed) = written::said(
        PropertyName(device::ADDRESS),
        &format!("AC:80:0A:12:34:{last:02X}"),
      );
      let Ok(connecting) = set(PropertyName(device::CONNECTED), connected);
      let path = format!("/org/bluez/hci0/dev_AC_80_0A_12_34_{last:02X}");
      let Ok(known) = object(
        &path,
        InterfaceName(device::INTERFACE),
        vec![addressed, connecting],
      );

      known
    });
    let every = std::iter::once(radio).chain(devices).collect();
    let Ok(reply) = managed(every);

    objects::read(&reply)
  }

  fn the_four() -> Result<[Objects; 4], Never> {
    let Ok(off) = around(Power::Off, &[]);
    let Ok(on) = around(Power::On, &[Connection::NotConnected]);
    let Ok(one) = around(
      Power::On,
      &[Connection::Connected, Connection::NotConnected],
    );
    let Ok(two) = around(Power::On, &[Connection::Connected, Connection::Connected]);

    Ok([off, on, one, two])
  }

  #[derive(serde::Deserialize)]
  #[serde(deny_unknown_fields)]
  struct BarLine {
    text: Box<str>,
    #[serde(default, rename = "class")]
    classes: Classes,
  }

  #[derive(Default)]
  struct Classes(Option<Vec<String>>);

  impl<'de> serde::Deserialize<'de> for Classes {
    fn deserialize<D: serde::Deserializer<'de>>(input: D) -> Result<Self, D::Error> {
      Vec::deserialize(input).map(|classes| Self(Some(classes)))
    }
  }

  fn worn(said: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let held: BarLine = serde_json::from_str(said)?;
    let classes = held.classes.0.ok_or("a list of classes")?;

    Ok(classes)
  }

  #[test]
  fn a_reading_with_nothing_to_say_about_itself_carries_no_class() -> Result<(), Box<dyn Error>> {
    let Ok(reading) = plain_reading("64%", Tone::Plain);
    let Ok(said) = line(&reading, Up::NotThere);
    let held: BarLine = serde_json::from_str(&said)?;

    assert!(held.classes.0.is_none());

    Ok(())
  }

  #[test]
  fn the_tab_in_front_is_the_only_thing_that_lights_it() -> Result<(), Box<dyn Error>> {
    let Ok(reading) = plain_reading("64%", Tone::Plain);
    let Ok(said) = line(&reading, Up::OnScreen);
    let worn = worn(&said)?;

    assert_eq!(worn, ["open"]);

    Ok(())
  }

  #[test]
  fn a_reading_that_says_something_says_it_beside_being_open() -> Result<(), Box<dyn Error>> {
    let Ok(reading) = plain_reading("x", Tone::Secondary);
    let Ok(open) = line(&reading, Up::OnScreen);
    let Ok(shut) = line(&reading, Up::NotThere);
    let open_classes = worn(&open)?;
    let closed_classes = worn(&shut)?;

    assert_eq!(open_classes, ["secondary", "open"]);
    assert_eq!(closed_classes, ["secondary"]);

    Ok(())
  }

  #[test]
  fn every_class_is_one_name_and_never_a_line_of_words() -> Result<(), Box<dyn Error>> {
    for tone in [
      Tone::Plain,
      Tone::Secondary,
      Tone::Pressed,
      Tone::Low,
      Tone::Error,
      Tone::Well,
    ] {
      'over_open_2: for open in [Up::OnScreen, Up::NotThere] {
        let Ok(reading) = plain_reading("x", tone);
        let Ok(said) = line(&reading, open);
        let held: BarLine = serde_json::from_str(&said)?;

        let worn = match held.classes.0 {
          Some(matched_worn) => matched_worn,
          ::core::option::Option::None => continue 'over_open_2,
        };

        for name in worn {
          assert!(!name.contains(char::is_whitespace), "{said} wears {name:?}");
          assert!(!name.is_empty(), "{said} wears an empty class");
        }
      }
    }

    Ok(())
  }

  #[test]
  fn the_text_is_written_as_json_rather_than_pasted_in() -> Result<(), Box<dyn Error>> {
    let Ok(reading) = plain_reading(r#"a "quoted" \ name"#, Tone::Plain);
    let Ok(said) = line(&reading, Up::NotThere);
    let held: BarLine = serde_json::from_str(&said)?;

    assert_eq!(held.text.as_ref(), r#"a "quoted" \ name"#);

    Ok(())
  }

  #[test]
  fn a_reading_with_something_beside_it_says_both_and_nothing_else_does()
  -> Result<(), Box<dyn Error>> {
    let Ok(charge) = battery("64 unplugged Discharging");
    let Ok(said) = line(&charge, Up::NotThere);
    let BarLine { text, .. } = serde_json::from_str(&said)?;
    let Ok(volume) = sound("Volume: 0.35");

    assert!(
      text.starts_with(&charge.icon),
      "{text} does not start with its icon"
    );
    assert!(text.contains("64%"), "{text} does not carry the charge");
    assert_eq!(volume.beside, None);

    Ok(())
  }

  #[test]
  fn a_battery_on_the_mains_says_so() {
    let Ok(plugged) = battery("95 plugged Charging");
    let Ok(full) = battery("95 unplugged Discharging");
    let Ok(empty) = battery("8 unplugged Discharging");
    let Ok(low) = battery("20 unplugged Discharging");

    assert_eq!(plugged.tone, Tone::Well);
    assert_eq!(full.tone, Tone::Plain);
    assert_eq!(empty.tone, Tone::Error);
    assert_eq!(low.tone, Tone::Low);
  }

  #[test]
  fn a_battery_held_at_a_limit_is_drawn_as_a_machine_on_the_cable() {
    let Ok(held) = battery("78 plugged Not charging");

    assert_eq!(
      held.tone,
      Tone::Well,
      "a plugged-in device was drawn as one running flat"
    );
    assert_eq!(
      held.icon, PLUGGED,
      "{} does not say the cable is in",
      held.icon
    );
    assert_ne!(
      held.icon, CHARGING,
      "a battery that is not filling was drawn as one that is"
    );
  }

  #[test]
  fn a_battery_nothing_answered_for_is_not_drawn_as_full() {
    let Ok(reading) = battery("");

    assert_eq!(reading.icon, NO_BATTERY);
    assert!(!reading.beside.is_some_and(|beside| beside.contains('%')));
  }

  fn width(reading: &Reading) -> Result<u32, Never> {
    let Ok(icon) = fitted::<_, u32>(reading.icon.chars().count());

    let beside = match &reading.beside {
      Some(beside) => fitted::<_, u32>(beside.chars().count()),
      None => Ok(0),
    };
    let Ok(beside_2) = beside;

    Ok(icon.saturating_add(beside_2))
  }

  #[test]
  fn no_reading_is_a_different_width_for_saying_a_different_thing() {
    let one_width = |item: &str, said: Vec<Reading>| {
      let widths: std::collections::BTreeSet<u32> = said
        .iter()
        .map(|reading| {
          let Ok(wide) = width(reading);

          wide
        })
        .collect();
      assert_eq!(widths.len(), 1, "{item} is drawn {widths:?} wide: {said:?}");
    };

    let Ok(unanswered) = battery("");
    let mut charges = vec![unanswered];

    for charge in 0..=100 {
      let Ok(discharging) = battery(&format!("{charge} Discharging"));
      let Ok(charging) = battery(&format!("{charge} Charging"));

      charges.push(discharging);
      charges.push(charging);
    }

    one_width("the battery", charges);

    let Ok(silent) = sound("");
    let Ok(muted) = sound("Volume: 0.35 [MUTED]");
    let mut volumes = vec![silent, muted];

    for step in 0_u32..=100 {
      let Ok(volume) = sound(&format!(
        "Volume: {}.{:02}",
        step.div_euclid(100),
        step.rem_euclid(100)
      ));

      volumes.push(volume);
    }

    one_width("the sound", volumes);

    let Ok(disconnected) = network(Readings {
      devices: "wifi:disconnected:",
      wifi: "",
    });
    let Ok(wired) = network(Readings {
      devices: "ethernet:connected:wired",
      wifi: "",
    });
    let mut networks = vec![disconnected, wired];

    for strength in 0..=100 {
      let Ok(wireless) = network(Readings {
        devices: DEVICES,
        wifi: &format!("*:{strength}"),
      });

      networks.push(wireless);
    }

    one_width("the network", networks);

    let Ok(radios) = the_four();
    let bluetooth_readings = radios
      .iter()
      .map(|objects| {
        let Ok(reading) = bluetooth(Some(objects));

        reading
      })
      .collect();

    one_width("bluetooth", bluetooth_readings);
  }

  #[test]
  fn the_ramp_holds_every_charge() {
    for charge in 0..=100 {
      let Ok(reading) = battery(&format!("{charge} Discharging"));

      assert!(LEVELS.contains(&reading.icon.as_str()), "{charge}");
    }
  }

  #[test]
  fn bluetooth_that_is_off_is_not_bluetooth_with_nothing_on_it() {
    let Ok([off_radio, on_radio, one_radio, two_radio]) = the_four();
    let Ok(off) = bluetooth(Some(&off_radio));
    let Ok(on) = bluetooth(Some(&on_radio));
    let Ok(one) = bluetooth(Some(&one_radio));
    let Ok(two) = bluetooth(Some(&two_radio));

    assert_eq!(off.tone, Tone::Secondary);
    assert_eq!(on.tone, Tone::Plain);
    assert_ne!(on.icon, one.icon);
    assert_eq!(one.icon, two.icon);
  }

  #[test]
  fn a_paired_device_that_is_not_here_is_not_a_connection() {
    let Ok(radio) = around(Power::On, &[Connection::NotConnected]);
    let Ok(on) = bluetooth(Some(&radio));

    assert_eq!(on.icon, "\u{f00af}");
  }

  #[test]
  fn a_device_left_connected_under_a_radio_that_is_off_is_still_off() {
    let Ok(radio) = around(Power::Off, &[Connection::Connected]);
    let Ok(off) = bluetooth(Some(&radio));

    assert_eq!(off.tone, Tone::Secondary);
    assert_eq!(off.icon, "\u{f00b2}");
  }

  #[test]
  fn bluetooth_nobody_answered_for_is_drawn_as_off() {
    let Ok(silent) = bluetooth(None);
    let no_radio = Objects {
      adapters: Vec::new(),
      devices: Vec::new(),
    };
    let Ok(radioless) = bluetooth(Some(&no_radio));

    assert_eq!(silent.tone, Tone::Secondary);
    assert_eq!(
      radioless.tone,
      Tone::Secondary,
      "a machine with no radio is not one that is on"
    );
  }

  const DEVICES: &str = "wifi:connected:home\nethernet:unavailable:\nloopback:connected:lo";

  #[test]
  fn the_wireless_this_machine_is_on_is_the_one_with_the_star() {
    let Ok(reading) = network(Readings {
      devices: DEVICES,
      wifi: "*:72\n :41\n :12",
    });

    assert_eq!(reading.tone, Tone::Plain);
    assert_eq!(Some(&reading.icon.as_str()), BARS.get(2));
  }

  #[test]
  fn every_strength_lands_on_a_bar_and_the_ends_are_not_the_same_bar() {
    let Ok(faint) = network(Readings {
      devices: DEVICES,
      wifi: "*:5",
    });
    let Ok(strong) = network(Readings {
      devices: DEVICES,
      wifi: "*:95",
    });

    for strength in 0..=100 {
      let Ok(reading) = network(Readings {
        devices: DEVICES,
        wifi: &format!("*:{strength}"),
      });

      assert!(
        BARS.contains(&reading.icon.as_str()),
        "{strength}: {:?}",
        reading.icon
      );
    }

    assert_ne!(faint.icon, strong.icon);
  }

  #[test]
  fn a_strength_that_cannot_be_read_is_the_faintest_bar() {
    let Ok(unread) = network(Readings {
      devices: DEVICES,
      wifi: "*:",
    });
    let Ok(unsaid) = network(Readings {
      devices: DEVICES,
      wifi: "",
    });

    assert_eq!(Some(&unread.icon.as_str()), BARS.first());
    assert_eq!(Some(&unsaid.icon.as_str()), BARS.first());
  }

  #[test]
  fn a_cable_is_not_a_wireless_and_neither_is_nothing() {
    let both = "ethernet:connected:wired\nwifi:disconnected:";
    let Ok(wired) = network(Readings {
      devices: both,
      wifi: "",
    });
    let Ok(nothing) = network(Readings {
      devices: "wifi:disconnected:",
      wifi: "",
    });

    assert_eq!(wired.tone, Tone::Plain);
    assert_eq!(nothing.tone, Tone::Secondary);
    assert_ne!(wired.icon, nothing.icon);
  }

  #[test]
  fn the_loopback_is_not_a_network() {
    let Ok(reading) = network(Readings {
      devices: "loopback:connected:lo",
      wifi: "",
    });

    assert_eq!(reading.tone, Tone::Secondary);
  }

  #[test]
  fn a_muted_sink_says_so_whatever_its_volume_is() {
    let Ok(muted) = sound("Volume: 0.35 [MUTED]");
    let Ok(heard) = sound("Volume: 0.35");

    assert_eq!(muted.tone, Tone::Secondary);
    assert_eq!(heard.tone, Tone::Plain);
  }

  #[test]
  fn a_volume_of_nothing_is_drawn_as_the_silence_it_is() {
    let Ok(nothing) = sound("Volume: 0.00");
    let Ok(muted) = sound("Volume: 0.35 [MUTED]");

    assert_eq!(nothing.icon, SILENT, "it reading {:?}", nothing.icon);
    assert_eq!(nothing.tone, muted.tone);
  }

  #[test]
  fn the_quietest_reading_that_is_not_silence_is_not_drawn_as_silence() {
    let Ok(quietest) = sound("Volume: 0.01");

    assert_ne!(quietest.icon, SILENT);
  }

  #[test]
  fn the_volume_is_read_as_a_share_and_drawn_as_one_of_three_marks() {
    let Ok(quiet) = sound("Volume: 0.15");
    let Ok(middling) = sound("Volume: 0.50");
    let Ok(loud) = sound("Volume: 1.00");

    for said in [&quiet.icon, &middling.icon, &loud.icon] {
      assert!(
        !said.contains('%'),
        "the number is back on the bar: {said:?}"
      );
    }

    assert_ne!(quiet.icon, middling.icon);
    assert_ne!(middling.icon, loud.icon);
  }

  #[test]
  fn a_level_carries_its_number_and_a_sink_nobody_answered_for_has_no_level() {
    let Ok(half) = level("Volume: 0.50");
    let Ok(muted) = level("Volume: 0.35 [MUTED]");
    let Ok(nothing) = level("");
    let Ok(heard) = sound("Volume: 0.50");

    assert_eq!(
      half.as_ref().and_then(|half_3| half_3.beside.as_deref()),
      Some("50%\u{2007}")
    );
    assert_eq!(half.map(|half_2| half_2.icon), Some(heard.icon));
    assert_eq!(
      muted.map(|muted_2| muted_2.icon),
      Some(String::from(SILENT))
    );
    assert_eq!(nothing, None);
  }
}
