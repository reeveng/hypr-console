//! Reading and querying the xref table of a PDF file.

use crate::answers::ObjectStorage;
use crate::crypto::{
  DecryptionError, DecryptionTarget, Decryptor, Password, RetainedPassword, get,
};
use crate::data::Data;
use crate::metadata::Metadata;
use crate::object::Name;
use crate::object::ObjectIdentifier;
use crate::object::Stream;
use crate::object::dict::keys::{
  AUTHOR, CREATION_DATE, CREATOR, ENCRYPT, FIRST, ID, INDEX, INFO, KEYWORDS, MOD_DATE, N,
  OCPROPERTIES, PAGES, PREV, PRODUCER, ROOT, SIZE, SUBJECT, TITLE, TYPE, VERSION, W, XREF_STM,
};
use crate::object::dict::{DictProbe, probe_dict};
use crate::object::indirect::IndirectObject;
use crate::object::{Array, MaybeRef};
use crate::object::{DateTime, Dict};
use crate::object::{Object, ObjectLike};
use crate::pdf::PdfVersion;
use crate::reader::Reader;
use crate::reader::{IndirectReferenceError, Readable, ReaderContext, ReaderExt};
use crate::sync::{Arc, FixedMap, RwLock, RwLockExt};
use crate::trivia::is_white_space_character;
use crate::util::findr_needle;
use crate::{
  ContainsKeyAnswer, End, HasOptionalContentGroupsAnswer, IsRepairedAnswer, ObjectReadMode,
  PdfData, object,
};
use alloc::collections::BTreeSet;
use alloc::vec;
use alloc::vec::Vec;
use console_core_never::Never;
use console_core_number_conversion::{checked_index, index};

const XREF_ENTRY_LEN: u64 = 20;

#[derive(Debug, Copy, Clone)]
pub(crate) enum XRefError {
  Unknown,
  Encryption(DecryptionError),
}

pub(crate) fn root_xref(data: PdfData, password: Password<'_>) -> Result<XRef, XRefError> {
  let mut xref_map = FixedMap::default();
  let xref_pos = ({
    let Ok(explicit_value) = find_last_xref_pos(data.as_ref());
    explicit_value
  })
  .ok_or(XRefError::Unknown)?;
  let trailer = ({
    let Ok(explicit_value) = populate_xref_impl(data.as_ref(), xref_pos, &mut xref_map);
    explicit_value
  })
  .ok_or(XRefError::Unknown)?;

  XRef::new(
    data.clone(),
    xref_map,
    XRefInput::TrailerDictData(trailer),
    IsRepairedAnswer::No,
    password,
  )
}

