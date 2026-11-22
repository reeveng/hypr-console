use crate::Visibility;
use console_core_portable_document_syntax::object::dict::keys::{
  BASE_STATE, D, OCGS, OCMD, OCPROPERTIES, OFF, ON, P, TYPE,
};
use console_core_portable_document_syntax::object::{Array, Dict, Name, ObjectIdentifier};
use std::collections::HashSet;

pub(crate) struct OcgState {
  inactive_ocgs: HashSet<ObjectIdentifier>,
  visibility_stack: Vec<Visibility>,
}

fn group_ids(groups: &Array<'_>) -> Result<Vec<ObjectIdentifier>, console_core_never::Never> {
  let Ok(items) = groups.raw_iter();

  Ok(
    items
      .filter_map(|item| {
        let Ok(reference) = item.as_obj_ref();
        reference.map(ObjectIdentifier::from)
      })
      .collect(),
  )
}

fn membership_ids(ocmd: &Dict<'_>) -> Result<Vec<ObjectIdentifier>, console_core_never::Never> {
  let Ok(groups) = ocmd.get::<Array<'_>>(OCGS);

  match groups {
    Some(groups_2) => group_ids(&groups_2),
    None => {
      let Ok(reference) = ocmd.get_ref(OCGS);
      Ok(reference.into_iter().map(ObjectIdentifier::from).collect())
    }
  }
}

fn set_group_visibility(
  inactive: &mut HashSet<ObjectIdentifier>,
  groups: Option<Array<'_>>,
  visibility: Visibility,
) -> Result<(), console_core_never::Never> {
  match groups {
    Some(groups_2) => {
      let Ok(ids) = group_ids(&groups_2);

      for id in ids {
        match visibility {
          Visibility::Visible => {
            inactive.remove(&id);
          }
          Visibility::Hidden => {
            inactive.insert(id);
          }
        }
      }
    }
    None => {}
  }

  Ok(())
}

