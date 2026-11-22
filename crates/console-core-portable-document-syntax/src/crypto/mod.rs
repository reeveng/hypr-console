//! Cryptographic implementations for console_core_portable_document_rasterizer, ported from pdf.js.
//!
//! **Important note**: Please keep in mind that these haven't been
//! audited and should not be used for security-critical purposes, like creating new
//! encrypted PDFs. They solely serve the purpose of being able to decrypt and read
//! _already_ encrypted documents, where security isn't really relevant.

use crate::crypto::DecryptionError::InvalidEncryption;
use crate::crypto::aes::{AES128Cipher, AES256Cipher, Padding};
use crate::crypto::rc4::Rc4;
use crate::object;
use crate::object::dict::keys::{
  CF, CFM, ENCRYPT_META_DATA, FILTER, LENGTH, O, OE, P, R, STM_F, STR_F, U, UE, V,
};
use crate::object::{Dict, Name, ObjectIdentifier, PdfBoolean};
use crate::sync::HashMap;
use alloc::string::ToString;
use alloc::vec::Vec;
use console_core_number_conversion::index;
use core::ops::Deref;

mod aes;
mod md5;
mod rc4;
mod sha256;
mod sha384;
mod sha512;

const PASSWORD_PADDING: [u8; 32] = [
  0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01, 0x08,
  0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53, 0x69, 0x7A,
];

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DecryptionError {
  MissingIDEntry,

  PasswordProtected,

  InvalidEncryption,

  UnsupportedAlgorithm,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum DecryptorTag {
  None,
  Rc4,
  Aes128,
  Aes256,
}

impl DecryptorTag {
  fn from_name(name: &Name<'_>) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(explicit_value) = name.as_str();

    Ok(match explicit_value {
      "None" | "Identity" => Some(Self::None),
      "V2" => Some(Self::Rc4),
      "AESV2" => Some(Self::Aes128),
      "AESV3" => Some(Self::Aes256),
      _ => None,
    })
  }
}

#[derive(Debug, Clone)]
pub(crate) enum Decryptor {
  None,
  Rc4 { key: Vec<u8> },
  Aes128 { key: Vec<u8>, dict: DecryptorData },
  Aes256 { key: Vec<u8>, dict: DecryptorData },
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum DecryptionTarget {
  String,
  Stream,
}

impl Decryptor {
  pub(crate) fn decrypt(
    &self,
    id: ObjectIdentifier,
    data: &[u8],
    target: DecryptionTarget,
  ) -> core::result::Result<Option<Vec<u8>>, console_core_never::Never> {
    Ok(match self {
      Self::None => Some(data.to_vec()),
      Self::Rc4 { key } => {
        let Ok(explicit_value) = decrypt_rc4(key, data, id);
        explicit_value
      }
      Self::Aes128 { key, dict } | Self::Aes256 { key, dict } => {
        let crypt_dict = match target {
          DecryptionTarget::String => dict.string_filter,
          DecryptionTarget::Stream => dict.stream_filter,
        };

        match crypt_dict.cfm {
          DecryptorTag::None => Some(data.to_vec()),
          DecryptorTag::Rc4 => {
            let Ok(explicit_value) = decrypt_rc4(key, data, id);
            explicit_value
          }
          DecryptorTag::Aes128 => {
            let Ok(explicit_value) = decrypt_aes128(key, data, id);
            explicit_value
          }
          DecryptorTag::Aes256 => {
            let Ok(explicit_value) = decrypt_aes256(key, data);
            explicit_value
          }
        }
      }
    })
  }
}

#[derive(Clone, Copy)]
pub(crate) struct Password<'a>(pub(crate) &'a [u8]);

pub(crate) struct RetainedPassword(pub(crate) Vec<u8>);

impl Drop for RetainedPassword {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "password erasure must use volatile writes so release builds preserve the stores"
    )
  )]
  fn drop(&mut self) {
    for byte in &mut self.0 {
      // SAFETY: each byte is borrowed exclusively from the live allocation until the write completes.
      unsafe { core::ptr::write_volatile(byte, 0) };
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Encryption dictionary references are resolved by the indirect reader with bounded ancestry"
  )
)]
pub(crate) fn get(
  dict: &Dict<'_>,
  id: &[u8],
  password: Password<'_>,
) -> Result<Decryptor, DecryptionError> {
  let filter = ({
    let Ok(explicit_value) = dict.get::<Name<'_>>(FILTER);
    explicit_value
  })
  .ok_or(InvalidEncryption)?;

  match ::core::primitive::bool::then_some(filter.deref() != b"Standard", ()) {
    ::core::option::Option::Some(()) => {
      return Err(DecryptionError::UnsupportedAlgorithm);
    }
    ::core::option::Option::None => {}
  }

  let encryption_v = ({
    let Ok(explicit_value) = dict.get::<u8>(V);
    explicit_value
  })
  .ok_or(InvalidEncryption)?;
  let Ok(encrypt_metadata) = dict.get::<PdfBoolean>(ENCRYPT_META_DATA);
  let metadata = match encrypt_metadata {
    Some(PdfBoolean::False) => MetadataEncryption::Plain,
    Some(PdfBoolean::True) | None => MetadataEncryption::Encrypted,
  };
  let revision = ({
    let Ok(explicit_value) = dict.get::<u8>(R);
    explicit_value
  })
  .ok_or(InvalidEncryption)?;
  let length = match encryption_v {
    1 => 40,
    2 => {
      let Ok(declared_length) = dict.get::<u16>(LENGTH);

      match declared_length {
        Some(bits) => bits,
        None => 40,
      }
    }
    4 => {
      let Ok(declared_length) = dict.get::<u16>(LENGTH);

      match declared_length {
        Some(bits) => bits,
        None => 128,
      }
    }
    5 => 256,
    _ => return Err(DecryptionError::UnsupportedAlgorithm),
  };

  enum Algorithm {
    LegacyRc4,
    Aes128(DecryptorData),
    Aes256(DecryptorData),
  }

  let algorithm = match encryption_v {
    1 | 2 => Algorithm::LegacyRc4,
    4 => {
      let Ok(filters) = DecryptorData::from_dict(dict, length);
      let crypt_filters = filters.ok_or(InvalidEncryption)?;
      Algorithm::Aes128(crypt_filters)
    }
    5 => {
      let Ok(filters) = DecryptorData::from_dict(dict, length);
      let crypt_filters = filters.ok_or(InvalidEncryption)?;
      Algorithm::Aes256(crypt_filters)
    }
    _ => return Err(DecryptionError::UnsupportedAlgorithm),
  };

  let byte_length = ::core::primitive::u16::wrapping_div(length, 8);

  match ::core::primitive::bool::then_some(byte_length == 0, ()) {
    ::core::option::Option::Some(()) => {
      return Err(InvalidEncryption);
    }
    ::core::option::Option::None => {}
  }

  let owner_string = ({
    let Ok(explicit_value) = dict.get::<object::String<'_>>(O);
    explicit_value
  })
  .ok_or(InvalidEncryption)?;
  let user_string = ({
    let Ok(explicit_value) = dict.get::<object::String<'_>>(U);
    explicit_value
  })
  .ok_or(InvalidEncryption)?;
  let permissions = {
    let raw = ({
      let Ok(explicit_value) = dict.get::<i64>(P);
      explicit_value
    })
    .ok_or(InvalidEncryption)?;

    match ::core::primitive::bool::then_some(raw < 0, ()) {
      ::core::option::Option::Some(()) => u32::from_be_bytes(
        ({
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(raw);
          numeric_value
        })
        .to_be_bytes(),
      ),
      ::core::option::Option::None => {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(raw);
        numeric_value
      }
    }
  };

  let mut decryption_key = match ::core::primitive::bool::then_some(revision <= 4, ()) {
    ::core::option::Option::Some(()) => {
      let key = decryption_key_rev1234(
        password,
        metadata,
        revision,
        byte_length,
        &owner_string,
        permissions,
        id,
      )?;
      authenticate_user_password_rev234(revision, &key, id, &user_string)?;

      key
    }
    ::core::option::Option::None => {
      decryption_key_rev56(dict, revision, password, &owner_string, &user_string)?
    }
  };

  match ::core::primitive::bool::then_some(encryption_v == 4 && decryption_key.len() < 16, ()) {
    ::core::option::Option::Some(()) => {
      decryption_key.resize(16, 0);
    }
    ::core::option::Option::None => {}
  }

  Ok(match algorithm {
    Algorithm::LegacyRc4 => Decryptor::Rc4 {
      key: decryption_key,
    },
    Algorithm::Aes128(filters) => Decryptor::Aes128 {
      key: decryption_key,
      dict: filters,
    },
    Algorithm::Aes256(filters) => Decryptor::Aes256 {
      key: decryption_key,
      dict: filters,
    },
  })
}