pub(crate) fn fallback(
  data: PdfData,
  password: Password<'_>,
) -> core::result::Result<Option<XRef>, console_core_never::Never> {
  warn!("xref table was invalid, trying to manually build xref table");
  let Ok((xref_map, xref_input)) = fallback_xref_map(&data, password);

  Ok(match xref_input {
    Some(xref_input_2) => {
      warn!("rebuild xref table with {} entries", xref_map.len());

      match XRef::new(
        data.clone(),
        xref_map,
        xref_input_2,
        IsRepairedAnswer::Yes,
        password,
      ) {
        Ok(xref) => Some(xref),
        Err(error) => {
          warn!("couldn't initialize rebuilt xref {:?}", error);
          None
        }
      }
    }
    None => {
      warn!("couldn't find trailer dictionary, failed to rebuild xref table");

      None
    }
  })
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Recovery scans twice at most, and the temporary xref is already marked repaired"
  )
)]
fn fallback_xref_map<'a>(
  data: &'a PdfData,
  password: Password<'_>,
) -> Result<(XrefMap, Option<XRefInput<'a>>), Never> {
  let Ok(context) = ReaderContext::dummy();
  let Ok((mut map, input)) = scan_xref(data.as_ref(), context);
  let trailer_bytes = match input {
    Some(XRefInput::TrailerDictData(bytes)) => bytes,
    Some(XRefInput::RootRef(_)) | None => return Ok((map, input)),
  };
  let Ok(dummy) = ReaderContext::dummy();
  let Ok(mut reader) = Reader::new(trailer_bytes);
  let trailer = match reader.read_with_context::<Dict<'_>>(&dummy) {
    Some(trailer) => trailer,
    None => return Ok((map, input)),
  };
  let Ok(encryption) = trailer.contains_key(ENCRYPT);

  match ::core::primitive::bool::then_some(encryption == ContainsKeyAnswer::Yes, ()) {
    ::core::option::Option::Some(()) => {
      let candidate = XRef::new(
        data.clone(),
        map.clone(),
        XRefInput::TrailerDictData(trailer_bytes),
        IsRepairedAnswer::Yes,
        password,
      );

      match candidate {
        Ok(xref) => {
          let Ok(encrypted_context) = ReaderContext::new(&xref, ObjectReadMode::Document);
          let Ok((resolved_map, _)) = scan_xref(data.as_ref(), encrypted_context);
          map = resolved_map;
        }
        Err(error) => warn!("couldn't reopen rebuilt encrypted xref {:?}", error),
      }
    }
    ::core::option::Option::None => {}
  }

  Ok((map, input))
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "A scan with a live xref resolves references under the 128-object limit and a map already marked repaired"
  )
)]
fn scan_xref<'a>(
  data: &'a [u8],
  mut context: ReaderContext<'a>,
) -> Result<(XrefMap, Option<XRefInput<'a>>), Never> {
  let mut map = FixedMap::default();
  let mut trailers = Vec::new();
  let mut root = None;
  let Ok(mut reader) = Reader::new(data);
  let mut last_id = None;

  for _ in data {
    let Ok(position) = reader.fixed_offset();
    let Ok(next_byte) = reader.peek_byte();

    match next_byte {
      Some(b'0'..=b'9') => {
        let Ok(object) = recover_object_header(&mut reader, &context);

        match object {
          Some(id) => {
            map.insert(id, EntryType::Normal(position));
            last_id = Some(id);
            let Ok(()) = context.set_obj_number(id);
          }
          None => {}
        }
      }
      Some(b'<') => {
        let Ok(discovered) = probe_recovered_dict(&mut reader, &context, last_id, &mut map);

        let (dictionary, catalog) = discovered;

        match dictionary {
          Some(dictionary_2) => trailers.push(dictionary_2),
          None => {}
        }

        match catalog {
          ContainsKeyAnswer::Yes => root = last_id,
          ContainsKeyAnswer::No => {}
        }
      }
      Some(_) | None => {}
    }

    let Ok(offset) = reader.fixed_offset();

    match ::core::primitive::bool::then_some(offset == position, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = reader.forward_while(|byte| {
          let Ok(whitespace) = is_white_space_character(byte);
          whitespace == crate::IsWhiteSpaceCharacterAnswer::No
        });
        let Ok(skipped_position) = reader.fixed_offset();

        match ::core::primitive::bool::then_some(skipped_position == offset, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(_) = reader.read_byte();
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    }

    let Ok(end) = reader.at_end();

    match ::core::primitive::bool::then_some(end == End::Reached, ()) {
      ::core::option::Option::Some(()) => {
        break;
      }
      ::core::option::Option::None => {}
    }
  }

  let mut selected = root.map(XRefInput::RootRef);

  for trailer in trailers {
    let Ok(pages) = recovered_root_has_pages(data, &map, &context, &trailer);

    match pages {
      ContainsKeyAnswer::Yes => {}
      ContainsKeyAnswer::No => continue,
    }

    let Ok(bytes) = trailer.data();
    selected = Some(XRefInput::TrailerDictData(bytes));
  }

  Ok((map, selected))
}

fn recover_object_header<'a>(
  reader: &mut Reader<'a>,
  context: &ReaderContext<'a>,
) -> Result<Option<ObjectIdentifier>, Never> {
  let id = match reader.read::<ObjectIdentifier>(context) {
    Some(id) => id,
    None => return Ok(None),
  };
  let mut object_reader = reader.clone();
  object_reader.skip_white_spaces_and_comments();

  match object_reader.skip::<Object<'_>>(ObjectReadMode::Document) {
    Some(_object_bytes) => Ok(Some(id)),
    None => Ok(None),
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The lookup context bounds nested object-stream reads and disables another repair during the resolved scan"
  )
)]
fn probe_recovered_dict<'a>(
  reader: &mut Reader<'a>,
  context: &ReaderContext<'a>,
  last_id: Option<ObjectIdentifier>,
  map: &mut XrefMap,
) -> Result<(Option<Dict<'a>>, ContainsKeyAnswer), Never> {
  let Ok(tag) = reader.peek_bytes(2);

  match ::core::primitive::bool::then_some(tag != Some(b"<<"), ()) {
    ::core::option::Option::Some(()) => {
      return Ok((None, ContainsKeyAnswer::No));
    }
    ::core::option::Option::None => {}
  }

  let mut probe_reader = reader.clone();
  let Ok(probe) = probe_dict(&mut probe_reader, context, Some(b"<<"), b">>");
  let dictionary = match probe {
    Some(found) => found,
    None => return Ok((None, ContainsKeyAnswer::No)),
  };
  let recovered = recover_dictionary(reader, context, dictionary, last_id, map);
  *reader = probe_reader;
  recovered
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Recovered dictionary fields resolve through the bounded indirect-object reader"
  )
)]
fn recover_dictionary<'a>(
  stream_reader: &Reader<'a>,
  context: &ReaderContext<'a>,
  probe: DictProbe<'a>,
  last_id: Option<ObjectIdentifier>,
  map: &mut XrefMap,
) -> Result<(Option<Dict<'a>>, ContainsKeyAnswer), Never> {
  match ::core::primitive::bool::then_some(
    (probe.root_key, probe.type_key) == (ContainsKeyAnswer::No, ContainsKeyAnswer::No),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok((None, ContainsKeyAnswer::No));
    }
    ::core::option::Option::None => {}
  }

  let Ok(mut reader) = Reader::new(probe.data);
  let dict = match reader.read_with_context::<Dict<'_>>(context) {
    Some(dictionary) => dictionary,
    None => return Ok((None, ContainsKeyAnswer::No)),
  };
  let Ok(root) = dict.contains_key(ROOT);
  let Ok(object_type) = dict.get::<Name<'_>>(TYPE);
  let catalog = match object_type.as_deref() {
    Some(b"Catalog") => ContainsKeyAnswer::Yes,
    Some(b"ObjStm") => {
      match last_id {
        Some(id) => {
          let mut reader_2 = stream_reader.clone();
          let Ok(()) = recover_stream_entries(&mut reader_2, context, id, map);
        }
        ::core::option::Option::None => {}
      }

      ContainsKeyAnswer::No
    }
    Some(_) | None => ContainsKeyAnswer::No,
  };

  let trailer = match (probe.root_key, root) {
    (ContainsKeyAnswer::Yes, ContainsKeyAnswer::Yes) => Some(dict),
    (ContainsKeyAnswer::No, _) | (_, ContainsKeyAnswer::No) => None,
  };
  Ok((trailer, catalog))
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Object-stream headers use the indirect reader's 128-object limit for referenced fields"
  )
)]
fn recover_stream_entries<'a>(
  reader: &mut Reader<'a>,
  context: &ReaderContext<'a>,
  id: ObjectIdentifier,
  map: &mut XrefMap,
) -> Result<(), Never> {
  let stream = {
    let optional_stream = reader.read::<Stream<'_>>(context);

    match optional_stream {
      Some(matched_stream) => matched_stream,
      ::core::option::Option::None => return Ok(()),
    }
  };

  let content = match stream.decoded() {
    Ok(decoded) => decoded,
    Err(error) => {
      warn!(
        "couldn't decode object stream while rebuilding xref {:?}",
        error
      );
      return Ok(());
    }
  };
  let Ok(objects) = ObjectStream::new(stream, &content, context);

  let objects_2 = match objects {
    Some(matched_objects) => matched_objects,
    ::core::option::Option::None => return Ok(()),
  };

  let Ok(stream_number) = console_core_number_conversion::wrapped::<_, u32>(id.obj_number);

  for (idx, (number, _)) in objects_2.offsets.iter().enumerate() {
    let Ok(object_number) = console_core_number_conversion::wrapped::<_, i32>(*number);
    let Ok(object) = ObjectIdentifier::new(object_number, 0);

    match map.get(&object) {
      Some(EntryType::Normal(_)) => {}
      Some(EntryType::ObjStream(_, _)) | None => {
        let Ok(index) = console_core_number_conversion::wrapped::<_, u32>(idx);
        map.insert(object, EntryType::ObjStream(stream_number, index));
      }
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Recovered catalog lookup traverses indirect fields through the bounded reader context"
  )
)]
fn recovered_root_has_pages<'a>(
  data: &'a [u8],
  map: &XrefMap,
  context: &ReaderContext<'a>,
  trailer: &Dict<'a>,
) -> Result<ContainsKeyAnswer, Never> {
  let Ok(root) = trailer.get_raw::<Dict<'_>>(ROOT);
  let reference = match root {
    Some(MaybeRef::NotRef(catalog)) => return catalog.contains_key(PAGES),
    Some(MaybeRef::Ref(reference)) => reference,
    None => return Ok(ContainsKeyAnswer::No),
  };

  match map.get(&ObjectIdentifier::from(reference)) {
    Some(EntryType::Normal(offset)) => {
      let Ok(native_offset) = index(*offset);
      let Ok(mut reader) = Reader::new_with(data, native_offset);

      match reader.read_with_context::<IndirectObject<Dict<'_>>>(context) {
        Some(indirect) => {
          let Ok(catalog) = indirect.get();
          catalog.contains_key(PAGES)
        }
        None => Ok(ContainsKeyAnswer::No),
      }
    }
    Some(EntryType::ObjStream(number, ordinal)) => {
      let Ok(object_number) = console_core_number_conversion::wrapped::<_, i32>(*number);
      let Ok(stream_id) = ObjectIdentifier::new(object_number, 0);

      match map.get(&stream_id) {
        Some(EntryType::Normal(offset)) => {
          compressed_root_has_pages(data, context, *offset, *ordinal)
        }
        Some(EntryType::ObjStream(_, _)) | None => Ok(ContainsKeyAnswer::No),
      }
    }
    None => Ok(ContainsKeyAnswer::No),
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The catalog stream's references use the indirect-object depth limit"
  )
)]
fn compressed_root_has_pages<'a>(
  data: &'a [u8],
  context: &ReaderContext<'a>,
  offset: u64,
  ordinal: u32,
) -> Result<ContainsKeyAnswer, Never> {
  let Ok(native_offset) = index(offset);
  let Ok(mut reader) = Reader::new_with(data, native_offset);
  let indirect = match reader.read_with_context::<IndirectObject<Stream<'_>>>(context) {
    Some(stream) => stream,
    None => return Ok(ContainsKeyAnswer::No),
  };
  let Ok(stream) = indirect.get();
  let bytes = match stream.decoded() {
    Ok(decoded) => decoded,
    Err(error) => {
      warn!("couldn't decode recovered catalog stream {:?}", error);
      return Ok(ContainsKeyAnswer::No);
    }
  };
  let Ok(parsed) = ObjectStream::new(stream, &bytes, context);
  let objects = match parsed {
    Some(objects) => objects,
    None => return Ok(ContainsKeyAnswer::No),
  };
  let Ok(catalog) = objects.get::<Dict<'_>>(ordinal);

  match catalog {
    Some(dictionary) => dictionary.contains_key(PAGES),
    None => Ok(ContainsKeyAnswer::No),
  }
}

