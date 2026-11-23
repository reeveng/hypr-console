//! A polled xHCI boot-keyboard driver for QEMU and AMD Phoenix.
//!
//! The ring setup, controller commands and USB enumeration are adapted from
//! the nos USB driver. See xhci/NOS-LICENSE. AMD-Vi grants physical USB
//! controllers DMA access to the pages they were allocated at boot.
//! Enumeration releases each rejected device before resetting the next port,
//! so one slot is enough. Startup enables bus mastering before writing the
//! event segment table base, which can trigger a DMA read immediately.

mod legacy;
mod pci;
mod regs;
mod ring;

use core::arch::x86_64::_rdtsc;
use core::fmt;
use core::mem::offset_of;
use core::ptr;

use console_core_never::Never;

use console_kernel::usb_keyboard::{BootReport, Endpoint};

use crate::{acpi, amd_vi, ivrs};

use self::pci::{Function, Master, Scope};
use self::regs::*;
use self::ring::{Mapping, Quadword, RING_LENGTH, Ring, Trb, read32, write32};

const PAGE_BYTES: u64 = 4096;
const BAR_BYTES: u64 = 0x4000;
const FIXED_DMA_BYTES: u64 = 40_960;
const _: [(); 40_960] = [(); size_of::<Dma>()];
const TIMEOUT_CYCLES: u64 = 6_000_000_000;

const PORT_WRITE_ACTIONS: u32 = (1 << 1) | PORTSC_RESET | (1 << 16) | 0x00fe_0000 | (1 << 31);

#[repr(C, align(4096))]
struct Page([u8; 4096]);

#[repr(C)]
struct Dma {
  context_addresses: Page,
  commands: Page,
  events: Page,
  segments: Page,
  input_context: Page,
  device_context: Page,
  control_transfer: Page,
  reports: Page,
  buffer: Page,
  scratch_addresses: Page,
}

impl Dma {
  const CONTEXT_ADDRESSES: u64 =
    u64::from_le_bytes(offset_of!(Self, context_addresses).to_le_bytes());
  const COMMANDS: u64 = u64::from_le_bytes(offset_of!(Self, commands).to_le_bytes());
  const EVENTS: u64 = u64::from_le_bytes(offset_of!(Self, events).to_le_bytes());
  const SEGMENTS: u64 = u64::from_le_bytes(offset_of!(Self, segments).to_le_bytes());
  const INPUT_CONTEXT: u64 = u64::from_le_bytes(offset_of!(Self, input_context).to_le_bytes());
  const DEVICE_CONTEXT: u64 = u64::from_le_bytes(offset_of!(Self, device_context).to_le_bytes());
  const CONTROL_TRANSFER: u64 =
    u64::from_le_bytes(offset_of!(Self, control_transfer).to_le_bytes());
  const REPORTS: u64 = u64::from_le_bytes(offset_of!(Self, reports).to_le_bytes());
  const BUFFER: u64 = u64::from_le_bytes(offset_of!(Self, buffer).to_le_bytes());
  const SCRATCH_ADDRESSES: u64 =
    u64::from_le_bytes(offset_of!(Self, scratch_addresses).to_le_bytes());
}

#[derive(Debug)]
pub(super) enum Error {
  Registers,
  Allocation,
  Timeout(&'static str),
  Controller,
  Completion(&'static str, u32),
  Descriptor,
  Disconnected,
  Acpi(acpi::Error),
  Ivrs(ivrs::Error),
  Dma(amd_vi::Error),
  TooManyControllers,
  InputQueue,
}

impl fmt::Display for Error {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Registers => out.write_str("unsupported controller registers"),
      Self::Allocation => out.write_str("unable to reserve identity-mapped DMA pages"),
      Self::Timeout(at) => write!(out, "timed out during {at}"),
      Self::Controller => out.write_str("the controller reported a host error"),
      Self::Completion(at, code) => write!(out, "{at} returned xHCI completion {code}"),
      Self::Descriptor => out.write_str("the USB device returned an invalid descriptor"),
      Self::Disconnected => out.write_str("the USB keyboard disconnected"),
      Self::Acpi(error) => write!(out, "{error}"),
      Self::Ivrs(error) => write!(out, "{error}"),
      Self::Dma(error) => write!(out, "{error}"),
      Self::TooManyControllers => {
        out.write_str("too many xHCI controllers for the diagnostic preview")
      }
      Self::InputQueue => out.write_str("the diagnostic keyboard queue rejected an event"),
    }
  }
}

struct Controller {
  pci: Function,
  operational: u64,
  interrupter: u64,
  doorbells: u64,
  ports: u8,
  context_size: u64,
  dma: Mapping,
  scratchpads: u16,
  commands: Ring,
  events: Ring,
  control: Ring,
  reports: Ring,
}

#[derive(Clone, Copy)]
struct Device {
  port: u8,
  speed: u8,
  slot: u8,
}

pub(super) struct Keyboard {
  controller: Controller,
  platform: Platform,
  device: Device,
  dci: u8,
  pending: u64,
}

