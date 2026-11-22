//! ACPI tables handed over by UEFI. The xHCI driver uses the IVRS requester
//! scopes to determine whether AMD-Vi can translate a controller's DMA.

use core::slice;

const MAX_SIZE: u32 = 65_536;
const HEADER_SIZE: u32 = 36;

#[derive(Clone, Copy)]
pub(super) struct Root(pub(super) u64);

#[derive(Debug)]
pub(super) enum Error {
  Address,
  Checksum,
  Length,
  Signature,
}

impl core::fmt::Display for Error {
  fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    out.write_str(match self {
      Self::Address => "invalid ACPI table address",
      Self::Checksum => "invalid ACPI table checksum",
      Self::Length => "invalid ACPI table length",
      Self::Signature => "invalid ACPI root signature",
    })
  }
}

impl Root {
  pub(super) fn ivrs(self) -> Result<Option<&'static [u8]>, Error> {
    let prefix = bytes(self.0, HEADER_SIZE);
    let rsdp = prefix?;
    let header = rsdp.first_chunk::<36>().ok_or(Error::Length);
    let complete = header?;
    let (revision, length, address) = match *complete {
      [
        b'R',
        b'S',
        b'D',
        b' ',
        b'P',
        b'T',
        b'R',
        b' ',
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        revision,
        _,
        _,
        _,
        _,
        len_a,
        len_b,
        len_c,
        len_d,
        a,
        b,
        c,
        e,
        f,
        g,
        h,
        i,
        _,
        _,
        _,
        _,
      ] => (
        revision,
        u32::from_le_bytes([len_a, len_b, len_c, len_d]),
        u64::from_le_bytes([a, b, c, e, f, g, h, i]),
      ),
      _ => return Err(Error::Signature),
    };

    let short_checksum = rsdp.get(..20).ok_or(Error::Length);
    let legacy = short_checksum?;
    let () = checksum(legacy)?;

    match ::core::primitive::bool::then_some(revision >= 2, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(Error::Signature),
    }

    let full_checksum = bytes(self.0, length);
    let extended = full_checksum?;
    let () = checksum(extended)?;
    let (signature, xsdt) = table(address)?;

    match signature {
      [b'X', b'S', b'D', b'T'] => find_ivrs(xsdt),
      _ => Err(Error::Signature),
    }
  }
}

fn find_ivrs(bytes: &[u8]) -> Result<Option<&'static [u8]>, Error> {
  let body = bytes.get(36..).ok_or(Error::Length);
  let entries = body?;
  let (addresses, tail) = entries.as_chunks::<8>();

  match tail {
    [] => {}
    _ => return Err(Error::Length),
  }

  for encoded in addresses {
    let address = u64::from_le_bytes(*encoded);
    let prefix = bytes_at(address, 4)?;

    match prefix {
      b"IVRS" => {
        let (_, ivrs) = table(address)?;

        return Ok(Some(ivrs));
      }
      _ => {}
    }
  }

  Ok(None)
}

fn table(address: u64) -> Result<([u8; 4], &'static [u8]), Error> {
  let header = bytes_at(address, HEADER_SIZE)?;
  let prefix = header.first_chunk::<8>().ok_or(Error::Length);
  let signature_length = prefix?;
  let [a, b, c, d, low, mid, high, top] = *signature_length;
  let length = u32::from_le_bytes([low, mid, high, top]);

  match ::core::primitive::bool::then_some(length >= HEADER_SIZE, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(Error::Length),
  }

  let data = bytes_at(address, length)?;
  let () = checksum(data)?;

  Ok(([a, b, c, d], data))
}

fn bytes(address: u64, length: u32) -> Result<&'static [u8], Error> {
  match ::core::primitive::bool::then_some(length >= HEADER_SIZE, ()) {
    ::core::option::Option::Some(()) => bytes_at(address, length),
    ::core::option::Option::None => Err(Error::Length),
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the firmware address and validated byte length become a Rust pointer and slice only here"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "the validated range comes from the UEFI configuration table or ACPI SDT, which UEFI keeps mapped and reserves in the memory map"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "this is the boundary that borrows raw firmware memory as a bounded ACPI table"
  )
)]
fn bytes_at(address: u64, length: u32) -> Result<&'static [u8], Error> {
  match ::core::primitive::bool::then_some(address != 0 && address.checked_add(u64::from(length)).is_some() && length <= MAX_SIZE, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(Error::Address),
  }

  let decoded_address = usize::try_from(address).map_err(|_error| Error::Address);
  let base = decoded_address?;
  let decoded_length = usize::try_from(length).map_err(|_error| Error::Length);
  let count = decoded_length?;

  // SAFETY: the validated firmware table serves immutable bytes, identity mapped and kept reserved for the kernel's lifetime.
  Ok(unsafe { slice::from_raw_parts(core::ptr::with_exposed_provenance(base), count) })
}

fn checksum(bytes: &[u8]) -> Result<(), Error> {
  match bytes.iter().copied().fold(0_u8, u8::wrapping_add) {
    0 => Ok(()),
    _ => Err(Error::Checksum),
  }
}