const DUMMY_XREF: XRef = XRef(Inner::Dummy);

#[derive(Debug, Clone)]
pub struct XRef(Inner);

impl XRef {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Public XRef clones and borrowed document objects retain one backing document and repaired map"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Constructor lookups use an indirect reader with a depth limit and cycle detection"
    )
  )]
  fn new(
    data: PdfData,
    xref_map: XrefMap,
    input: XRefInput<'_>,
    repaired: IsRepairedAnswer,
    password: Password<'_>,
  ) -> Result<Self, XRefError> {
    let Ok(data_2) = Data::new(data);
    let mut xref = Self(Inner::Some(Arc::new(SomeRepr {
      data: data_2,
      map: RwLock::new(MapRepr { xref_map, repaired }),
      decryptor: Decryptor::None,
      has_ocgs: HasOptionalContentGroupsAnswer::No,
      metadata: Metadata::default(),
      trailer_data: EMPTY_TRAILER,
      password: RetainedPassword(password.0.to_vec()),
    })));

    let decryptor = match input {
      XRefInput::TrailerDictData(bytes) => {
        let trailer = read_trailer_context(&xref, bytes)?;
        get_decryptor(&trailer, password)?
      }
      XRefInput::RootRef(_) => Decryptor::None,
    };
    let state = xref.initializing_state()?;
    state.decryptor = decryptor;
    let fields = match input {
      XRefInput::TrailerDictData(bytes) => document_fields(&xref, bytes)?,
      XRefInput::RootRef(id) => recovered_document_fields(&xref, id)?,
    };
    let state_2 = xref.initializing_state()?;
    state_2.trailer_data = fields.trailer;
    state_2.has_ocgs = fields.has_ocgs;
    state_2.metadata = fields.metadata;

    Ok(xref)
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The xref remains uniquely owned during construction, before cloned readers retain it"
    )
  )]
  fn initializing_state(&mut self) -> Result<&mut SomeRepr, XRefError> {
    match &mut self.0 {
      Inner::Some(repr) => Arc::get_mut(repr).ok_or(XRefError::Unknown),
      Inner::Dummy => Err(XRefError::Unknown),
    }
  }

  pub(crate) fn dummy() -> Result<&'static Self, Never> {
    Ok(&DUMMY_XREF)
  }

  pub(crate) fn len(&self) -> Result<u64, Never> {
    Ok(match &self.0 {
      Inner::Dummy => 0,
      Inner::Some(repr) => match repr.map.get() {
        Some(table) => {
          let Ok(count) = console_core_number_conversion::fixed_index(table.xref_map.len());
          count
        }
        None => 0,
      },
    })
  }

  pub(crate) fn trailer_data(&self) -> Result<&TrailerData, Never> {
    Ok(match &self.0 {
      Inner::Dummy => &EMPTY_TRAILER,
      Inner::Some(repr) => &repr.trailer_data,
    })
  }

  pub(crate) fn metadata(&self) -> Result<&Metadata, Never> {
    Ok(match &self.0 {
      Inner::Dummy => &EMPTY_METADATA,
      Inner::Some(repr) => &repr.metadata,
    })
  }

  pub fn root_id(&self) -> Result<ObjectIdentifier, Never> {
    let Ok(trailer) = self.trailer_data();

    Ok(trailer.root_ref)
  }

  pub fn has_optional_content_groups(&self) -> Result<HasOptionalContentGroupsAnswer, Never> {
    Ok(match &self.0 {
      Inner::Dummy => HasOptionalContentGroupsAnswer::No,
      Inner::Some(repr) => repr.has_ocgs,
    })
  }

  pub(crate) fn objects(&self) -> Result<impl IntoIterator<Item = Object<'_>> + '_, Never> {
    let Ok(mut order) = self.object_order();
    order.sort_by_key(|entry| entry.position);
    let Ok(context) = ReaderContext::new(self, ObjectReadMode::Document);

    Ok(order.into_iter().filter_map(move |entry| {
      let Ok(object) = self.get_with(entry.id, &context);
      object
    }))
  }

  fn object_order(&self) -> Result<Vec<OrderedObject>, Never> {
    let repr = match &self.0 {
      Inner::Some(repr) => repr,
      Inner::Dummy => return Ok(Vec::new()),
    };
    let table = match repr.map.get() {
      Some(table) => table,
      None => return Ok(Vec::new()),
    };

    Ok(
      table
        .xref_map
        .iter()
        .map(|(id, entry)| {
          let Ok(position) = object_position(&table.xref_map, *entry);
          OrderedObject { id: *id, position }
        })
        .collect(),
    )
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "The replacement is scanned with a dummy context and at most one repaired xref that cannot start another repair"
    )
  )]
  pub(crate) fn repair(&self) -> Result<(), Never> {
    let repr = match &self.0 {
      Inner::Some(repr) => repr,
      Inner::Dummy => return Ok(()),
    };
    let status = match repr.map.get() {
      Some(table) => table.repaired,
      None => return Ok(()),
    };

    match status {
      IsRepairedAnswer::Yes => return Ok(()),
      IsRepairedAnswer::No => {}
    }

    let Ok(data) = repr.data.get();
    let Ok((rebuilt, _trailer)) = fallback_xref_map(data, Password(&repr.password.0));
    let mut table = match repr.map.try_put() {
      Some(table) => table,
      None => return Ok(()),
    };

    match table.repaired {
      IsRepairedAnswer::Yes => {}
      IsRepairedAnswer::No => {
        table.xref_map = rebuilt;
        table.repaired = IsRepairedAnswer::Yes;
      }
    }

    Ok(())
  }

  #[inline]
  pub(crate) fn needs_decryption(
    &self,
    ctx: &ReaderContext<'_>,
  ) -> core::result::Result<crate::NeedsDecryptionAnswer, console_core_never::Never> {
    use crate::NeedsDecryptionAnswer;

    let decryptor = match &self.0 {
      Inner::Dummy => return Ok(NeedsDecryptionAnswer::No),
      Inner::Some(tables) => &tables.decryptor,
    };
    let Ok(mode) = ctx.read_mode();
    let Ok(storage) = ctx.object_storage();

    Ok(match (decryptor, mode, storage) {
      (
        Decryptor::Rc4 { .. } | Decryptor::Aes128 { .. } | Decryptor::Aes256 { .. },
        ObjectReadMode::Document,
        ObjectStorage::Standalone,
      ) => NeedsDecryptionAnswer::Yes,
      (Decryptor::None, _, _)
      | (_, ObjectReadMode::ContentStream, _)
      | (_, _, ObjectStorage::ObjectStream) => NeedsDecryptionAnswer::No,
    })
  }

  #[inline]
  pub(crate) fn decrypt(
    &self,
    id: ObjectIdentifier,
    data: &[u8],
    target: DecryptionTarget,
  ) -> core::result::Result<Option<Vec<u8>>, console_core_never::Never> {
    Ok(match &self.0 {
      Inner::Dummy => Some(data.to_vec()),
      Inner::Some(r) => {
        let Ok(decrypt) = r.decryptor.decrypt(id, data, target);

        decrypt
      }
    })
  }

  #[allow(
    private_bounds,
    reason = "users shouldn't be able to implement `ObjectLike` for custom objects."
  )]
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Indirect lookup records each object and stops at a fixed depth"
    )
  )]
  pub fn get<'a, T>(
    &'a self,
    id: ObjectIdentifier,
  ) -> core::result::Result<Option<T>, console_core_never::Never>
  where
    T: ObjectLike<'a>,
  {
    let Ok(ctx) = ReaderContext::new(self, ObjectReadMode::Document);

    let Ok(with) = self.get_with(id, &ctx);

    Ok(with)
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Indirect lookup records each object and stops at a fixed depth"
    )
  )]
  pub(crate) fn get_with<'a, T>(
    &'a self,
    id: ObjectIdentifier,
    ctx: &ReaderContext<'a>,
  ) -> core::result::Result<Option<T>, console_core_never::Never>
  where
    T: ObjectLike<'a>,
  {
    self.lookup(id, ctx, ObjectLookupKind::Any)
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "IndirectObject records each object ID in the reader context before resolving its dependent objects"
    )
  )]
  fn lookup<'a, T>(
    &'a self,
    id: ObjectIdentifier,
    ctx: &ReaderContext<'a>,
    kind: ObjectLookupKind,
  ) -> Result<Option<T>, Never>
  where
    T: ObjectLike<'a>,
  {
    let repr = match &self.0 {
      Inner::Some(repr) => repr,
      Inner::Dummy => return Ok(None),
    };
    let mut local = ctx.clone();
    let Ok(()) = local.set_obj_number(id);
    let Ok(()) = local.set_read_mode(ObjectReadMode::Document);

    for attempt in [LookupAttempt::First, LookupAttempt::Retry] {
      let Ok(found) = repr.entry(id);
      let (entry, repaired) = match found {
        Some(entry) => entry,
        None => return Ok(None),
      };

      match entry {
        EntryType::ObjStream(container, index) => {
          return match kind {
            ObjectLookupKind::Any => {
              self.read_stream_object(repr, id, container, ObjectStreamIndex(index), &local)
            }
            ObjectLookupKind::Standalone => Ok(None),
          };
        }
        EntryType::Normal(position) => {
          let Ok(object) = repr.read_standalone::<T>(position, id, &mut local);

          match object {
            StandaloneObject::Found(object_2) => return Ok(Some(object_2)),
            StandaloneObject::WrongType => return Ok(None),
            StandaloneObject::Invalid => {}
          }
        }
      }

      match (attempt, repaired) {
        (LookupAttempt::First, IsRepairedAnswer::No) => {
          warn!("broken xref, attempting to repair");
          let Ok(()) = self.repair();
        }
        (LookupAttempt::Retry, _) | (_, IsRepairedAnswer::Yes) => {
          error!(
            "attempt was made at repairing xref, but object {:?} still couldn't be read",
            id
          );
          return Ok(None);
        }
      }
    }

    Ok(None)
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "An object stream container is loaded as a standalone object, which refuses nested stream containers"
    )
  )]
  fn read_stream_object<'a, T>(
    &'a self,
    repr: &'a SomeRepr,
    object: ObjectIdentifier,
    container: u32,
    stream_index: ObjectStreamIndex,
    ctx: &ReaderContext<'a>,
  ) -> Result<Option<T>, Never>
  where
    T: ObjectLike<'a>,
  {
    let Ok(container_number) = console_core_number_conversion::wrapped::<_, i32>(container);
    let Ok(id) = ObjectIdentifier::new(container_number, 0);

    match (id == object).then_some(()) {
      Some(()) => {
        warn!("cycle detected in object stream");
        return Ok(None);
      }
      None => {}
    }

    let Ok(found) = self.lookup::<Stream<'_>>(id, ctx, ObjectLookupKind::Standalone);
    let stream = match found {
      Some(stream) => stream,
      None => return Ok(None),
    };
    let Ok(decoded) = repr.data.decoded_stream(id, &stream);
    let data = match decoded {
      Some(data) => data,
      None => return Ok(None),
    };
    let Ok(parsed) = ObjectStream::new(stream, data, ctx);

    match parsed {
      Some(objects) => objects.get(stream_index.0),
      None => Ok(None),
    }
  }
}