#[derive(Clone, Copy)]
pub(super) enum Machine {
  Virtual,
  Physical,
}

enum Platform {
  Remapped {
    unit: ivrs::Unit<'static>,
    active: Option<amd_vi::Remapper>,
  },
  Virtual,
  Unavailable,
}

impl Platform {
  fn new(root: Option<acpi::Root>, machine: Machine) -> Result<Self, Error> {
    let fallback = match machine {
      Machine::Virtual => Self::Virtual,
      Machine::Physical => Self::Unavailable,
    };
    let acpi_root = match root {
      Some(value) => value,
      None => return Ok(fallback),
    };
    let reported = acpi_root.ivrs().map_err(Error::Acpi);
    let tables = reported?;
    let table = match tables {
      Some(table) => table,
      None => return Ok(fallback),
    };
    let routing = ivrs::Ivrs::new(table).map_err(Error::Ivrs);
    let ivrs = routing?;
    let topology = ivrs.unit().map_err(Error::Ivrs);
    let selected = topology?;

    Ok(match selected {
      Some(unit) => Self::Remapped { unit, active: None },
      None => fallback,
    })
  }

  fn candidate(&self, pci: Function) -> Result<ivrs::Coverage, Error> {
    let Ok(requester) = pci.requester();

    match self {
      Self::Remapped { unit, .. } => unit.covers(requester).map_err(Error::Ivrs),
      Self::Virtual => Ok(ivrs::Coverage::Assigned),
      Self::Unavailable => Ok(ivrs::Coverage::Skipped),
    }
  }

  fn map(&mut self, pci: Function, pages: amd_vi::Pages) -> Result<Mapping, Error> {
    let Ok(requester) = pci.requester();
    let cpu = pages.base;
    let bus = match self {
      Self::Remapped { unit, active, .. } => {
        let remapper = match active {
          Some(remapper) => remapper,
          None => {
            let started = amd_vi::Remapper::start(*unit).map_err(Error::Dma);
            let remapper = started?;

            active.insert(remapper)
          }
        };
        let mapped = remapper.grant(requester, pages).map_err(Error::Dma);
        let amd_vi::Mapped(bus) = mapped?;

        bus
      }
      Self::Virtual => pages.base,
      Self::Unavailable => return Err(Error::Registers),
    };

    Ok(Mapping { cpu, bus })
  }

  fn revoke(&mut self) -> Result<(), Error> {
    match self {
      Self::Remapped {
        active: Some(unit), ..
      } => unit.revoke().map_err(Error::Dma),
      Self::Remapped { active: None, .. } | Self::Virtual | Self::Unavailable => Ok(()),
    }
  }
}

pub(super) fn open(
  root: Option<acpi::Root>,
  machine: Machine,
) -> Result<Option<Keyboard>, Error> {
  let mut platform = Platform::new(root, machine)?;
  let scope = match platform {
    Platform::Remapped { .. } => Scope::Remapped,
    Platform::Virtual => Scope::Emulated,
    Platform::Unavailable => return Ok(None),
  };
  let discovered = pci::controllers(scope).map_err(|_excess| Error::TooManyControllers);
  let controllers = discovered?;

  let Ok(functions) = controllers.all();

  for pci in functions {
    let coverage = platform.candidate(pci)?;

    match coverage {
      ivrs::Coverage::Assigned => {}
      ivrs::Coverage::Skipped => continue,
    }

    let opened = Controller::open(pci, &mut platform);
    let mut controller = match opened {
      Ok(controller) => controller,
      Err(error) => {
        let () = platform.revoke()?;

        return Err(error);
      }
    };
    let found = controller.keyboard();

    match found {
      Ok(Some((device, endpoint))) => {
        let mut keyboard = Keyboard {
          controller,
          platform,
          device,
          dci: endpoint.dci,
          pending: 0,
        };
        let Ok(()) = keyboard.request();

        return Ok(Some(keyboard));
      }
      Ok(None) => {
        let () = release(controller, &mut platform)?;
      }
      Err(error) => {
        let () = release(controller, &mut platform)?;

        return Err(error);
      }
    }
  }

  Ok(None)
}

fn release(controller: Controller, platform: &mut Platform) -> Result<(), Error> {
  let halted = controller.halt();
  drop(controller);
  let revoked = platform.revoke();
  let () = halted?;

  revoked
}

struct Geometry {
  base: u64,
  operational: u64,
  interrupter: u64,
  doorbells: u64,
  parameters: u32,
  ports: u8,
  scratchpads: u16,
}

