//! Run with `cargo kani -p console-kernel --lib --harness frame_mappings::proofs`.

use super::{
  FrameAccess, FrameGrant, InstalledFrame, InvalidFrame, NewBranch, UserBranchError, UserPage,
  clear_frame, supervisor_entry, user_branch, user_leaf,
};

fn legal_address(physical: u64) -> bool {
  physical != 0 && physical.trailing_zeros() >= 12 && physical < (1_u64 << 52)
}

#[kani::proof]
fn inherited_mappings_always_require_supervisor_privilege() {
  let firmware: u64 = kani::any();
  let Ok(imported) = supervisor_entry(firmware);

  assert_eq!(imported & 0b100, 0);
  assert_eq!(imported | 0b100, firmware | 0b100);
}

#[kani::proof]
fn a_user_branch_cannot_replace_a_present_entry() {
  let inherited: u64 = kani::any();
  let child: u64 = kani::any();
  let Ok(reserved) = supervisor_entry(inherited);

  match user_branch(NewBranch {
    previous: reserved,
    table: child,
  }) {
    Ok(entry) => {
      assert_eq!(inherited & 1, 0);
      assert!(legal_address(child));
      assert_eq!(entry, child | 0b111);
    }
    Err(UserBranchError::Occupied) => {
      assert_ne!(inherited & 1, 0);
    }
    Err(UserBranchError::InvalidFrame) => {
      assert_eq!(inherited & 1, 0);
      assert!(!legal_address(child));
    }
  }
}

#[kani::proof]
fn no_user_leaf_is_both_writable_and_executable() {
  let physical: u64 = kani::any();
  let code: bool = kani::any();
  let write: bool = kani::any();
  let page = match (code, write) {
    (true, _) => UserPage::Code,
    (false, true) => UserPage::Data(FrameAccess::Write),
    (false, false) => UserPage::Data(FrameAccess::Read),
  };

  match user_leaf(physical, page) {
    Ok(entry) => {
      let writable = entry & 0b010 != 0;
      let executable = entry >> 63 == 0;

      assert!(legal_address(physical));
      assert_eq!(entry & 0x000f_ffff_ffff_f000, physical);
      assert_eq!(entry & 0xffd, 0b101);
      assert_eq!(entry >> 52, if code { 0 } else { 0x800 });
      assert_eq!(writable, !code && write);
      assert_eq!(executable, code);
      assert!(!(writable && executable));
    }
    Err(InvalidFrame) => {
      assert!(!legal_address(physical));
    }
  }
}

#[kani::proof]
fn every_frame_address_gets_only_the_requested_permissions() {
  let physical: u64 = kani::any();
  let writable: bool = kani::any();
  let access = if writable {
    FrameAccess::Write
  } else {
    FrameAccess::Read
  };
  let legal = legal_address(physical);

  match FrameGrant::new(physical, access) {
    Ok(grant) => {
      let mut aliases = kani::any();
      let Ok(()) = grant.install(&mut aliases);
      let [first, second] = aliases;
      assert!(legal);
      assert_eq!(first, second);
      assert_eq!(first & 0x000f_ffff_ffff_f000, physical);
      assert_eq!(first & 0xfff, if writable { 0b111 } else { 0b101 });
      assert_eq!(first >> 52, 0x800);
    }
    Err(InvalidFrame) => {
      assert!(!legal);
    }
  }
}

#[kani::proof]
fn revocation_removes_every_alias_and_the_old_grant_cannot_restore_it() {
  let physical: u64 = kani::any();
  let access = if kani::any() {
    FrameAccess::Write
  } else {
    FrameAccess::Read
  };
  let Ok(mut grant) = FrameGrant::new(physical, access) else {
    return;
  };
  let mut aliases: [u64; 2] = kani::any();

  let Ok(()) = grant.revoke(&mut aliases);
  assert_eq!(aliases, [0, 0]);

  let mut next_table = kani::any();
  let Ok(()) = grant.install(&mut next_table);
  assert_eq!(next_table, [0, 0]);

  let Ok(()) = grant.revoke(&mut aliases);
  assert_eq!(aliases, [0, 0]);
}

#[kani::proof]
fn scoped_mapping_installs_exact_aliases_and_clears_them_on_drop() {
  let physical: u64 = kani::any();
  kani::assume(legal_address(physical));
  let write: bool = kani::any();
  let access = if write {
    FrameAccess::Write
  } else {
    FrameAccess::Read
  };
  let grant = FrameGrant::new(physical, access).unwrap();
  let mut aliases: [u64; 2] = kani::any();
  let entry = physical | (1 << 63) | if write { 0b111 } else { 0b101 };

  {
    let Ok(mapping) = InstalledFrame::new(grant, &mut aliases);
    assert_eq!(*mapping.entries, [entry; 2]);
  }

  assert_eq!(aliases, [0; 2]);
}

#[kani::proof]
fn scoped_mapping_clears_aliases_on_success_and_error() {
  let physical: u64 = kani::any();
  kani::assume(legal_address(physical));
  let write: bool = kani::any();
  let access = if write {
    FrameAccess::Write
  } else {
    FrameAccess::Read
  };
  let grant = FrameGrant::new(physical, access).unwrap();
  let mut aliases: [u64; 2] = kani::any();
  let outcome: Result<u64, u64> = if kani::any() {
    Ok(kani::any())
  } else {
    Err(kani::any())
  };
  let mut calls = 0;

  let Ok(result) = grant.with_installed(&mut aliases, || {
    calls += 1;
    outcome
  });

  assert_eq!(calls, 1);
  assert_eq!(result, outcome);
  assert_eq!(aliases, [0; 2]);
  kani::cover!(outcome.is_ok());
  kani::cover!(outcome.is_err());
}

#[kani::proof]
fn clearing_a_frame_erases_every_old_word() {
  let index: usize = kani::any();
  kani::assume(index < 1024);
  let mut words: [u32; 1024] = kani::any();

  let Ok(()) = clear_frame(&mut words);

  assert_eq!(words[index], 0);
}