#[derive(Clone, Copy)]
enum ObjectLookupKind {
  Any,
  Standalone,
}

enum LookupAttempt {
  First,
  Retry,
}

enum StandaloneObject<T> {
  Found(T),
  WrongType,
  Invalid,
}

struct OrderedObject {
  id: ObjectIdentifier,
  position: (u64, u32),
}

fn object_position(map: &XrefMap, entry: EntryType) -> Result<(u64, u32), Never> {
  Ok(match entry {
    EntryType::Normal(offset) => (offset, 0),
    EntryType::ObjStream(container, position) => {
      let Ok(number) = console_core_number_conversion::wrapped::<_, i32>(container);
      let Ok(id) = ObjectIdentifier::new(number, 0);

      match map.get(&id) {
        Some(EntryType::Normal(offset)) => (*offset, position),
        Some(EntryType::ObjStream(_, _)) | None => (u64::MAX, 0),
      }
    }
  })
}

const EMPTY_ID: ObjectIdentifier = ObjectIdentifier {
  obj_number: 0,
  gen_number: 0,
};
const EMPTY_TRAILER: TrailerData = TrailerData {
  pages_ref: EMPTY_ID,
  root_ref: EMPTY_ID,
  version: None,
};
const EMPTY_METADATA: Metadata = Metadata {
  creation_date: None,
  modification_date: None,
  title: None,
  author: None,
  subject: None,
  keywords: None,
  creator: None,
  producer: None,
};

struct DocumentFields {
  trailer: TrailerData,
  metadata: Metadata,
  has_ocgs: HasOptionalContentGroupsAnswer,
}