impl Geometry {
  fn probe(pci: Function) -> Result<Self, Error> {
    let Ok(assigned) = pci.bar();
    let address = assigned.ok_or(Error::Registers)?;
    let Ok(()) = pci.memory_space();
    let Ok(header) = read32(address.saturating_add(CAP_CAPLENGTH));
    let [length, _, _, _] = header.to_le_bytes();
    let Ok(topology) = read32(address.saturating_add(CAP_HCSPARAMS1));
    let [slots, _, _, ports] = topology.to_le_bytes();
    let Ok(parameters) = read32(address.saturating_add(CAP_HCCPARAMS1));
    let Ok(secondary) = read32(address.saturating_add(CAP_HCSPARAMS2));
    let count = (secondary.wrapping_shr(21) & 0x1f).wrapping_shl(5) | secondary.wrapping_shr(27);
    let scratch_count = u16::try_from(count).map_err(|_too_many| Error::Registers);
    let scratchpads = scratch_count?;
    let Ok(doorbell_offset) = read32(address.saturating_add(CAP_DBOFF));
    let doorbells = u64::from(doorbell_offset & !3);
    let Ok(runtime_offset) = read32(address.saturating_add(CAP_RTSOFF));
    let runtime = u64::from(runtime_offset & !0x1f);
    let interrupter = runtime.saturating_add(32);
    let registers_used = u64::from(length)
      .saturating_add(0x400)
      .saturating_add(u64::from(ports).saturating_mul(16));

    match ::core::primitive::bool::then_some(
      length >= 0x20
        && slots != 0
        && ports != 0
        && scratchpads <= 480
        && registers_used <= BAR_BYTES
        && doorbells.saturating_add(8) <= BAR_BYTES
        && interrupter.saturating_add(32) <= BAR_BYTES,
      (),
    ) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(Error::Registers),
    }

    let operational = address.saturating_add(u64::from(length));
    let Ok(page_sizes) = read32(operational.saturating_add(OP_PAGESIZE));

    match page_sizes & 1 {
      0 => return Err(Error::Registers),
      _ => {}
    }

    Ok(Self {
      base: address,
      operational,
      interrupter: address.saturating_add(interrupter),
      doorbells: address.saturating_add(doorbells),
      parameters,
      ports,
      scratchpads,
    })
  }
}

impl Controller {
  fn keyboard(&mut self) -> Result<Option<(Device, Endpoint)>, Error> {
    for port in 1..=self.ports {
      let device = self.enumerate(port)?;

      match device {
        Some(keyboard) => return Ok(Some(keyboard)),
        None => {}
      }
    }

    Ok(None)
  }

  fn open(pci: Function, platform: &mut Platform) -> Result<Self, Error> {
    let properties = Geometry::probe(pci)?;
    let ownership = legacy::Capabilities {
      base: properties.base,
      parameters: properties.parameters,
    };
    let () = ownership.claim()?;
    let Ok(()) = pci.bus_master(Master::Disabled);
    let bytes =
      FIXED_DMA_BYTES.saturating_add(u64::from(properties.scratchpads).saturating_mul(4096));
    let memory = amd_vi::reserve(bytes).map_err(Error::Dma);
    let physical = memory?;
    let dma = platform.map(
      pci,
      amd_vi::Pages {
        base: physical,
        bytes,
      },
    )?;

    match ::core::primitive::bool::then_some(
      properties.parameters & 1 == 0
        && dma.bus.saturating_add(bytes.saturating_sub(1)) > u64::from(u32::MAX),
      (),
    ) {
      ::core::option::Option::Some(()) => return Err(Error::Allocation),
      ::core::option::Option::None => {}
    }

    let context_size = match properties.parameters & 4 {
      0 => 32,
      _ => 64,
    };
    let Ok(command_region) = dma.at(Dma::COMMANDS);
    let Ok(event_region) = dma.at(Dma::EVENTS);
    let Ok(control_region) = dma.at(Dma::CONTROL_TRANSFER);
    let Ok(report_region) = dma.at(Dma::REPORTS);
    let Ok(commands) = Ring::new(command_region);
    let Ok(events) = Ring::new(event_region);
    let Ok(control) = Ring::new(control_region);
    let Ok(reports) = Ring::new(report_region);
    let controller = Self {
      pci,
      operational: properties.operational,
      interrupter: properties.interrupter,
      doorbells: properties.doorbells,
      ports: properties.ports,
      context_size,
      dma,
      scratchpads: properties.scratchpads,
      commands,
      events,
      control,
      reports,
    };

    match controller.start() {
      Ok(()) => Ok(controller),
      Err(error) => {
        let Ok(()) = pci.bus_master(Master::Disabled);

        Err(error)
      }
    }
  }