fn decrypt_aes256(
  key: &[u8],
  data: &[u8],
) -> core::result::Result<Option<Vec<u8>>, console_core_never::Never> {
  let (iv, data_2) = match data.split_at_checked(16) {
    Some(value) => value,
    None => return Ok(None),
  };
  let iv_2: [u8; 16] = match match iv.try_into() {
    ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
    ::core::result::Result::Err(_wrong_slice_length) => ::core::option::Option::None,
  } {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(explicit_value) = AES256Cipher::new(key);
  let cipher = match explicit_value {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(decrypt_cbc) = cipher.decrypt_cbc(data_2, &iv_2, Padding::Strip);

  Ok(Some(decrypt_cbc))
}

fn decrypt_aes128(
  key: &[u8],
  data: &[u8],
  id: ObjectIdentifier,
) -> core::result::Result<Option<Vec<u8>>, console_core_never::Never> {
  Ok({
    let Ok(explicit_value) = decrypt_rc_aes(key, id, ObjectCipher::Aes, |key_2| {
      let Ok(aes128cipher) = AES128Cipher::new(key_2);
      let cipher = aes128cipher?;
      let (iv, data_2) = data.split_at_checked(16)?;
      let iv_2: [u8; 16] = match iv.try_into() {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_wrong_slice_length) => ::core::option::Option::None,
      }?;

      let Ok(decrypt_cbc) = cipher.decrypt_cbc(data_2, &iv_2, Padding::Strip);

      Some(decrypt_cbc)
    });
    explicit_value
  })
}

fn decrypt_rc4(
  key: &[u8],
  data: &[u8],
  id: ObjectIdentifier,
) -> core::result::Result<Option<Vec<u8>>, console_core_never::Never> {
  Ok({
    let Ok(explicit_value) = decrypt_rc_aes(key, id, ObjectCipher::Rc4, |key_2| {
      let mut rc = match Rc4::new(key_2) {
        Ok(cipher) => cipher,
        Err(error) => {
          warn!("cannot initialize RC4 {:?}", error);
          return None;
        }
      };

      match rc.decrypt(data) {
        Ok(decrypted) => Some(decrypted),
        Err(error) => {
          warn!("cannot decrypt RC4 object {:?}", error);
          None
        }
      }
    });
    explicit_value
  })
}

enum ObjectCipher {
  Rc4,
  Aes,
}

fn decrypt_rc_aes(
  key: &[u8],
  id: ObjectIdentifier,
  cipher: ObjectCipher,
  with_key: impl FnOnce(&[u8]) -> Option<Vec<u8>>,
) -> core::result::Result<Option<Vec<u8>>, console_core_never::Never> {
  let mut object_key_input = key.to_vec();
  let [first, second, third, ..] = id.obj_number.to_le_bytes();
  let [low, high, ..] = id.gen_number.to_le_bytes();
  object_key_input.extend([first, second, third, low, high]);

  match cipher {
    ObjectCipher::Aes => object_key_input.extend(b"sAlT"),
    ObjectCipher::Rc4 => {}
  }

  let Ok(hash) = md5::calculate(&object_key_input);
  let ::core::result::Result::Ok(used_bytes) =
    ::console_core_number_conversion::fixed_index(key.len().saturating_add(5).min(hash.len()));
  Ok(
    hash
      .get(
        ..match ::console_core_number_conversion::index(used_bytes) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
      )
      .and_then(with_key),
  )
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct DecryptorData {
  stream_filter: CryptDictionary,
  string_filter: CryptDictionary,
}

impl DecryptorData {
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Crypt filter dictionaries resolve through the bounded indirect-object reader"
    )
  )]
  fn from_dict(
    dict: &Dict<'_>,
    default_length: u16,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let mut mappings = HashMap::new();

    let Ok(explicit_value) = dict.get::<Dict<'_>>(CF);

    match explicit_value {
      Some(dict_2) => {
        let Ok(keys) = dict_2.keys();

        for key in keys {
          let Ok(filter_dict) = dict_2.get::<Dict<'_>>(key.as_ref());
          let crypt_dict = filter_dict.and_then(|filter_dict_2| {
            let Ok(crypt_dict) = CryptDictionary::from_dict(&filter_dict_2, default_length);
            crypt_dict
          });

          match crypt_dict {
            Some(crypt_dict_2) => {
              let Ok(filter_name) = key.as_str();
              mappings.insert(filter_name.to_string(), crypt_dict_2);
            }
            None => {}
          }
        }
      }
      None => {}
    }

    let Ok(stm_name) = dict.get::<Name<'_>>(STM_F);
    let stm_name_2 = match stm_name {
      Some(name) => name,
      None => return Ok(None),
    };
    let Ok(stm_name_3) = stm_name_2.as_str();
    let Ok(str_name) = dict.get::<Name<'_>>(STR_F);
    let str_name_2 = match str_name {
      Some(name) => name,
      None => return Ok(None),
    };
    let Ok(str_name_3) = str_name_2.as_str();
    let Ok(identity) = CryptDictionary::identity(default_length);
    let stm_f = match mappings.get(stm_name_3) {
      Some(filter) => *filter,
      None => identity,
    };
    let str_f = match mappings.get(str_name_3) {
      Some(filter) => *filter,
      None => identity,
    };

    Ok(Some(Self {
      stream_filter: stm_f,
      string_filter: str_f,
    }))
  }
}