fn read_trailer_context<'a>(xref: &'a XRef, data: &'a [u8]) -> Result<Dict<'a>, XRefError> {
  let Ok(mut reader) = Reader::new(data);
  let Ok(context) = ReaderContext::new(xref, ObjectReadMode::Document);

  reader
    .read_with_context::<Dict<'_>>(&context)
    .ok_or(XRefError::Unknown)
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Any referenced trailer field is read under the indirect-object limit"
  )
)]
fn document_fields(xref: &XRef, data: &[u8]) -> Result<DocumentFields, XRefError> {
  let trailer = read_trailer_context(xref, data)?;
  let Ok(root_id) = trailer.get_ref(ROOT);
  let root_ref = root_id.ok_or(XRefError::Unknown)?;
  let Ok(root_dict) = trailer.get::<Dict<'_>>(ROOT);
  let root = root_dict.ok_or(XRefError::Unknown)?;
  let Ok(info) = trailer.get::<Dict<'_>>(INFO);
  let metadata = match info {
    Some(info_2) => {
      let Ok(metadata) = parse_metadata(&info_2);
      metadata
    }
    None => Metadata::default(),
  };
  let Ok(pages_id) = root.get_ref(PAGES);
  let pages_ref = pages_id.ok_or(XRefError::Unknown)?;
  let Ok(layer_dictionary) = root.get::<Dict<'_>>(OCPROPERTIES);
  let has_ocgs = match layer_dictionary {
    Some(_) => HasOptionalContentGroupsAnswer::Yes,
    None => HasOptionalContentGroupsAnswer::No,
  };
  let Ok(version_name) = root.get::<Name<'_>>(VERSION);
  let version = version_name.and_then(|name| {
    let Ok(version) = PdfVersion::from_bytes(name.as_ref());
    version
  });

  Ok(DocumentFields {
    trailer: TrailerData {
      pages_ref: <_ as ::core::convert::Into<crate::object::ObjectIdentifier>>::into(pages_ref),
      root_ref: <_ as ::core::convert::Into<crate::object::ObjectIdentifier>>::into(root_ref),
      version,
    },
    metadata,
    has_ocgs,
  })
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "A recovered catalog resolves references under the indirect-object limit"
  )
)]
fn recovered_document_fields(
  xref: &XRef,
  root_ref: ObjectIdentifier,
) -> Result<DocumentFields, XRefError> {
  let Ok(found) = xref.get::<Dict<'_>>(root_ref);
  let root = found.ok_or(XRefError::Unknown)?;
  let Ok(pages_id) = root.get_ref(PAGES);
  let pages_ref = pages_id.ok_or(XRefError::Unknown)?;

  Ok(DocumentFields {
    trailer: TrailerData {
      pages_ref: <_ as ::core::convert::Into<crate::object::ObjectIdentifier>>::into(pages_ref),
      root_ref,
      version: None,
    },
    metadata: Metadata::default(),
    has_ocgs: HasOptionalContentGroupsAnswer::No,
  })
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum XRefInput<'a> {
  TrailerDictData(&'a [u8]),

  RootRef(ObjectIdentifier),
}

pub(crate) fn find_last_xref_pos(data: &[u8]) -> Result<Option<u64>, Never> {
  let needle = b"startxref";
  let mut reader = match findr_needle(data, needle) {
    Ok(Some(offset)) => {
      let Ok(position) = index(offset);
      let Ok(reader) = Reader::new_with(data, position);
      reader
    }
    Ok(None) => return Ok(None),
  };
  let Ok(matched) = reader.forward_tag(needle);

  match matched {
    Some(()) => {}
    None => return Ok(None),
  }

  reader.skip_white_spaces_and_comments();

  Ok(match reader.read_without_context::<i32>() {
    Some(offset) => match u64::try_from(offset) {
      Ok(value) => Some(value),
      Err(_negative_startxref) => None,
    },
    None => None,
  })
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum EntryType {
  Normal(u64),

  ObjStream(u32, u32),
}

type XrefMap = FixedMap<ObjectIdentifier, EntryType>;

#[derive(Debug)]
struct MapRepr {
  xref_map: XrefMap,
  repaired: IsRepairedAnswer,
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct TrailerData {
  pub(crate) pages_ref: ObjectIdentifier,
  pub(crate) root_ref: ObjectIdentifier,
  pub(crate) version: Option<PdfVersion>,
}

struct SomeRepr {
  data: Data,
  map: RwLock<MapRepr>,
  metadata: Metadata,
  decryptor: Decryptor,
  has_ocgs: HasOptionalContentGroupsAnswer,
  password: RetainedPassword,
  trailer_data: TrailerData,
}

impl SomeRepr {
  fn entry(&self, id: ObjectIdentifier) -> Result<Option<(EntryType, IsRepairedAnswer)>, Never> {
    Ok(match self.map.try_get() {
      Some(state) => state
        .xref_map
        .get(&id)
        .map(|entry| (*entry, state.repaired)),
      None => None,
    })
  }

  fn read_standalone<'a, T>(
    &'a self,
    offset: u64,
    id: ObjectIdentifier,
    context: &mut ReaderContext<'a>,
  ) -> Result<StandaloneObject<T>, Never>
  where
    T: ObjectLike<'a>,
  {
    let Ok(data) = self.data.get();
    let Ok(position) = index(offset);
    let Ok(mut reader) = Reader::new_with(data.as_ref(), position);
    let Ok(()) = context.set_object_storage(ObjectStorage::Standalone);

    Ok(
      match reader.read_with_context::<IndirectObject<T>>(context) {
        Some(object) => {
          let Ok(found) = object.id();

          match (found == &id).then_some(()) {
            Some(()) => {
              let Ok(parsed) = object.get();
              StandaloneObject::Found(parsed)
            }
            None => StandaloneObject::Invalid,
          }
        }
        None => match reader.skip::<IndirectObject<Object<'_>>>(ObjectReadMode::Document) {
          Some(_) => StandaloneObject::WrongType,
          None => StandaloneObject::Invalid,
        },
      },
    )
  }
}

impl core::fmt::Debug for SomeRepr {
  fn fmt(&self, to: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    to.debug_struct("SomeRepr")
      .field("data", &self.data)
      .field("map", &self.map)
      .field("metadata", &self.metadata)
      .field("decryptor", &self.decryptor)
      .field(
        "has_ocgs",
        &matches!(self.has_ocgs, HasOptionalContentGroupsAnswer::Yes),
      )
      .field("trailer_data", &self.trailer_data)
      .finish_non_exhaustive()
  }
}

#[derive(Debug, Clone)]
enum Inner {
  Dummy,

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "XRef clones and borrowed document objects share one owned index and its decoded object streams"
    )
  )]
  Some(Arc<SomeRepr>),
}

#[derive(Debug)]
enum EntryUsage {
  InUse,
  Inactive,
}

#[derive(Debug)]
struct XRefEntry {
  offset: u64,
  gen_number: i32,
  usage: EntryUsage,
}

impl XRefEntry {
  fn read(data: &[u8]) -> Result<Option<Self>, Never> {
    let (
      [
        a0,
        a1,
        a2,
        a3,
        a4,
        a5,
        a6,
        a7,
        a8,
        a9,
        _gap1,
        b0,
        b1,
        b2,
        b3,
        b4,
        _gap2,
        usage,
        _end1,
        _end2,
      ],
      _remaining,
    ) = match data.split_first_chunk::<20>() {
      Some(entry) => entry,
      None => return Ok(None),
    };
    let Ok(offset) = parse_decimal([*a0, *a1, *a2, *a3, *a4, *a5, *a6, *a7, *a8, *a9].as_slice());
    let Ok(generation) = parse_decimal([*b0, *b1, *b2, *b3, *b4].as_slice());
    let (position, generation_number) = match (offset, generation) {
      (Some(position), Some(generation_2)) => (position, generation_2),
      (None, _) | (_, None) => return Ok(None),
    };
    let gen_number = match i32::try_from(generation_number) {
      Ok(generation_3) => generation_3,
      Err(_generation_too_large) => return Ok(None),
    };

    Ok(Some(Self {
      offset: u64::from(position),
      gen_number,
      usage: match usage {
        b'n' => EntryUsage::InUse,
        _inactive_marker => EntryUsage::Inactive,
      },
    }))
  }
}