  fn start(&self) -> Result<(), Error> {
    let command = self.operational.saturating_add(OP_USBCMD);
    let status = self.operational.saturating_add(OP_USBSTS);
    let ready = Condition {
      register: status,
      mask: USBSTS_CONTROLLER_NOT_READY,
      expected: 0,
    };
    let halted = Condition {
      register: status,
      mask: USBSTS_HALTED,
      expected: USBSTS_HALTED,
    };
    wait("controller ready", &[ready])?;
    let Ok(()) = write32(command, 0);
    wait("controller halt", &[halted])?;
    let Ok(()) = write32(command, USBCMD_RESET);
    let reset = Condition {
      register: command,
      mask: USBCMD_RESET,
      expected: 0,
    };
    wait("controller reset", &[reset, ready])?;

    let Ok(()) = write32(self.operational.saturating_add(OP_CONFIG), 1);
    let Ok(()) = write32(self.operational.saturating_add(OP_DNCTRL), 0);
    let Ok(contexts) = self.dma.at(Dma::CONTEXT_ADDRESSES);
    let Ok(()) = Quadword(self.operational.saturating_add(OP_DCBAAP)).write(contexts.bus);
    let Ok(()) = self.prepare_scratchpads();
    let Ok(command_head) = self.commands.dequeue();
    let Ok(()) = Quadword(self.operational.saturating_add(OP_CRCR))
      .write(command_head | CRCR_RING_CYCLE_STATE);
    let Ok(segment) = self.dma.at(Dma::SEGMENTS);
    let Ok(event_head) = self.events.dequeue();
    let Ok(()) = Quadword(segment.cpu).write(event_head);
    let Ok(()) = write32(segment.cpu.saturating_add(8), RING_LENGTH);
    let Ok(()) = write32(self.interrupter.saturating_add(IR_ERSTSZ), 1);
    let Ok(()) = Quadword(self.interrupter.saturating_add(IR_ERDP)).write(event_head);
    let Ok(()) = self.pci.bus_master(Master::Enabled);
    let Ok(()) = Quadword(self.interrupter.saturating_add(IR_ERSTBA)).write(segment.bus);
    let Ok(()) = write32(
      self.interrupter.saturating_add(IR_IMAN),
      IMAN_INTERRUPT_PENDING,
    );
    let Ok(()) = write32(command, USBCMD_RUN);
    let running = Condition {
      register: status,
      mask: USBSTS_HALTED,
      expected: 0,
    };

    wait("controller start", &[running])
  }

  fn prepare_scratchpads(&self) -> Result<(), Never> {
    match self.scratchpads {
      0 => return Ok(()),
      _ => {}
    }

    let Ok(addresses) = self.dma.at(Dma::SCRATCH_ADDRESSES);
    let Ok(contexts) = self.dma.at(Dma::CONTEXT_ADDRESSES);
    let Ok(pages) = self.dma.at(FIXED_DMA_BYTES);

    for index in 0..self.scratchpads {
      let Ok(entry) = addresses.at(u64::from(index).saturating_mul(8));
      let address = pages
        .bus
        .saturating_add(u64::from(index).saturating_mul(PAGE_BYTES));
      let Ok(()) = Quadword(entry.cpu).write(address);
    }

    Quadword(contexts.cpu).write(addresses.bus)
  }

  fn halt(&self) -> Result<(), Error> {
    let Ok(()) = write32(self.operational.saturating_add(OP_USBCMD), 0);
    let halted = Condition {
      register: self.operational.saturating_add(OP_USBSTS),
      mask: USBSTS_HALTED,
      expected: USBSTS_HALTED,
    };
    let stopped = wait("USB DMA stop", &[halted]);
    let Ok(()) = self.pci.bus_master(Master::Disabled);

    stopped
  }

  fn ring_doorbell(&self, pipe: Pipe) -> Result<(), Never> {
    let address = self
      .doorbells
      .saturating_add(u64::from(pipe.slot).saturating_mul(4));

    write32(address, u32::from(pipe.endpoint))
  }

  fn next_event(&mut self) -> Result<Option<Trb>, Error> {
    let Ok(status) = read32(self.operational.saturating_add(OP_USBSTS));

    match status & (USBSTS_HOST_SYSTEM_ERROR | USBSTS_HOST_CONTROLLER_ERROR) {
      0 => {}
      _ => return Err(Error::Controller),
    }

    let Ok(event) = self.events.pop();

    match event {
      Some(_) => {
        let Ok(head) = self.events.dequeue();
        let Ok(()) = Quadword(self.interrupter.saturating_add(IR_ERDP))
          .write(head | ERDP_EVENT_HANDLER_BUSY);
        let Ok(()) = write32(
          self.interrupter.saturating_add(IR_IMAN),
          IMAN_INTERRUPT_PENDING,
        );
      }
      None => {}
    }

    Ok(event)
  }

  fn command(
    &mut self,
    name: &'static str,
    kind: u32,
    slot: u8,
    context: u64,
  ) -> Result<u8, Error> {
    let Ok(command) = trb_type_field(kind);
    let Ok(target) = trb_slot_field(slot);
    let Ok([address]) = self.commands.append([Trb {
      parameter: context,
      status: 0,
      control: command | target,
    }]);
    let Ok(()) = self.ring_doorbell(Pipe {
      slot: 0,
      endpoint: 0,
    });
    let Ok(started) = timestamp();

    for _poll in 0..TIMEOUT_CYCLES {
      let next = self.next_event()?;

      let completion = command_completion(next, address, name)?;

      match completion {
        Some(assigned) => return Ok(assigned),
        None => {}
      }

      let Ok(now) = timestamp();

      match ::core::primitive::bool::then_some(now.wrapping_sub(started) >= TIMEOUT_CYCLES, ()) {
        ::core::option::Option::Some(()) => break,
        ::core::option::Option::None => core::hint::spin_loop(),
      }
    }

    Err(Error::Timeout(name))
  }