#[derive(Debug, Copy, Clone)]
struct CryptDictionary {
  cfm: DecryptorTag,
  _length: u16,
}

impl CryptDictionary {
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Crypt filter dictionaries resolve through the bounded indirect-object reader"
    )
  )]
  fn from_dict(
    dict: &Dict<'_>,
    default_length: u16,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(explicit_value) = DecryptorTag::from_name(&match match dict.get::<Name<'_>>(CFM) {
      Ok(explicit_value) => explicit_value,
    } {
      Some(value) => value,
      None => return Ok(None),
    });
    let cfm = match explicit_value {
      Some(value) => value,
      None => return Ok(None),
    };

    let mut length = match (
      ({
        let Ok(explicit_value_2) = dict.get::<u16>(LENGTH);
        explicit_value_2
      }),
      ::core::primitive::u16::wrapping_div(default_length, 8),
    ) {
      (::core::option::Option::Some(option_value), _) => option_value,
      (::core::option::Option::None, alternate) => alternate,
    };

    match ::core::primitive::bool::then_some(cfm == DecryptorTag::Aes128, ()) {
      ::core::option::Option::Some(()) => {
        length = 16;
      }
      ::core::option::Option::None => {
        match ::core::primitive::bool::then_some(cfm == DecryptorTag::Aes256, ()) {
          ::core::option::Option::Some(()) => {
            length = 32;
          }
          ::core::option::Option::None => {}
        }
      }
    }

    Ok(Some(Self {
      cfm,
      _length: length,
    }))
  }

  fn identity(default_length: u16) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      cfm: DecryptorTag::None,
      _length: default_length,
    })
  }
}