fn parse_decimal(data: &[u8]) -> Result<Option<u32>, Never> {
  let mut value = 0_u32;

  for &byte in data {
    let digit = match byte {
      b'0'..=b'9' => u32::from(byte.saturating_sub(b'0')),
      _not_a_digit => return Ok(None),
    };
    value = match value
      .checked_mul(10)
      .and_then(|prefix| prefix.checked_add(digit))
    {
      Some(next) => next,
      None => return Ok(None),
    };
  }

  Ok(Some(value))
}

const MAX_XREF_CHAIN_DEPTH: u32 = 256;

enum Section<'a> {
  Table {
    reader: Reader<'a>,
    trailer: Dict<'a>,
  },
  Stream(Stream<'a>),
}

enum SectionWork<'a> {
  Read(u64),
  Apply(Section<'a>),
}

fn populate_xref_impl<'a>(
  data: &'a [u8],
  position: u64,
  xref_map: &mut XrefMap,
) -> Result<Option<&'a [u8]>, Never> {
  let mut visited = BTreeSet::new();
  let mut work = vec![SectionWork::Read(position)];
  let mut latest_trailer = None;

  for _ in 0..=MAX_XREF_CHAIN_DEPTH.saturating_mul(2) {
    match work.pop() {
      Some(SectionWork::Read(offset)) => {
        let Ok(section) = read_pending_section(data, offset, &mut visited);
        let current = match section {
          Some(section_2) => section_2,
          None => return Ok(None),
        };

        let Ok([older, supplement]) = current.dependencies();
        work.push(SectionWork::Apply(current));

        for next in [supplement, older].into_iter().flatten() {
          let Ok(dependency) = console_core_number_conversion::wrapped::<_, u64>(next);
          work.push(SectionWork::Read(dependency));
        }
      }
      Some(SectionWork::Apply(section)) => {
        let Ok(trailer) = section.apply(xref_map);

        match trailer {
          Some(data_2) => latest_trailer = Some(data_2),
          None => return Ok(None),
        }
      }
      None => return Ok(latest_trailer),
    }
  }

  Ok(None)
}

fn read_pending_section<'a>(
  data: &'a [u8],
  position: u64,
  visited: &mut BTreeSet<u64>,
) -> Result<Option<Section<'a>>, Never> {
  match visited.insert(position).then_some(()) {
    Some(()) => {}
    None => {
      warn!("circular xref PREV chain detected at offset {}", position);
      return Ok(None);
    }
  }

  let Ok(visited_count) = console_core_number_conversion::fixed_index(visited.len());

  match (visited_count > u64::from(MAX_XREF_CHAIN_DEPTH)).then_some(()) {
    Some(()) => {
      warn!(
        "xref PREV chain exceeds maximum depth of {}",
        MAX_XREF_CHAIN_DEPTH
      );
      return Ok(None);
    }
    None => {}
  }

  let ::core::result::Result::Ok(native) =
    ::console_core_number_conversion::fixed_index(match checked_index(position) {
      Ok(offset) => offset,
      Err(_offset_outside_address_space) => return Ok(None),
    });
  let Ok(mut reader) = Reader::new_with(
    data,
    match ::console_core_number_conversion::index(native) {
      ::core::result::Result::Ok(native_position) => native_position,
    },
  );
  reader.skip_white_spaces_and_comments();
  let Ok(context) = ReaderContext::dummy();

  let mut probe = reader.clone();

  match probe.read_without_context::<ObjectIdentifier>() {
    Some(_object_id) => {
      let object = match reader.read_with_context::<IndirectObject<Stream<'_>>>(&context) {
        Some(object) => object,
        None => return Ok(None),
      };
      let Ok(stream) = object.get();

      Ok(Some(Section::Stream(stream)))
    }
    None => {
      let mut trailer_reader = reader.clone();
      let Ok(trailer) = read_xref_table_trailer(&mut trailer_reader, &context);
      let dictionary = match trailer {
        Some(trailer_2) => trailer_2,
        None => return Ok(None),
      };

      Ok(Some(Section::Table {
        reader,
        trailer: dictionary,
      }))
    }
  }
}

impl<'a> Section<'a> {
  fn dependencies(&self) -> Result<[Option<i32>; 2], Never> {
    let (dictionary, supplemental) = match self {
      Self::Table { trailer, .. } => {
        let Ok(supplemental) = trailer.get::<i32>(XREF_STM);
        (trailer, supplemental)
      }
      Self::Stream(stream) => {
        let Ok(dict) = stream.dict();
        (dict, None)
      }
    };
    let Ok(previous) = dictionary.get::<i32>(PREV);

    Ok([previous, supplemental])
  }

  fn apply(self, xref_map: &mut XrefMap) -> Result<Option<&'a [u8]>, Never> {
    match self {
      Self::Table {
        mut reader,
        trailer,
      } => populate_from_xref_table(&mut reader, &trailer, xref_map),
      Self::Stream(stream) => populate_from_xref_stream(&stream, xref_map),
    }
  }
}

pub(super) struct SubsectionHeader {
  pub(super) start: u32,
  pub(super) num_entries: u32,
}

impl Readable<'_> for SubsectionHeader {
  fn read(r: &mut Reader<'_>, _: &ReaderContext<'_>) -> Option<Self> {
    r.skip_white_spaces();
    let start = r.read_without_context::<u32>()?;
    r.skip_white_spaces();
    let num_entries = r.read_without_context::<u32>()?;
    r.skip_white_spaces();

    Some(Self { start, num_entries })
  }
}

fn populate_from_xref_table<'a>(
  reader: &mut Reader<'a>,
  trailer: &Dict<'a>,
  insert_map: &mut XrefMap,
) -> Result<Option<&'a [u8]>, Never> {
  let Ok(start) = read_table_start(reader);

  match start {
    Some(()) => {}
    None => return Ok(None),
  }

  let Ok(unread) = reader.tail();
  let input = match unread {
    Some(input) => input,
    None => return Ok(None),
  };

  for _subsection_byte in input {
    let header = match reader.read_without_context::<SubsectionHeader>() {
      Some(header) => header,
      None => break,
    };
    reader.skip_white_spaces();
    let end = match header.start.checked_add(header.num_entries) {
      Some(end) => end,
      None => return Ok(None),
    };

    for object_number in header.start..end {
      let Ok(entry_width) = index(XREF_ENTRY_LEN);
      let Ok(read) = reader.read_bytes(entry_width);
      let data = match read {
        Some(data) => data,
        None => return Ok(None),
      };
      let Ok(parsed) = XRefEntry::read(data);
      let entry = match parsed {
        Some(entry) => entry,
        None => return Ok(None),
      };

      match entry.usage {
        EntryUsage::InUse => {
          let Ok(object_number_2) =
            console_core_number_conversion::wrapped::<_, i32>(object_number);
          let Ok(identity) = ObjectIdentifier::new(object_number_2, entry.gen_number);
          insert_map.insert(identity, EntryType::Normal(entry.offset));
        }
        EntryUsage::Inactive => {}
      }
    }
  }

  trailer.data().map(Some)
}