impl OcgState {
  fn dummy() -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      inactive_ocgs: HashSet::default(),
      visibility_stack: vec![],
    })
  }

  pub(crate) fn from_catalog(
    catalog: &Dict<'_>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(optional_content_entry) = catalog.get::<Dict<'_>>(OCPROPERTIES);
    let oc_properties = match optional_content_entry {
      Some(oc_properties) => oc_properties,
      None => {
        let Ok(dummy) = Self::dummy();
        return Ok(dummy);
      }
    };

    let Ok(oc_property) = oc_properties.get::<Dict<'_>>(D);
    let config = match oc_property {
      Some(config) => config,
      None => {
        let Ok(dummy) = Self::dummy();
        return Ok(dummy);
      }
    };

    let mut inactive = HashSet::new();

    let base_state = ({
      let Ok(explicit_value) = config.get::<Name<'_>>(BASE_STATE);
      explicit_value
    })
    .and_then(|b| {
      let Ok(explicit_value) = BaseState::from_name(b.as_ref());
      explicit_value
    });

    let default_hidden_groups = match base_state {
      Some(BaseState::Off) => {
        let Ok(groups) = oc_properties.get::<Array<'_>>(OCGS);
        groups
      }
      Some(BaseState::On | BaseState::Unchanged) | None => None,
    };

    let Ok(()) = set_group_visibility(&mut inactive, default_hidden_groups, Visibility::Hidden);
    let Ok(visible) = config.get::<Array<'_>>(ON);
    let Ok(hidden) = config.get::<Array<'_>>(OFF);
    let Ok(()) = set_group_visibility(&mut inactive, visible, Visibility::Visible);
    let Ok(()) = set_group_visibility(&mut inactive, hidden, Visibility::Hidden);

    Ok(Self {
      inactive_ocgs: inactive,
      visibility_stack: Vec::new(),
    })
  }

  pub(crate) fn begin_single_oc(
    &mut self,
    ocg_id: ObjectIdentifier,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(parent) = self.is_visible();
      let visibility = match parent {
        Visibility::Visible => Visibility::from(!self.inactive_ocgs.contains(&ocg_id)),
        Visibility::Hidden => Visibility::Hidden,
      };
      self.visibility_stack.push(visibility);
    };

    Ok(())
  }

  pub(crate) fn begin_ocmd(
    &mut self,
    ocmd: &Dict<'_>,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let policy = match ({
        let Ok(explicit_value) = ocmd.get::<Name<'_>>(P);
        explicit_value
      })
      .and_then(|n| {
        let Ok(ocmd_policy) = OcmdPolicy::from_name(n.as_ref());

        ocmd_policy
      }) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => OcmdPolicy::AnyOn,
      };

      let Ok(ocg_ids) = membership_ids(ocmd);

      let visibility = match ocg_ids.as_slice() {
        [] => Visibility::Visible,
        ids => match policy {
          OcmdPolicy::AllOn => {
            Visibility::from(ids.iter().all(|id| !self.inactive_ocgs.contains(id)))
          }
          OcmdPolicy::AnyOn => {
            Visibility::from(ids.iter().any(|id| !self.inactive_ocgs.contains(id)))
          }
          OcmdPolicy::AnyOff => {
            Visibility::from(ids.iter().any(|id| self.inactive_ocgs.contains(id)))
          }
          OcmdPolicy::AllOff => {
            Visibility::from(ids.iter().all(|id| self.inactive_ocgs.contains(id)))
          }
        },
      };

      let Ok(parent) = self.is_visible();
      self.visibility_stack.push(match parent {
        Visibility::Visible => visibility,
        Visibility::Hidden => Visibility::Hidden,
      });
    };

    Ok(())
  }

  pub(crate) fn begin_ocg(
    &mut self,
    props: &Dict<'_>,
    ref_id: ObjectIdentifier,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(prop) = props.get::<Name<'_>>(TYPE);

      match prop.as_deref() {
        Some(OCMD) => {
          let Ok(begin_ocmd) = self.begin_ocmd(props);

          begin_ocmd
        }
        ::core::option::Option::None | ::core::option::Option::Some(_) => {
          let Ok(begin_single_oc) = self.begin_single_oc(ref_id);

          begin_single_oc
        }
      }
    };

    Ok(())
  }

  pub(crate) fn begin_marked_content(
    &mut self,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(visibility) = self.is_visible();
      self.visibility_stack.push(visibility);
    };

    Ok(())
  }

  pub(crate) fn end_marked_content(
    &mut self,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      self.visibility_stack.pop();
    };

    Ok(())
  }

  pub(crate) fn set_visibility(
    &mut self,
    id: ObjectIdentifier,
    visibility: Visibility,
  ) -> core::result::Result<(), console_core_never::Never> {
    match visibility {
      Visibility::Visible => {
        self.inactive_ocgs.remove(&id);
      }
      Visibility::Hidden => {
        self.inactive_ocgs.insert(id);
      }
    };

    Ok(())
  }

  pub(crate) fn is_visible(
    &self,
  ) -> core::result::Result<crate::Visibility, console_core_never::Never> {
    Ok(match self.visibility_stack.last().copied() {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => Visibility::Visible,
    })
  }
}

impl Default for OcgState {
  fn default() -> Self {
    let Ok(dummy) = Self::dummy();

    dummy
  }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum BaseState {
  On,
  Off,
  Unchanged,
}

impl BaseState {
  fn from_name(name: &[u8]) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(match name {
      b"ON" => Some(Self::On),
      b"OFF" => Some(Self::Off),
      b"Unchanged" => Some(Self::Unchanged),
      _ => None,
    })
  }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum OcmdPolicy {
  AllOn,
  AnyOn,
  AnyOff,
  AllOff,
}

impl OcmdPolicy {
  fn from_name(name: &[u8]) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(match name {
      b"AllOn" => Some(Self::AllOn),
      b"AnyOn" => Some(Self::AnyOn),
      b"AnyOff" => Some(Self::AnyOff),
      b"AllOff" => Some(Self::AllOff),
      _ => None,
    })
  }
}