  fn request(&mut self, device: Device, request: [u8; 8]) -> Result<u32, Error> {
    let [_, _, _, _, _, _, low, high] = request;
    let length = u32::from(u16::from_le_bytes([low, high]));
    let pipe = Pipe {
      slot: device.slot,
      endpoint: 1,
    };
    let Ok(transfer) = self.control_transfer(request, length);
    let Ok(()) = self.ring_doorbell(pipe);

    self.await_control(pipe, transfer)
  }

  fn control_transfer(
    &mut self,
    request: [u8; 8],
    length: u32,
  ) -> Result<ControlTransfer, Never> {
    let Ok(setup_stage) = trb_type_field(TRB_SETUP_STAGE);
    let Ok(status_stage) = trb_type_field(TRB_STATUS_STAGE);
    let transfer_type = match length {
      0 => TRT_NO_DATA,
      _ => TRT_IN_DATA,
    };
    let setup = Trb {
      parameter: u64::from_le_bytes(request),
      status: 8,
      control: setup_stage | TRB_IDT | transfer_type.wrapping_shl(16),
    };

    Ok(match length {
      0 => {
        let status = Trb {
          parameter: 0,
          status: 0,
          control: status_stage | TRB_IOC | TRB_DIR_IN,
        };
        let Ok([_, last]) = self.control.append([setup, status]);

        ControlTransfer {
          data: None,
          last,
          length,
        }
      }
      _ => {
        let Ok(buffer) = self.dma.at(Dma::BUFFER);
        let Ok(data_stage) = trb_type_field(TRB_DATA_STAGE);
        let data = Trb {
          parameter: buffer.bus,
          status: length,
          control: data_stage | TRB_DIR_IN | TRB_ISP,
        };
        let status = Trb {
          parameter: 0,
          status: 0,
          control: status_stage | TRB_IOC,
        };
        let Ok([_, data_address, last]) = self.control.append([setup, data, status]);

        ControlTransfer {
          data: Some(data_address),
          last,
          length,
        }
      }
    })
  }

  fn await_control(&mut self, pipe: Pipe, transfer: ControlTransfer) -> Result<u32, Error> {
    let Ok(started) = timestamp();
    let mut transferred = transfer.length;

    for _poll in 0..TIMEOUT_CYCLES {
      let next = self.next_event()?;

      let progress = transfer.observe(pipe, next)?;

      match progress {
        ControlProgress::Waiting => {}
        ControlProgress::Short(residual) => transferred = transferred.saturating_sub(residual),
        ControlProgress::Complete => return Ok(transferred),
      }

      let Ok(now) = timestamp();

      match ::core::primitive::bool::then_some(now.wrapping_sub(started) >= TIMEOUT_CYCLES, ()) {
        ::core::option::Option::Some(()) => break,
        ::core::option::Option::None => core::hint::spin_loop(),
      }
    }

    Err(Error::Timeout("control transfer"))
  }

  fn port(&self, port: u8) -> Result<u64, Never> {
    let offset = u64::from(port.saturating_sub(1)).saturating_mul(OP_PORT_REG_SIZE);

    Ok(
      self
        .operational
        .saturating_add(OP_PORTSC_BASE)
        .saturating_add(offset),
    )
  }

  fn enumerate(&mut self, port: u8) -> Result<Option<(Device, Endpoint)>, Error> {
    let resumed = self.reset_port(port)?;
    let speed = match resumed {
      Some(speed) => speed,
      None => return Ok(None),
    };
    let packet = match speed {
      1 | 2 => 8,
      3 => 64,
      _ => return Ok(None),
    };
    let slot = self.command("enable slot", TRB_ENABLE_SLOT, 0, 0)?;

    match slot {
      1 => {}
      _ => return Err(Error::Controller),
    }

    let device = Device { port, speed, slot };
    self.address_device(device, packet)?;
    let selected = self.keyboard_endpoint(device)?;
    let endpoint = match selected {
      Some(endpoint) => endpoint,
      None => {
        let _disabled = self.command("disable slot", TRB_DISABLE_SLOT, slot, 0)?;

        return Ok(None);
      }
    };
    self.configure_keyboard(device, endpoint)?;

    Ok(Some((device, endpoint)))
  }

  fn reset_port(&self, port: u8) -> Result<Option<u8>, Error> {
    let Ok(reg) = self.port(port);
    let Ok(before) = read32(reg);
    let Ok(speed) = port_speed(before);

    match ::core::primitive::bool::then_some(before & PORTSC_CONNECTED != 0 && speed <= 3, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(None),
    }

    let Ok(()) = write32(
      reg,
      (before & !PORT_WRITE_ACTIONS) | PORTSC_POWER | PORTSC_RESET,
    );
    let reset = Condition {
      register: reg,
      mask: PORTSC_RESET,
      expected: 0,
    };
    wait("USB root port reset", &[reset])?;
    let Ok(after) = read32(reg);
    let Ok(reported_speed) = port_speed(after);
    let connected = PORTSC_CONNECTED | PORTSC_ENABLED;

    Ok(
      match ::core::primitive::bool::then_some(after & connected == connected, ()) {
        ::core::option::Option::Some(()) => Some(reported_speed),
        ::core::option::Option::None => None,
      },
    )
  }

