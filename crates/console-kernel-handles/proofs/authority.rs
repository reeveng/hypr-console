//! Run with `cargo kani -p console-kernel-handles --lib`.

use super::{
  Condition, Contains, FrameMapping, HandleError, HandleId, MappingAccess, MemoryAccess, Object,
  ObjectType, Policy, PolicyAction, Right, Rights, authorize_duplicate, check_revocation_target,
  executable_rights,
};

const RIGHTS: [Right; 7] = [
  Right::Read,
  Right::Write,
  Right::Execute,
  Right::Duplicate,
  Right::Transfer,
  Right::ManageJob,
  Right::Destroy,
];

const CONDITIONS: [Condition; 5] = [
  Condition::NewProcess,
  Condition::NewJob,
  Condition::NewChannel,
  Condition::NewMemoryObject,
  Condition::ReplaceAsExecutable,
];

fn rights(selected: [bool; 7]) -> Rights {
  let mut result = Rights::NONE;

  for (right, chosen) in RIGHTS.into_iter().zip(selected) {
    if chosen {
      let Ok(next) = result.union(right);

      result = next;
    }
  }

  result
}

fn deny(policy: Policy, selected: [bool; 5]) -> Policy {
  let mut policy = policy;

  for (condition, chosen) in CONDITIONS.into_iter().zip(selected) {
    if chosen {
      let Ok(next) = policy.deny(&[condition]);

      policy = next;
    }
  }

  policy
}

#[kani::proof]
#[kani::unwind(8)]
fn delegation_requires_duplicate_and_can_only_keep_or_drop_rights() {
  let source: [bool; 7] = kani::any();
  let requested: [bool; 7] = kani::any();
  let [_, _, _, permits_duplicate, _, _, _] = source;
  let source_rights = rights(source);
  let requested_rights = rights(requested);

  for (right, chosen) in RIGHTS.into_iter().zip(requested) {
    let Ok(has_right) = requested_rights.contains(right);

    assert_eq!(has_right == Contains::Yes, chosen);
  }

  let subset = requested
    .into_iter()
    .zip(source)
    .all(|(need, has)| !need || has);
  let expected = match (permits_duplicate, subset) {
    (false, _) => Err(HandleError::AccessDenied(Right::Duplicate)),
    (true, false) => Err(HandleError::InvalidRights),
    (true, true) => Ok(()),
  };

  assert_eq!(
    authorize_duplicate(source_rights, requested_rights),
    expected
  );
}

#[kani::proof]
#[kani::unwind(8)]
fn executable_replacement_keeps_other_rights_but_drops_write() {
  let chosen = kani::any();
  let source = rights(chosen);
  let Ok(replacement) = executable_rights(source);

  for (right, had) in RIGHTS.into_iter().zip(chosen) {
    let Ok(after) = replacement.contains(right);
    let expected = match right {
      Right::Write => false,
      Right::Execute => true,
      _ => had,
    };

    assert_eq!(after == Contains::Yes, expected);
  }
}

#[kani::proof]
#[kani::unwind(7)]
fn each_job_policy_change_preserves_all_existing_denials() {
  let inherited: [bool; 5] = kani::any();
  let added: [bool; 5] = kani::any();
  let parent = deny(Policy::ALLOW_ALL, inherited);
  let child = deny(parent, added);

  for (condition, (initial, extra)) in
    CONDITIONS.into_iter().zip(inherited.into_iter().zip(added))
  {
    let Ok(before) = parent.action(condition);
    let Ok(after) = child.action(condition);

    assert_eq!(before == PolicyAction::Deny, initial);
    assert_eq!(after == PolicyAction::Deny, initial || extra);
  }
}

#[kani::proof]
fn sealing_the_object_blocks_writes_even_if_an_alias_has_the_write_right() {
  let mut state = if kani::any() {
    MemoryAccess::Writable
  } else {
    MemoryAccess::Sealed
  };

  let Ok(()) = state.seal();

  assert_eq!(state, MemoryAccess::Sealed);
  assert_eq!(
    state.permits(Right::Write),
    Err(HandleError::AccessDenied(Right::Write))
  );
  assert_eq!(state.permits(Right::Read), Ok(()));
}

