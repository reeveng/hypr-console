//! xHCI capability, operational and interrupter offsets (chapter 5) and
//! transfer ring encodings (chapter 6). Adapted from nos. See `docs/third-party/nos.txt`.

use console_core_never::Never;

pub const CAP_CAPLENGTH: u64 = 0x00;
pub const CAP_HCSPARAMS1: u64 = 0x04;
pub const CAP_HCSPARAMS2: u64 = 0x08;
pub const CAP_HCCPARAMS1: u64 = 0x10;
pub const CAP_DBOFF: u64 = 0x14;
pub const CAP_RTSOFF: u64 = 0x18;

pub const OP_USBCMD: u64 = 0x00;
pub const OP_USBSTS: u64 = 0x04;
pub const OP_PAGESIZE: u64 = 0x08;
pub const OP_DNCTRL: u64 = 0x14;
pub const OP_CRCR: u64 = 0x18;
pub const OP_DCBAAP: u64 = 0x30;
pub const OP_CONFIG: u64 = 0x38;
pub const OP_PORTSC_BASE: u64 = 0x400;
pub const OP_PORT_REG_SIZE: u64 = 0x10;

pub const USBCMD_RUN: u32 = 1 << 0;
pub const USBCMD_RESET: u32 = 1 << 1;

pub const USBSTS_HALTED: u32 = 1 << 0;
pub const USBSTS_HOST_SYSTEM_ERROR: u32 = 1 << 2;
pub const USBSTS_CONTROLLER_NOT_READY: u32 = 1 << 11;
pub const USBSTS_HOST_CONTROLLER_ERROR: u32 = 1 << 12;

pub const IR_IMAN: u64 = 0x00;
pub const IR_ERSTSZ: u64 = 0x08;
pub const IR_ERSTBA: u64 = 0x10;
pub const IR_ERDP: u64 = 0x18;

pub const IMAN_INTERRUPT_PENDING: u32 = 1 << 0;
pub const ERDP_EVENT_HANDLER_BUSY: u64 = 1 << 3;

pub const PORTSC_CONNECTED: u32 = 1 << 0;
pub const PORTSC_ENABLED: u32 = 1 << 1;
pub const PORTSC_RESET: u32 = 1 << 4;
pub const PORTSC_POWER: u32 = 1 << 9;

pub const PORTSC_SPEED_SHIFT: u32 = 10;
pub const PORTSC_SPEED_MASK: u32 = 0xF;

pub const CRCR_RING_CYCLE_STATE: u64 = 1 << 0;

pub const EP_TYPE_CONTROL: u32 = 4;
pub const EP_TYPE_INTERRUPT_IN: u32 = 7;

pub const TRT_NO_DATA: u32 = 0;
pub const TRT_IN_DATA: u32 = 3;

pub const TRB_NORMAL: u32 = 1;
pub const TRB_SETUP_STAGE: u32 = 2;
pub const TRB_DATA_STAGE: u32 = 3;
pub const TRB_STATUS_STAGE: u32 = 4;
pub const TRB_LINK: u32 = 6;
pub const TRB_ENABLE_SLOT: u32 = 9;
pub const TRB_DISABLE_SLOT: u32 = 10;
pub const TRB_ADDRESS_DEVICE: u32 = 11;
pub const TRB_CONFIGURE_ENDPOINT: u32 = 12;
pub const TRB_EVALUATE_CONTEXT: u32 = 13;
pub const TRB_TRANSFER_EVENT: u32 = 32;
pub const TRB_COMMAND_COMPLETION: u32 = 33;

pub const COMP_SUCCESS: u32 = 1;
pub const COMP_SHORT_PACKET: u32 = 13;

pub const TRB_CYCLE: u32 = 1 << 0;
pub const TRB_ISP: u32 = 1 << 2;
pub const TRB_IOC: u32 = 1 << 5;
pub const TRB_IDT: u32 = 1 << 6;
pub const TRB_TOGGLE_CYCLE: u32 = 1 << 1;
pub const TRB_DIR_IN: u32 = 1 << 16;

pub const fn trb_type_field(kind: u32) -> Result<u32, Never> {
  Ok((kind & 0x3f).wrapping_shl(10))
}

pub const fn trb_type_of(control: u32) -> Result<u32, Never> {
  Ok(control.wrapping_shr(10) & 0x3f)
}

pub fn trb_slot_field(slot: u8) -> Result<u32, Never> {
  Ok(u32::from(slot).wrapping_shl(24))
}

pub const fn trb_slot_of(control: u32) -> Result<u8, Never> {
  let [_, _, _, slot] = control.to_le_bytes();

  Ok(slot)
}

pub const fn trb_endpoint_of(control: u32) -> Result<u8, Never> {
  let [_, _, endpoint, _] = control.to_le_bytes();

  Ok(endpoint & 0x1f)
}

pub const fn trb_completion_of(status: u32) -> Result<u32, Never> {
  Ok(status.wrapping_shr(24) & 0xff)
}

pub const fn trb_residual_of(status: u32) -> Result<u32, Never> {
  Ok(status & 0xff_ffff)
}