  fn address_device(&mut self, device: Device, packet: u16) -> Result<(), Error> {
    let Ok(output) = self.dma.at(Dma::DEVICE_CONTEXT);
    let Ok(input) = self.dma.at(Dma::INPUT_CONTEXT);
    let Ok(ring) = self.dma.at(Dma::CONTROL_TRANSFER);
    let Ok(()) = clear_page(output.cpu);
    let Ok(()) = clear_page(ring.cpu);
    let Ok(control) = Ring::new(ring);
    self.control = control;
    let Ok(contexts) = self.dma.at(Dma::CONTEXT_ADDRESSES);
    let Ok(device_context) = contexts.at(u64::from(device.slot).saturating_mul(8));
    let Ok(()) = Quadword(device_context.cpu).write(output.bus);
    let Ok(()) = self.slot_context(
      device,
      SlotContext {
        last_endpoint: 1,
        updates: 0b11,
      },
    );
    let endpoint_offset = self.context_size.saturating_mul(2);
    let Ok(()) = self.endpoint_context(EndpointContext {
      offset: endpoint_offset,
      kind: EndpointKind::Control,
      packet,
      interval: 0,
      dequeue: ring.bus,
    });
    let _addressed =
      self.command("address device", TRB_ADDRESS_DEVICE, device.slot, input.bus)?;

    let mut descriptor = [0; 8];
    self.descriptor(device, 1, &mut descriptor)?;
    let [_, _, _, _, _, _, _, reported] = descriptor;
    let actual = u16::from(reported);

    match actual {
      8 | 16 | 32 | 64 => {}
      _ => return Err(Error::Descriptor),
    }

    match ::core::primitive::bool::then_some(actual == packet, ()) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => {
        let Ok(()) = clear_page(input.cpu);
        let Ok(()) = write32(input.cpu.saturating_add(4), 1_u32.wrapping_shl(1));
        let at = input.cpu.saturating_add(endpoint_offset).saturating_add(4);
        let Ok(()) = write32(at, u32::from(actual).wrapping_shl(16));
        let _evaluated = self.command(
          "update control packet size",
          TRB_EVALUATE_CONTEXT,
          device.slot,
          input.bus,
        )?;

        Ok(())
      }
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the bounded total length from the USB header becomes a slice index"
    )
  )]
  fn keyboard_endpoint(&mut self, device: Device) -> Result<Option<Endpoint>, Error> {
    let mut config = [0; 512];
    let header = config.first_chunk_mut::<9>().ok_or(Error::Descriptor)?;
    self.descriptor(device, 2, header)?;
    let [_, _, low, high, ..] = *header;
    let size = u16::from_le_bytes([low, high]);

    match ::core::primitive::bool::then_some(size >= 9, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(Error::Descriptor),
    }

    let data = config
      .get_mut(..usize::from(size))
      .ok_or(Error::Descriptor)?;
    self.descriptor(device, 2, data)?;
    let Ok(endpoint) = Endpoint::from_configuration(data);

    Ok(endpoint)
  }

  fn configure_keyboard(&mut self, device: Device, endpoint: Endpoint) -> Result<(), Error> {
    let Ok(input) = self.dma.at(Dma::INPUT_CONTEXT);
    let context_at = self
      .context_size
      .saturating_mul(u64::from(endpoint.dci).saturating_add(1));
    let Ok(()) = self.slot_context(
      device,
      SlotContext {
        last_endpoint: endpoint.dci,
        updates: 1 | 1_u32.wrapping_shl(u32::from(endpoint.dci)),
      },
    );
    let interval = match device.speed {
      3 => endpoint.interval.clamp(1, 16).saturating_sub(1),
      _ => {
        let value = u32::from(endpoint.interval.max(1))
          .saturating_mul(8)
          .ilog2()
          .min(10);
        let [first, _, _, _] = value.to_le_bytes();

        first
      }
    };
    let Ok(head) = self.reports.dequeue();
    let Ok(()) = self.endpoint_context(EndpointContext {
      offset: context_at,
      kind: EndpointKind::InterruptIn,
      packet: endpoint.max_packet,
      interval,
      dequeue: head,
    });
    let _configured = self.command(
      "configure keyboard endpoint",
      TRB_CONFIGURE_ENDPOINT,
      device.slot,
      input.bus,
    )?;
    let _selected = self.request(device, [0, 9, endpoint.configuration, 0, 0, 0, 0, 0])?;
    let _boot_protocol = self.request(device, [0x21, 0x0b, 0, 0, endpoint.interface, 0, 0, 0])?;

    Ok(())
  }

  fn descriptor(&mut self, device: Device, kind: u8, dest: &mut [u8]) -> Result<(), Error> {
    let size = u16::try_from(dest.len()).map_err(|_wide| Error::Descriptor)?;
    let [low, high] = size.to_le_bytes();
    let received = self.request(device, [0x80, 6, 0, kind, 0, 0, low, high])?;

    match ::core::primitive::bool::then_some(received == u32::from(size), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(Error::Descriptor),
    }

    let Ok(buffer) = self.dma.at(Dma::BUFFER);
    let Ok(()) = copy_buffer(buffer.cpu, dest);

    Ok(())
  }

  fn slot_context(&self, device: Device, context: SlotContext) -> Result<(), Never> {
    let Ok(input) = self.dma.at(Dma::INPUT_CONTEXT);
    let Ok(()) = clear_page(input.cpu);
    let Ok(()) = write32(input.cpu.saturating_add(4), context.updates);
    let slot = input.cpu.saturating_add(self.context_size);
    let speed = u32::from(device.speed).wrapping_shl(20);
    let entries = u32::from(context.last_endpoint).wrapping_shl(27);
    let Ok(()) = write32(slot, speed | entries);

    write32(
      slot.saturating_add(4),
      u32::from(device.port).wrapping_shl(16),
    )
  }

  fn endpoint_context(&self, endpoint: EndpointContext) -> Result<(), Never> {
    let Ok(context) = self.dma.at(Dma::INPUT_CONTEXT);
    let at = context.cpu.saturating_add(endpoint.offset);
    let packet = u32::from(endpoint.packet);
    let (kind, transfer) = match endpoint.kind {
      EndpointKind::Control => (EP_TYPE_CONTROL, 8),
      EndpointKind::InterruptIn => (EP_TYPE_INTERRUPT_IN, packet | packet.wrapping_shl(16)),
    };
    let Ok(()) = write32(at, u32::from(endpoint.interval).wrapping_shl(16));
    let Ok(()) = write32(
      at.saturating_add(4),
      packet.wrapping_shl(16) | kind.wrapping_shl(3) | 3_u32.wrapping_shl(1),
    );
    let Ok(()) = Quadword(at.saturating_add(8)).write(endpoint.dequeue | 1);

    write32(at.saturating_add(16), transfer)
  }
}

