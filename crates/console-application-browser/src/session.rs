use console_core_number_conversion::fixed_index;
use serde_json::{Map, Value, json};

use crate::renderer::Renderer;
use crate::toolbar::MAX_TABS;

const BLANK: &str = "about:blank";
const MAX_BYTES: u64 = 64 * 1024;
const MAX_ADDRESS: u64 = 8 * 1024;

pub(crate) struct Tab {
  pub renderer: Renderer,
  pub address: String,
}

pub(crate) struct Window {
  pub active: u64,
  pub default_engine: Renderer,
  pub tabs: Vec<Tab>,
}

#[must_use]
fn renderer(name: &str) -> core::result::Result<Option<Renderer>, console_core_never::Never> {
  Ok(match name {
    "WebKit" => Some(Renderer::WebKit),
    "Chromium" => Some(Renderer::Chromium),
    "Servo" => Some(Renderer::Servo),
    _unknown => None,
  })
}

#[must_use]
fn address(text: &str) -> core::result::Result<&str, console_core_never::Never> {
  let Ok(length) = fixed_index(text.len());
  let allowed = text == BLANK
    || text.starts_with("https://")
    || text.starts_with("http://")
    || text.starts_with("file://");

  Ok(
    match bool::then_some(length <= MAX_ADDRESS && allowed, ()) {
      Some(()) => text,
      None => BLANK,
    },
  )
}

#[must_use]
fn tab_from(value: &Value) -> core::result::Result<Option<Tab>, console_core_never::Never> {
  let fields = match value.as_object() {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let saved_engine = match fields.get("engine").and_then(Value::as_str) {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let engine = match {
    let Ok(explicit_value) = renderer(saved_engine);
    explicit_value
  } {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let location = match fields.get("address").and_then(Value::as_str) {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };

  Ok(Some(Tab {
    renderer: engine,
    address: ({
      let Ok(explicit_value) = address(location);
      explicit_value
    })
    .to_owned(),
  }))
}

impl Window {
  #[must_use]
  pub(crate) fn parse(
    said: &str,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(length) = fixed_index(said.len());

    match bool::then_some(length <= MAX_BYTES, ()) {
      Some(()) => {}
      None => return Ok(None),
    }

    let fields = match serde_json::from_str::<Map<String, Value>>(said) {
      Ok(fields) => fields,
      Err(_invalid_session_object) => return Ok(None),
    };
    let saved_tabs = match fields.get("tabs").and_then(Value::as_array) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(limit) = console_core_number_conversion::index(MAX_TABS);
    let tabs = match saved_tabs
      .iter()
      .take(limit)
      .map(|explicit_argument_0| {
        let Ok(explicit_value) = tab_from(explicit_argument_0);
        explicit_value
      })
      .collect::<Option<Vec<_>>>()
    {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let active = match fields.get("active").and_then(Value::as_u64) {
      Some(position) => position,
      None => return Ok(None),
    };
    let Ok(count) = fixed_index(tabs.len());

    match (active < count).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    let saved_default = match fields.get("default").and_then(Value::as_str) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let default_engine = match {
      let Ok(explicit_value) = renderer(saved_default);
      explicit_value
    } {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    Ok(Some(Self {
      active,
      default_engine,
      tabs,
    }))
  }
}

#[must_use]
pub(crate) fn encode(
  default_engine: Renderer,
  active_id: u32,
  mut tabs: Vec<(u32, Renderer, &str)>,
) -> core::result::Result<Option<String>, console_core_never::Never> {
  tabs.sort_unstable_by_key(|(id, _, _)| *id);
  let active = match (0u64..)
    .zip(&tabs)
    .find_map(|(position, (id, _, _))| (*id == active_id).then_some(position))
  {
    Some(position) => position,
    None => return Ok(None),
  };
  let encoded_tabs: Vec<_> = tabs
    .into_iter()
    .map(|(_, engine, location)| {
      let Ok(label) = engine.label();
      json!({ "engine": label, "address": ({ let Ok(explicit_value) = address(location); explicit_value }) })
    })
    .collect();
  let Ok(default) = default_engine.label();
  let text = json!({ "active": active, "default": default, "tabs": encoded_tabs }).to_string();
  let Ok(length) = fixed_index(text.len());

  Ok((length <= MAX_BYTES).then_some(text))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_window_retains_tab_order_and_its_active_tab() {
    let held = ({
      let Ok(explicit_value) = encode(
        Renderer::Chromium,
        9,
        vec![
          (9, Renderer::WebKit, "https://example.org/active"),
          (2, Renderer::Chromium, "https://example.org/first"),
        ],
      );
      explicit_value
    })
    .and_then(|text| {
      let Ok(parse) = Window::parse(&text);

      parse
    });
    assert_eq!(
      held.and_then(|window| {
        let first = window.tabs.first()?;
        Some((window.active, first.address.clone()))
      }),
      Some((1, "https://example.org/first".to_owned()))
    );
  }

  #[test]
  fn older_windows_restore_the_address_from_a_saved_profile_entry() {
    let Ok(held) = Window::parse(
      r#"{"active":0,"default":"Chromium","tabs":[{"engine":"Chromium","address":"https://example.org","profile":"work"}]}"#,
    );
    let restored = held.and_then(|window| window.tabs.into_iter().next().map(|tab| tab.address));
    assert_eq!(restored.as_deref(), Some("https://example.org"));
  }
}