#[kani::proof]
fn read_only_aliases_cannot_authorize_executable_replacement() {
  let sealed: bool = kani::any();
  let write: bool = kani::any();
  let execute: bool = kani::any();
  let state = if sealed {
    MemoryAccess::Sealed
  } else {
    MemoryAccess::Writable
  };
  let Ok(read) = Rights::from_rights(&[Right::Read]);
  let Ok(with_write) = if write {
    read.union(Right::Write)
  } else {
    Ok(read)
  };
  let Ok(held) = if execute {
    with_write.union(Right::Execute)
  } else {
    Ok(with_write)
  };

  assert_eq!(
    state.authorize_executable(held).is_ok(),
    write || (sealed && execute)
  );
}

#[kani::proof]
#[kani::unwind(8)]
fn a_frame_mapping_has_the_objects_exact_address_and_requires_the_minimum_rights() {
  let chosen: [bool; 7] = kani::any();
  let address: u64 = kani::any();
  let mut object = Object::PhysicalFrame(address);
  let writable: bool = kani::any();
  let access = if writable {
    MappingAccess::Write
  } else {
    MappingAccess::Read
  };
  let result = FrameMapping::authorize(&mut object, rights(chosen), access);

  if !chosen[0] {
    assert!(matches!(
      result,
      Err(HandleError::AccessDenied(Right::Read))
    ));
  } else if writable && !chosen[1] {
    assert!(matches!(
      result,
      Err(HandleError::AccessDenied(Right::Write))
    ));
  } else {
    let mapping = result.unwrap();
    assert_eq!(mapping.address(), address);
    assert_eq!(mapping.access(), access);
  }
}

#[kani::proof]
fn a_non_frame_handle_cannot_supply_a_mapping() {
  let mut resource = Object::ExecutableResource;
  let Ok(held) = Rights::from_rights(&[Right::Read, Right::Write]);
  assert!(matches!(
    FrameMapping::authorize(&mut resource, held, MappingAccess::Write),
    Err(HandleError::WrongType(ObjectType::PhysicalFrame))
  ));
}

#[kani::proof]
fn revocation_cannot_remove_a_handle_from_a_mismatched_location() {
  let target: u64 = kani::any();
  let located: u64 = kani::any();
  assert_eq!(
    check_revocation_target(HandleId(target), HandleId(located)).is_ok(),
    target == located
  );
}

// Fixed-size model of the derivation index. Each new handle is a root or
// has one older parent. Moving a handle never changes its derivation ID.
#[kani::proof]
#[kani::unwind(7)]
fn revocation_selects_only_strict_descendants_in_tables_or_messages() {
  const COUNT: usize = 5;
  const ROOT: usize = COUNT;
  let parents: [usize; COUNT] = kani::any();
  let locations: [u8; COUNT] = kani::any();
  let origin: usize = kani::any();
  kani::assume(origin < COUNT);

  for (id, parent) in parents.iter().copied().enumerate() {
    kani::assume(parent == ROOT || parent < id);
    kani::assume(locations[id] <= 2); // retained lineage, table, or queued
  }

  let mut branch = [false; COUNT];
  let mut removed = [false; COUNT];
  for (id, parent) in parents.iter().copied().enumerate() {
    branch[id] = parent != ROOT && (parent == origin || branch[parent]);
    removed[id] = branch[id] && locations[id] != 0;
  }

  for id in 0..COUNT {
    let mut ancestor = parents[id];
    let mut descended = false;
    while ancestor != ROOT {
      descended |= ancestor == origin;
      ancestor = parents[ancestor];
    }
    assert_eq!(branch[id], descended);
    assert_eq!(removed[id], descended && locations[id] != 0);
  }
  assert!(!removed[origin]);
}