struct XrefFields {
  tag: u8,
  position: u8,
  index: u8,
}

fn populate_from_xref_stream<'a>(
  stream: &Stream<'a>,
  insert_map: &mut XrefMap,
) -> Result<Option<&'a [u8]>, Never> {
  let Ok(dict) = stream.dict();
  let Ok(count) = dict.get::<u32>(SIZE);
  let Ok(widths) = dict.get::<[u8; 3]>(W);
  let (size, fields) = match (count, widths) {
    (Some(size), Some([tag, position, index])) => (
      size,
      XrefFields {
        tag,
        position,
        index,
      },
    ),
    (None, _) | (_, None) => return Ok(None),
  };

  match (fields.tag, fields.position, fields.index) {
    (0, 0, 0) => return Ok(None),
    _record_consumes_bytes => {}
  }

  match fields.position {
    0..=8 => {}
    _too_wide => {
      error!("xref offset length is larger than the allowed limit");
      return Ok(None);
    }
  }

  match fields.tag {
    1 => {}
    _other_tag_width => warn!("first field in xref stream was longer than 1"),
  }

  let xref_data = match stream.decoded() {
    Ok(data) => data,
    Err(_invalid_xref_stream) => return Ok(None),
  };
  let Ok(mut reader) = Reader::new(xref_data.as_ref());
  let Ok(index_entries) = dict.get::<Array<'_>>(INDEX);

  match index_entries {
    Some(index_array) => {
      let Ok(iter) = index_array.iter::<(u32, u32)>();

      for (first, count_2) in iter {
        let Ok(result) = xref_stream_subsection(
          &mut reader,
          SubsectionHeader {
            start: first,
            num_entries: count_2,
          },
          &fields,
          insert_map,
        );

        match result {
          Some(()) => {}
          None => return Ok(None),
        }
      }
    }
    None => {
      let Ok(result) = xref_stream_subsection(
        &mut reader,
        SubsectionHeader {
          start: 0,
          num_entries: size,
        },
        &fields,
        insert_map,
      );

      match result {
        Some(()) => {}
        None => return Ok(None),
      }
    }
  }

  dict.data().map(Some)
}

fn xref_stream_num(data: &[u8]) -> Result<Option<u32>, Never> {
  Ok(Some(match data {
    [] => return Ok(None),
    [a] => u32::from(*a),
    [a, b] => u32::from(u16::from_be_bytes([*a, *b])),
    [a, b, c] => {
      let Ok(value) = console_core_bytes::endian::u24_be([*a, *b, *c]);

      value
    }
    [a, b, c, d] | [0, 0, 0, 0, a, b, c, d] => u32::from_be_bytes([*a, *b, *c, *d]),
    [_, _, _, _, _, _, _, _] => {
      warn!("xref stream number is too large");
      return Ok(None);
    }
    _unsupported_width => {
      warn!("invalid xref stream number {}", data.len());
      return Ok(None);
    }
  }))
}

fn read_stream_number(reader: &mut Reader<'_>, width: u8) -> Result<Option<u32>, Never> {
  let Ok(count) = index(width);
  let Ok(read) = reader.read_bytes(count);

  match read {
    Some(bytes) => xref_stream_num(bytes),
    None => Ok(None),
  }
}

fn read_stream_number_or_zero(reader: &mut Reader<'_>, width: u8) -> Result<Option<u32>, Never> {
  match width {
    0 => Ok(Some(0)),
    1.. => read_stream_number(reader, width),
  }
}

fn xref_stream_subsection(
  reader: &mut Reader<'_>,
  subsection: SubsectionHeader,
  fields: &XrefFields,
  insert_map: &mut XrefMap,
) -> Result<Option<()>, Never> {
  for offset in 0..subsection.num_entries {
    let tag = match fields.tag {
      0 => 1,
      _present => {
        let Ok(read) = reader.read_byte();

        match read {
          Some(tag) => tag,
          None => return Ok(None),
        }
      }
    };

    match tag {
      0 => {
        let width = u16::from(fields.position).saturating_add(u16::from(fields.index));
        let Ok(advance) = index(width);
        let Ok(skipped) = reader.skip_bytes(advance);

        match skipped {
          Some(()) => continue,
          None => return Ok(None),
        }
      }
      1 | 2 => {}
      _unknown => {
        warn!("xref has unknown field type {}", tag);
        return Ok(None);
      }
    }

    let Ok(field) = match tag {
      1 => read_stream_number_or_zero(reader, fields.position),
      _compressed => read_stream_number(reader, fields.position),
    };
    let first = match field {
      Some(first) => first,
      None => return Ok(None),
    };
    let Ok(last) = read_stream_number_or_zero(reader, fields.index);
    let second = match last {
      Some(second) => second,
      None => return Ok(None),
    };
    let object_number = subsection.start.wrapping_add(offset);
    let Ok(object) = console_core_number_conversion::wrapped::<_, i32>(object_number);
    let (generation, entry) = match tag {
      1 => {
        let Ok(generation) = console_core_number_conversion::wrapped::<_, i32>(second);
        (generation, EntryType::Normal(u64::from(first)))
      }
      _compressed => (0, EntryType::ObjStream(first, second)),
    };
    let Ok(identity) = ObjectIdentifier::new(object, generation);
    insert_map.insert(identity, entry);
  }

  Ok(Some(()))
}

fn read_table_start(reader: &mut Reader<'_>) -> Result<Option<()>, Never> {
  reader.skip_white_spaces();
  let Ok(matched) = reader.forward_tag(b"xref");

  match matched {
    Some(()) => {
      reader.skip_white_spaces();
      Ok(Some(()))
    }
    None => Ok(None),
  }
}

fn read_xref_table_trailer<'a>(
  reader: &mut Reader<'a>,
  ctx: &ReaderContext<'a>,
) -> Result<Option<Dict<'a>>, Never> {
  let Ok(start) = read_table_start(reader);

  match start {
    Some(()) => {}
    None => return Ok(None),
  }

  let Ok(unread) = reader.tail();
  let input = match unread {
    Some(input) => input,
    None => return Ok(None),
  };

  for _subsection_byte in input {
    let header = match reader.read_without_context::<SubsectionHeader>() {
      Some(header) => header,
      None => break,
    };
    let width = XREF_ENTRY_LEN.saturating_mul(u64::from(header.num_entries));
    let Ok(advanced) = match checked_index(width) {
      Ok(width_2) => reader.skip_bytes(width_2),
      Err(_xref_subsection_too_long) => return Ok(None),
    };

    match advanced {
      Some(()) => {}
      None => return Ok(None),
    }
  }

  reader.skip_white_spaces();
  let Ok(trailer) = reader.forward_tag(b"trailer");

  match trailer {
    Some(()) => {
      reader.skip_white_spaces();
      Ok(reader.read_with_context::<Dict<'_>>(ctx))
    }
    None => Ok(None),
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The encryption dictionary and file ID resolve under the indirect-object limit"
  )
)]
fn get_decryptor(
  trailer_dict: &Dict<'_>,
  password: Password<'_>,
) -> Result<Decryptor, XRefError> {
  let Ok(explicit_value) = trailer_dict.get::<Dict<'_>>(ENCRYPT);

  match explicit_value {
    Some(encryption_dict) => {
      let id = match ({
        let Ok(explicit_value_2) = trailer_dict.get::<Array<'_>>(ID);
        explicit_value_2
      })
      .and_then(|a| {
        let Ok(mut flex_iter) = a.flex_iter();
        let Ok(next) = flex_iter.next::<object::String<'_>>();

        next
      }) {
        Some(id) => id.to_vec(),
        None => {
          vec![]
        }
      };

      get(&encryption_dict, &id, password).map_err(XRefError::Encryption)
    }
    None => Ok(Decryptor::None),
  }
}

