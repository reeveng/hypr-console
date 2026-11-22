#[path = "../src/ivrs.rs"]
mod ivrs;

use ivrs::{Coverage, Error, Ivrs, Requester};

const HEADER: [u8; 48] = [0; 48];
const UNIT: [u8; 24] = [
  0x10, 0, 40, 0, 2, 0, 0, 0, 0, 0, 0xd8, 0xfe, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[test]
fn the_scope_admits_a_bridge_requester_and_leaves_aliases_alone() -> Result<(), Error> {
  let direct = [0x03, 0, 1, 0, 0x04, 0xff, 1, 0];
  let aliased = [0x42, 0, 2, 0, 0, 1, 2, 0];
  let table = [
    HEADER.as_slice(),
    UNIT.as_slice(),
    direct.as_slice(),
    aliased.as_slice(),
  ]
  .concat();
  let ivrs = Ivrs::new(&table)?;
  let unit = ivrs.unit()?;
  let selected = unit.ok_or(Error::Length)?;
  let bridged = selected.covers(Requester(0x108))?;
  let alias = selected.covers(Requester(0x201))?;
  let unlisted = selected.covers(Requester(0x208))?;
  assert_eq!(selected.registers, 0xfed8_0000);
  assert!(bridged == Coverage::Assigned);
  assert!(alias == Coverage::Skipped);
  assert!(unlisted == Coverage::Skipped);

  Ok(())
}

#[test]
fn firmware_dma_reservations_prevent_replacing_the_device_table() -> Result<(), Error> {
  let reservation = [0x20, 1, 8, 0, 0, 0, 0, 0];
  let table = [HEADER.as_slice(), reservation.as_slice()].concat();
  let ivrs = Ivrs::new(&table)?;
  assert!(matches!(ivrs.unit(), Err(Error::FirmwareMemory)));

  Ok(())
}