fn compute_hash_rev56(
  password: Password<'_>,
  validation_salt: &[u8],
  user_string: Option<&[u8]>,
  revision: u8,
) -> Result<[u8; 32], DecryptionError> {
  let mut input = password.0.to_vec();
  input.extend_from_slice(validation_salt);

  match user_string {
    Some(user_bytes) => {
      input.extend_from_slice(user_bytes);
    }
    ::core::option::Option::None => {}
  }

  let Ok(initial_hash) = sha256::calculate(&input);

  match revision {
    5 => {
      return Ok(initial_hash);
    }
    _ => {}
  }

  let mut digest = initial_hash.to_vec();

  for round in 1_u16..=287 {
    let mut single = password.0.to_vec();
    single.extend_from_slice(&digest);

    match user_string {
      Some(user_bytes) => {
        single.extend_from_slice(user_bytes);
      }
      ::core::option::Option::None => {}
    }

    let repeated = single.repeat(64);

    let key = digest.first_chunk::<16>().ok_or(InvalidEncryption)?;
    let iv = digest
      .get(16..)
      .and_then(<[u8]>::first_chunk::<16>)
      .ok_or(InvalidEncryption)?;
    let Ok(cipher) = AES128Cipher::new(key);
    let aes = cipher.ok_or(InvalidEncryption)?;
    let Ok(mut encrypted) = aes.encrypt_cbc(&repeated, iv);
    encrypted.truncate(repeated.len());

    let first_block = encrypted.first_chunk::<16>().ok_or(InvalidEncryption)?;
    let selection = u128::from_be_bytes(*first_block).wrapping_rem(3);
    digest = match selection {
      0 => {
        let Ok(next_digest) = sha256::calculate(&encrypted);
        next_digest.to_vec()
      }
      1 => {
        let Ok(next_digest) = sha384::calculate(&encrypted);
        next_digest.to_vec()
      }
      2 => {
        let Ok(next_digest) = sha512::calculate(&encrypted);
        next_digest.to_vec()
      }
      _ => return Err(InvalidEncryption),
    };

    match ::core::primitive::bool::then_some(round >= 64, ()) {
      ::core::option::Option::Some(()) => {
        let last_byte = encrypted.last().copied().ok_or(InvalidEncryption)?;
        let Ok(last_value) = console_core_number_conversion::wrapped::<_, u16>(last_byte);

        match ::core::primitive::bool::then_some(last_value <= round.wrapping_sub(32), ()) {
          ::core::option::Option::Some(()) => {
            break;
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    }
  }

  digest.first_chunk::<32>().copied().ok_or(InvalidEncryption)
}

enum MetadataEncryption {
  Encrypted,
  Plain,
}

fn decryption_key_rev1234(
  password: Password<'_>,
  metadata: MetadataEncryption,
  revision: u8,
  byte_length: u16,
  owner_string: &object::String<'_>,
  permissions: u32,
  id: &[u8],
) -> Result<Vec<u8>, DecryptionError> {
  let mut md5_input = Vec::new();
  let mut padded_password = PASSWORD_PADDING;
  let ::core::result::Result::Ok(copied_bytes) =
    ::console_core_number_conversion::fixed_index(password.0.len().min(padded_password.len()));
  let ::core::result::Result::Ok(padded_width) =
    ::console_core_number_conversion::fixed_index(padded_password.len().wrapping_sub(
      match ::console_core_number_conversion::index(copied_bytes) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    ));
  let prefix = password
    .0
    .get(
      ..match ::console_core_number_conversion::index(copied_bytes) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    )
    .ok_or(InvalidEncryption)?;
  let tail = PASSWORD_PADDING
    .get(
      ..match ::console_core_number_conversion::index(padded_width) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    )
    .ok_or(InvalidEncryption)?;

  for (destination, byte) in padded_password.iter_mut().zip(prefix.iter().chain(tail)) {
    *destination = *byte;
  }

  md5_input.extend(padded_password);
  md5_input.extend(owner_string.as_ref());
  md5_input.extend(permissions.to_le_bytes());
  md5_input.extend(id);

  match (metadata, revision) {
    (MetadataEncryption::Plain, 4..) => md5_input.extend([0xff; 4]),
    _ => {}
  }

  let Ok(mut hash) = md5::calculate(&md5_input);
  let Ok(key_width) = index(byte_length);

  match revision.cmp(&3) {
    ::core::cmp::Ordering::Equal | ::core::cmp::Ordering::Greater => {
      for _ in 0..50 {
        let portion = hash.get(..key_width).ok_or(InvalidEncryption)?;
        let Ok(next_hash) = md5::calculate(portion);
        hash = next_hash;
      }
    }
    ::core::cmp::Ordering::Less => {}
  }

  let final_key = hash.get(..key_width).ok_or(InvalidEncryption)?;
  Ok(final_key.to_vec())
}

fn authenticate_user_password_rev234(
  revision: u8,
  decryption_key: &[u8],
  id: &[u8],
  user_string: &object::String<'_>,
) -> Result<(), DecryptionError> {
  let matches = match revision {
    2 => {
      let mut rc = Rc4::new(decryption_key)?;
      let expected = rc.decrypt(&PASSWORD_PADDING)?;
      expected.as_slice() == user_string.as_ref()
    }
    3 | 4 => {
      let expected = user_password_rev34(decryption_key, id)?;
      let expected_prefix = expected.first_chunk::<16>().ok_or(InvalidEncryption)?;
      Some(expected_prefix) == user_string.as_ref().first_chunk::<16>()
    }
    _ => return Err(InvalidEncryption),
  };

  match ::core::primitive::bool::then_some(matches, ()) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(DecryptionError::PasswordProtected),
  }
}

fn user_password_rev34(decryption_key: &[u8], id: &[u8]) -> Result<Vec<u8>, DecryptionError> {
  let mut cipher = Rc4::new(decryption_key)?;
  let mut input = PASSWORD_PADDING.to_vec();
  input.extend(id);
  let Ok(hash) = md5::calculate(&input);
  let mut encrypted = cipher.encrypt(&hash)?;

  for round in 1..=19 {
    let key: Vec<u8> = decryption_key.iter().map(|byte| *byte ^ round).collect();
    let mut round_cipher = Rc4::new(&key)?;
    let next = round_cipher.encrypt(&encrypted)?;
    encrypted = next;
  }

  encrypted.resize(32, 0);
  Ok(encrypted)
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The document's selected encrypted key is looked up using bounded indirect resolution"
  )
)]
fn decryption_key_rev56(
  dict: &Dict<'_>,
  revision: u8,
  password: Password<'_>,
  owner_string: &object::String<'_>,
  user_string: &object::String<'_>,
) -> Result<Vec<u8>, DecryptionError> {
  let password_prefix = password.0.get(..password.0.len().min(127));
  let password_bytes = password_prefix.ok_or(InvalidEncryption)?;
  let truncated_password = Password(password_bytes);
  let ::core::result::Result::Ok(string_width) = ::console_core_number_conversion::fixed_index(
    match ::core::cmp::Ord::cmp(
      &::core::convert::identity(revision),
      &::core::convert::identity(4),
    ) {
      ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => 32,
      ::core::cmp::Ordering::Greater => 48,
    },
  );
  let owner_bytes = owner_string
    .get(
      ..match ::console_core_number_conversion::index(string_width) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    )
    .ok_or(InvalidEncryption)?;
  let user_bytes = user_string
    .get(
      ..match ::console_core_number_conversion::index(string_width) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    )
    .ok_or(InvalidEncryption)?;

  let (owner_hash, owner_tail) = owner_bytes.split_at_checked(32).ok_or(InvalidEncryption)?;
  let (owner_validation_salt, owner_key_salt) =
    owner_tail.split_at_checked(8).ok_or(InvalidEncryption)?;
  let (user_hash, user_tail) = user_bytes.split_at_checked(32).ok_or(InvalidEncryption)?;
  let (user_validation_salt, user_key_salt) =
    user_tail.split_at_checked(8).ok_or(InvalidEncryption)?;

  let owner_candidate = compute_hash_rev56(
    truncated_password,
    owner_validation_salt,
    Some(user_bytes),
    revision,
  )?;
  let (encrypted_key_name, encryption_key) =
    match ::core::primitive::bool::then_some(owner_candidate == owner_hash, ()) {
      ::core::option::Option::Some(()) => {
        let owner_key = compute_hash_rev56(
          truncated_password,
          owner_key_salt,
          Some(user_bytes),
          revision,
        )?;
        (OE, owner_key)
      }
      ::core::option::Option::None => {
        let user_candidate =
          compute_hash_rev56(truncated_password, user_validation_salt, None, revision)?;

        match ::core::primitive::bool::then_some(user_candidate != user_hash, ()) {
          ::core::option::Option::Some(()) => {
            return Err(DecryptionError::PasswordProtected);
          }
          ::core::option::Option::None => {}
        }

        let user_key = compute_hash_rev56(truncated_password, user_key_salt, None, revision)?;
        (UE, user_key)
      }
    };

  let Ok(dictionary_value) = dict.get::<object::String<'_>>(encrypted_key_name);
  let encrypted_key = dictionary_value.ok_or(InvalidEncryption)?;

  match ::core::primitive::bool::then_some(encrypted_key.len() != 32, ()) {
    ::core::option::Option::Some(()) => {
      return Err(InvalidEncryption);
    }
    ::core::option::Option::None => {}
  }

  let Ok(cipher) = AES256Cipher::new(&encryption_key);
  let aes = cipher.ok_or(InvalidEncryption)?;
  let Ok(decrypted_key) = aes.decrypt_cbc(&encrypted_key, &[0; 16], Padding::Keep);
  Ok(decrypted_key)
}
