//! Match windows by their launch tag. Older sessions and windows managed by
//! a shared process still have class, title and workspace as a fallback.

use std::collections::{BTreeMap, BTreeSet};

use console_compositor::{Window, setup::SessionClient};
use console_core_state_machine::{EffectDependencies, Machine, Never, Queue, Service};

use super::{Differs, is_same_client};

pub(super) struct Restoration<'a>(std::marker::PhantomData<&'a ()>);

#[derive(Clone, Copy)]
pub(super) struct Targets<'a> {
  pub windows: &'a [Window],
  pub tags: &'a BTreeMap<String, String>,
}

pub(super) struct Claims<'a> {
  targets: Targets<'a>,
  pub placed: BTreeMap<String, String>,
}

pub(super) struct Observed(pub Vec<SessionClient>);

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Effect<'a> {
  Observe,
  Place { real: Window, saved: &'a Window },
}

impl EffectDependencies for Effect<'_> {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Observe | Self::Place { .. } => &[Service::Compositor],
    })
  }
}

impl<'a> Machine for Restoration<'a> {
  type Input = Targets<'a>;
  type State = Claims<'a>;
  type Request = Observed;
  type Effect = Effect<'a>;

  fn initialize(
    targets: &Targets<'a>,
    _previous: Option<Self::State>,
    effects: &mut Queue<Effect<'a>>,
  ) -> Result<Self::State, Never> {
    match { ::core::primitive::bool::then_some(!targets.windows.is_empty(), ()) } {
      ::core::option::Option::Some(()) => {
        let Ok(()) = effects.offer(Effect::Observe);
      }
      ::core::option::Option::None => {}
    }

    Ok(Claims {
      targets: *targets,
      placed: BTreeMap::new(),
    })
  }

  fn handle(
    mut claims: Self::State,
    Observed(clients): Observed,
    effects: &mut Queue<Effect<'a>>,
  ) -> Result<Self::State, Never> {
    let visible: BTreeSet<_> = clients
      .iter()
      .map(|client| client.window.address.as_str())
      .collect();
    claims
      .placed
      .retain(|_, address| visible.contains(address.as_str()));
    let mut used_addresses: BTreeSet<_> = claims.placed.values().cloned().collect();

    for client in clients {
      match used_addresses
        .contains(&client.window.address)
        .then_some(())
      {
        Some(()) => continue,
        None => {}
      }

      let Ok(found) = claims.find(&client);

      match found {
        Some(saved) => {
          let address = client.window.address.clone();
          let _ = used_addresses.insert(address.clone());
          let _ = claims.placed.insert(saved.address.clone(), address);
          let Ok(()) = effects.offer(Effect::Place {
            real: client.window,
            saved,
          });
        }
        None => {}
      }
    }

    match {
      ::core::primitive::bool::then_some(claims.placed.len() < claims.targets.windows.len(), ())
    } {
      ::core::option::Option::Some(()) => {
        let Ok(()) = effects.offer(Effect::Observe);
      }
      ::core::option::Option::None => {}
    }

    Ok(claims)
  }
}

impl<'a> Claims<'a> {
  fn find(&self, client: &SessionClient) -> Result<Option<&'a Window>, Never> {
    let tags: BTreeSet<_> = client
      .tags
      .iter()
      .map(|tag| tag.trim_end_matches('*'))
      .collect();
    let claimed_tag = self
      .targets
      .tags
      .iter()
      .any(|(address, tag)| !self.placed.contains_key(address) && tags.contains(tag.as_str()));
    let unclaimed = self
      .targets
      .windows
      .iter()
      .filter(|window| !self.placed.contains_key(&window.address));
    let best = unclaimed
      .filter_map(|saved| {
        match self.targets.tags.get(&saved.address) {
          Some(tag) => return tags.contains(tag.as_str()).then_some((4, saved)),
          None => {}
        }

        let Ok(same) = is_same_client(saved, &client.window);

        match (same == Differs::No && !claimed_tag).then_some(()) {
          Some(()) => {}
          None => return None,
        }

        let in_workspace = saved.workspace_named == client.window.workspace_named;
        let titled = saved.title == client.window.title;
        let score = u8::from(in_workspace)
          .saturating_mul(2)
          .saturating_add(u8::from(titled));
        Some((score, saved))
      })
      .max_by_key(|(score, _)| *score)
      .map(|(_, saved)| saved);

    Ok(best)
  }
}