struct SlotContext {
  last_endpoint: u8,
  updates: u32,
}

enum EndpointKind {
  Control,
  InterruptIn,
}

struct EndpointContext {
  offset: u64,
  kind: EndpointKind,
  packet: u16,
  interval: u8,
  dequeue: u64,
}

struct ControlTransfer {
  data: Option<u64>,
  last: u64,
  length: u32,
}

enum ControlProgress {
  Waiting,
  Short(u32),
  Complete,
}

impl ControlTransfer {
  fn observe(&self, pipe: Pipe, next: Option<Trb>) -> Result<ControlProgress, Error> {
    let event = match next {
      Some(event) => event,
      None => return Ok(ControlProgress::Waiting),
    };
    let Ok(relation) = pipe.accepts(event);

    match relation {
      Relation::Same => {}
      Relation::Other => return Ok(ControlProgress::Waiting),
    }

    let Ok(code) = trb_completion_of(event.status);

    match (
      code,
      self.data == Some(event.parameter),
      event.parameter == self.last,
    ) {
      (COMP_SHORT_PACKET, true, _) => {
        let Ok(residual) = trb_residual_of(event.status);

        Ok(ControlProgress::Short(residual))
      }
      (COMP_SUCCESS, _, true) => Ok(ControlProgress::Complete),
      (COMP_SUCCESS, _, false) => Ok(ControlProgress::Waiting),
      (error, _, _) => Err(Error::Completion("control transfer", error)),
    }
  }
}

fn command_completion(
  next: Option<Trb>,
  address: u64,
  name: &'static str,
) -> Result<Option<u8>, Error> {
  let event = match next {
    Some(event) => event,
    None => return Ok(None),
  };
  let Ok(kind) = trb_type_of(event.control);

  match ::core::primitive::bool::then_some(
    kind == TRB_COMMAND_COMPLETION && event.parameter == address,
    (),
  ) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(None),
  }

  let Ok(code) = trb_completion_of(event.status);
  let Ok(assigned) = trb_slot_of(event.control);

  match code {
    COMP_SUCCESS => Ok(Some(assigned)),
    other => Err(Error::Completion(name, other)),
  }
}

#[derive(Clone, Copy)]
struct Pipe {
  slot: u8,
  endpoint: u8,
}

enum Relation {
  Same,
  Other,
}

impl Pipe {
  fn accepts(self, event: Trb) -> Result<Relation, Never> {
    let Ok(kind) = trb_type_of(event.control);
    let Ok(slot) = trb_slot_of(event.control);
    let Ok(endpoint) = trb_endpoint_of(event.control);

    Ok(
      match ::core::primitive::bool::then_some(
        kind == TRB_TRANSFER_EVENT && slot == self.slot && endpoint == self.endpoint,
        (),
      ) {
        ::core::option::Option::Some(()) => Relation::Same,
        ::core::option::Option::None => Relation::Other,
      },
    )
  }
}

impl Drop for Controller {
  fn drop(&mut self) {
    let Ok(()) = write32(self.operational.saturating_add(OP_USBCMD), 0);
    let Ok(()) = self.pci.bus_master(Master::Disabled);
  }
}

pub(super) enum DmaMode {
  AmdVi,
  Direct,
}