struct ObjectStreamIndex(u32);

struct ObjectStream<'a> {
  data: &'a [u8],
  ctx: ReaderContext<'a>,
  offsets: Vec<(u32, u64)>,
}

impl<'a> ObjectStream<'a> {
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Object stream N and First fields resolve through the bounded xref reader"
    )
  )]
  fn new(
    inner: Stream<'_>,
    data: &'a [u8],
    ctx: &ReaderContext<'a>,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(dict) = inner.dict();
    let Ok(count) = dict.get::<u64>(N);
    let num_objects = match count {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(first) = dict.get::<u64>(FIRST);
    let first_offset = match first {
      Some(value) => value,
      None => return Ok(None),
    };

    let Ok(mut reader) = Reader::new(data);
    let mut offsets = Vec::new();

    for _ in 0..num_objects {
      reader.skip_white_spaces_and_comments();
      let obj_num = match reader.read_without_context::<u32>() {
        Some(value) => value,
        None => return Ok(None),
      };
      reader.skip_white_spaces_and_comments();
      let relative = match reader.read_without_context::<u64>() {
        Some(value) => value,
        None => return Ok(None),
      };
      let offset = match first_offset.checked_add(relative) {
        Some(offset) => offset,
        None => return Ok(None),
      };
      offsets.push((obj_num, offset));
    }

    let mut ctx_2 = ctx.clone();
    let Ok(()) = ctx_2.set_object_storage(ObjectStorage::ObjectStream);

    Ok(Some(Self {
      data,
      ctx: ctx_2,
      offsets,
    }))
  }

  fn get<T>(&self, index: u32) -> core::result::Result<Option<T>, console_core_never::Never>
  where
    T: ObjectLike<'a>,
  {
    let Ok(position) = console_core_number_conversion::index(index);
    let (object_number, fixed_offset) = match self.offsets.get(position) {
      Some(entry) => *entry,
      None => return Ok(None),
    };
    let Ok(number) = console_core_number_conversion::wrapped::<_, i32>(object_number);
    let Ok(id) = ObjectIdentifier::new(number, 0);
    let mut context = self.ctx.clone();

    match context.push_parent(id) {
      Ok(()) => {}
      Err(IndirectReferenceError::Cycle) => {
        warn!("cycle detected in compressed object {:?}", id);
        return Ok(None);
      }
      Err(IndirectReferenceError::DepthLimit) => {
        warn!(
          "indirect object depth limit reached for compressed object {:?}",
          id
        );
        return Ok(None);
      }
    }

    let Ok(mut r) = Reader::new(self.data);
    let Ok(()) = match checked_index(fixed_offset) {
      Ok(offset) => r.jump(offset),
      Err(_object_offset_out_of_range) => return Ok(None),
    };
    r.skip_white_spaces_and_comments();

    Ok(r.read_with_context::<T>(&context))
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Metadata fields resolve through the bounded indirect-object reader"
  )
)]
fn parse_metadata(
  info_dict: &Dict<'_>,
) -> core::result::Result<Metadata, console_core_never::Never> {
  Ok(Metadata {
    creation_date: ({
      let Ok(explicit_value) = info_dict.get::<object::String<'_>>(CREATION_DATE);
      explicit_value
    })
    .and_then(|c| {
      let Ok(date_time) = DateTime::from_bytes(&c);

      date_time
    }),
    modification_date: ({
      let Ok(explicit_value) = info_dict.get::<object::String<'_>>(MOD_DATE);
      explicit_value
    })
    .and_then(|c| {
      let Ok(date_time) = DateTime::from_bytes(&c);

      date_time
    }),
    title: ({
      let Ok(explicit_value) = info_dict.get::<object::String<'_>>(TITLE);
      explicit_value
    })
    .map(|t| t.to_vec()),
    author: ({
      let Ok(explicit_value) = info_dict.get::<object::String<'_>>(AUTHOR);
      explicit_value
    })
    .map(|t| t.to_vec()),
    subject: ({
      let Ok(explicit_value) = info_dict.get::<object::String<'_>>(SUBJECT);
      explicit_value
    })
    .map(|t| t.to_vec()),
    keywords: ({
      let Ok(explicit_value) = info_dict.get::<object::String<'_>>(KEYWORDS);
      explicit_value
    })
    .map(|t| t.to_vec()),
    creator: ({
      let Ok(explicit_value) = info_dict.get::<object::String<'_>>(CREATOR);
      explicit_value
    })
    .map(|t| t.to_vec()),
    producer: ({
      let Ok(explicit_value) = info_dict.get::<object::String<'_>>(PRODUCER);
      explicit_value
    })
    .map(|t| t.to_vec()),
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_support::TestFailure;

  #[test]
  fn circular_prev_chain() -> Result<(), TestFailure> {
    let mut pdf = b"%PDF-1.0\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec();
    let Ok(expected_xref_pos) = console_core_number_conversion::fitted::<_, u64>(pdf.len());
    pdf.extend_from_slice(
      alloc::format!(
        "xref\n\
                 0 1\n\
                 0000000000 65535 f \r\n\
                 trailer\n<< /Size 1 /Root 1 0 R /Prev {expected_xref_pos} >>\n\
                 startxref\n{expected_xref_pos}\n%%EOF"
      )
      .as_bytes(),
    );

    let mut xref_map = FixedMap::default();

    match find_last_xref_pos(pdf.as_ref()) {
      Ok(Some(xref_pos)) => {
        let Ok(_result) = populate_xref_impl(pdf.as_ref(), xref_pos, &mut xref_map);

        Ok(())
      }
      Ok(None) => Err(TestFailure::MissingValue),
    }
  }

  #[test]
  fn find_last_xref_uses_last_startxref() {
    let pdf = b"%PDF-1.0\nstartxref\n5\n%%EOF\nstartxref\n42\n%%EOF";
    assert_eq!(find_last_xref_pos(pdf), Ok(Some(42)));
  }

  #[test]
  fn xref_table_trailer_rejects_overflowing_entry_skip() {
    let data = b"xref\n0 999999999999999999999\ntrailer\n<<>>";
    let Ok(mut reader) = Reader::new(data);
    let Ok(context) = ReaderContext::dummy();
    assert_eq!(read_xref_table_trailer(&mut reader, &context), Ok(None));
  }
}