impl Keyboard {
  pub(super) fn dma_mode(&self) -> Result<DmaMode, Never> {
    Ok(match self.platform {
      Platform::Remapped { .. } => DmaMode::AmdVi,
      Platform::Virtual | Platform::Unavailable => DmaMode::Direct,
    })
  }

  pub(super) fn close(self) -> Result<(), Error> {
    let Self {
      controller,
      mut platform,
      ..
    } = self;

    release(controller, &mut platform)
  }

  fn request(&mut self) -> Result<(), Never> {
    let Ok(buffer) = self.controller.dma.at(Dma::BUFFER);
    let Ok(normal) = trb_type_field(TRB_NORMAL);
    let Ok([address]) = self.controller.reports.append([Trb {
      parameter: buffer.bus,
      status: BootReport::BYTES,
      control: normal | TRB_IOC | TRB_ISP,
    }]);
    self.pending = address;

    self.controller.ring_doorbell(Pipe {
      slot: self.device.slot,
      endpoint: self.dci,
    })
  }

  pub(super) fn poll(&mut self) -> Result<Option<[u8; BootReport::LENGTH]>, Error> {
    let Ok(port) = self.controller.port(self.device.port);
    let Ok(state) = read32(port);

    match state & PORTSC_CONNECTED {
      0 => return Err(Error::Disconnected),
      _ => {}
    }

    let pipe = Pipe {
      slot: self.device.slot,
      endpoint: self.dci,
    };

    for _event in 0..RING_LENGTH {
      let queued = self.controller.next_event()?;
      let event = match queued {
        Some(event) => event,
        None => return Ok(None),
      };
      let Ok(relation) = pipe.accepts(event);

      match (relation, event.parameter == self.pending) {
        (Relation::Same, true) => {}
        (Relation::Other, _) | (Relation::Same, false) => continue,
      }

      let Ok(code) = trb_completion_of(event.status);

      match code {
        COMP_SUCCESS | COMP_SHORT_PACKET => {}
        error => return Err(Error::Completion("keyboard report", error)),
      }

      let Ok(residual) = trb_residual_of(event.status);
      let report = match residual {
        0 => {
          let mut report = [0; BootReport::LENGTH];
          let Ok(buffer) = self.controller.dma.at(Dma::BUFFER);
          let Ok(()) = copy_buffer(buffer.cpu, &mut report);

          Some(report)
        }
        _ => None,
      };
      let Ok(()) = self.request();

      return Ok(report);
    }

    Ok(None)
  }
}

fn port_speed(value: u32) -> Result<u8, Never> {
  let [speed, ..] = (value.wrapping_shr(PORTSC_SPEED_SHIFT) & PORTSC_SPEED_MASK).to_le_bytes();

  Ok(speed)
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the DMA page address becomes a Rust pointer only for the page write"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "clearing an inactive DMA page requires writing its owned raw memory"
  )
)]
fn clear_page(address: u64) -> Result<(), Never> {
  let pointer = usize::from_le_bytes(address.to_le_bytes());

  // SAFETY: only unused or disabled device pages and the input context
  // after command completion are cleared. Each region is a full DMA page.
  unsafe { ptr::with_exposed_provenance_mut::<u8>(pointer).write_bytes(0, 4096) };

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the DMA address becomes a Rust pointer only for the volatile reads"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "copying a completed DMA report uses volatile reads from the controller's owned window"
  )
)]
fn copy_buffer(address: u64, dest: &mut [u8]) -> Result<(), Never> {
  for (byte_address, out) in (address..).zip(dest) {
    let at = usize::from_le_bytes(byte_address.to_le_bytes());
    // SAFETY: completion was acquired from the event ring before this read.
    // The buffer has a whole page and the caller requests at most 512 bytes.
    *out = unsafe { ptr::with_exposed_provenance::<u8>(at).read_volatile() };
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the TSC instruction supplies a deadline on this boot CPU"
  )
)]
fn timestamp() -> Result<u64, Never> {
  // SAFETY: all polling runs on the boot CPU and the x86-64 target has a TSC.
  Ok(unsafe { _rdtsc() })
}

#[derive(Clone, Copy)]
struct Condition {
  register: u64,
  mask: u32,
  expected: u32,
}

fn wait(stage: &'static str, conditions: &[Condition]) -> Result<(), Error> {
  let Ok(started) = timestamp();

  for _poll in 0..TIMEOUT_CYCLES {
    let ready = conditions.iter().all(|wanted| {
      let Ok(value) = read32(wanted.register);

      value & wanted.mask == wanted.expected
    });

    match ::core::primitive::bool::then_some(ready, ()) {
      ::core::option::Option::Some(()) => return Ok(()),
      ::core::option::Option::None => {}
    }

    let Ok(now) = timestamp();

    match ::core::primitive::bool::then_some(now.wrapping_sub(started) >= TIMEOUT_CYCLES, ()) {
      ::core::option::Option::Some(()) => break,
      ::core::option::Option::None => core::hint::spin_loop(),
    }
  }

  Err(Error::Timeout(stage))
}
