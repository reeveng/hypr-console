//! Dictionaries.

use crate::ObjectReadMode;
use crate::object::macros::object;
use crate::object::r#ref::{MaybeRef, ObjRef};
use crate::object::{Name, ObjectIdentifier};
use crate::object::{Object, ObjectLike};
use crate::reader::Reader;
use crate::reader::{Readable, ReaderContext, ReaderExt, Skippable};
use crate::sync::Arc;
use crate::sync::FixedMap;
use alloc::format;
use alloc::vec::Vec;
use console_core_number_conversion::index;
use core::fmt::{Debug, Formatter};
use core::ops::Deref;

#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "Default is an empty valid PDF dictionary, while TryFrom checks that an Object has dictionary syntax"
  )
)]
#[derive(Clone)]
pub struct Dict<'a>(Inner<'a>);

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Cloned dictionaries share the immutable bytes and key index while retaining their single-pointer layout"
  )
)]
#[derive(Clone)]
enum Inner<'a> {
  Empty,
  Some(Arc<Repr<'a>>),
}

impl Default for Dict<'_> {
  fn default() -> Self {
    let Ok(empty) = Self::empty();

    empty
  }
}

impl PartialEq for Dict<'_> {
  fn eq(&self, other: &Self) -> bool {
    let Ok(data) = self.data();
    let Ok(other_data) = other.data();

    data == other_data
  }
}

impl<'a> Dict<'a> {
  pub fn empty() -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self(Inner::Empty))
  }

  pub fn data(&self) -> core::result::Result<&'a [u8], console_core_never::Never> {
    Ok(match &self.0 {
      Inner::Empty => &[],
      Inner::Some(repr) => repr.data,
    })
  }

  #[allow(
    clippy::len_without_is_empty,
    reason = "is_empty answers an Emptiness, which is not the bool clippy looks for"
  )]
  pub fn len(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    let Ok(offsets) = self.offsets();

    Ok(offsets.map_or(0, FixedMap::len))
  }

  pub fn is_empty(&self) -> core::result::Result<crate::Emptiness, console_core_never::Never> {
    let Ok(offsets) = self.offsets();

    Ok(
      match ::core::primitive::bool::then_some(offsets.is_none_or(FixedMap::is_empty), ()) {
        ::core::option::Option::Some(()) => crate::Emptiness::Empty,
        ::core::option::Option::None => crate::Emptiness::NonEmpty,
      },
    )
  }

  pub fn contains_key(
    &self,
    key: impl AsRef<[u8]>,
  ) -> core::result::Result<crate::ContainsKeyAnswer, console_core_never::Never> {
    let Ok(own_offsets) = self.offsets();

    Ok(
      match ::core::primitive::bool::then_some(
        own_offsets.is_some_and(|offsets| offsets.contains_key(key.as_ref())),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::ContainsKeyAnswer::Yes,
        ::core::option::Option::None => crate::ContainsKeyAnswer::No,
      },
    )
  }

  #[allow(
    private_bounds,
    reason = "users shouldn't be able to implement `ObjectLike` for custom objects."
  )]
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Dictionary references advance the underlying indirect-object reader's ancestry and depth"
    )
  )]
  pub fn get<T>(
    &self,
    key: impl AsRef<[u8]>,
  ) -> core::result::Result<Option<T>, console_core_never::Never>
  where
    T: ObjectLike<'a>,
  {
    let Ok(raw) = self.get_raw::<T>(key.as_ref());

    Ok({
      let Ok(explicit_value) = match raw {
        Some(value) => value,
        None => return Ok(None),
      }
      .resolve({
        let Ok(explicit_value) = self.ctx();
        explicit_value
      });
      explicit_value
    })
  }

  pub fn get_ref(
    &self,
    key: impl AsRef<[u8]>,
  ) -> core::result::Result<Option<ObjRef>, console_core_never::Never> {
    self.read_entry::<ObjRef>(key.as_ref())
  }

  pub fn keys(
    &self,
  ) -> core::result::Result<impl Iterator<Item = Name<'a>> + '_, console_core_never::Never> {
    let Ok(own_offsets) = self.offsets();

    Ok(
      own_offsets
        .into_iter()
        .flat_map(|offsets| offsets.keys().cloned()),
    )
  }

  pub fn entries(
    &self,
  ) -> core::result::Result<
    impl Iterator<Item = (Name<'a>, MaybeRef<Object<'a>>)> + '_,
    console_core_never::Never,
  > {
    let Ok(keys) = self.keys();
    let mut sorted_keys = keys.collect::<Vec<_>>();
    sorted_keys.sort_by(|n1, n2| n1.as_ref().cmp(n2.as_ref()));

    Ok(sorted_keys.into_iter().filter_map(|key| {
      let Ok(raw) = self.get_raw(key.deref());

      match raw {
        Some(object) => Some((key, object)),
        None => {
          warn!("invalid PDF dictionary value for {:?}", key);
          None
        }
      }
    }))
  }

  pub fn obj_id(
    &self,
  ) -> core::result::Result<Option<ObjectIdentifier>, console_core_never::Never> {
    let Ok(ctx) = self.ctx();
    let Ok(obj_number) = ctx.obj_number();

    Ok(obj_number)
  }

  pub fn get_raw<T>(
    &self,
    key: impl AsRef<[u8]>,
  ) -> core::result::Result<Option<MaybeRef<T>>, console_core_never::Never>
  where
    T: Readable<'a>,
  {
    self.read_entry::<MaybeRef<T>>(key.as_ref())
  }

  fn read_entry<T: Readable<'a>>(
    &self,
    key: &[u8],
  ) -> core::result::Result<Option<T>, console_core_never::Never> {
    let Ok(offsets) = self.offsets();
    let offset = match offsets.and_then(|entries| entries.get(key)) {
      Some(offset) => *offset,
      None => return Ok(None),
    };
    let Ok(data) = self.data();
    let Ok(position) = index(offset);
    let tail = match data.get(position..) {
      Some(tail) => tail,
      None => return Ok(None),
    };
    let Ok(mut reader) = Reader::new(tail);
    let Ok(context) = self.ctx();

    Ok(reader.read_with_context::<T>(context))
  }

  pub(crate) fn ctx(
    &self,
  ) -> core::result::Result<&ReaderContext<'a>, console_core_never::Never> {
    Ok(match &self.0 {
      Inner::Empty => {
        let Ok(dummy_ref) = ReaderContext::dummy_ref();

        dummy_ref
      }
      Inner::Some(repr) => &repr.ctx,
    })
  }

  fn offsets(
    &self,
  ) -> core::result::Result<Option<&FixedMap<Name<'a>, u64>>, console_core_never::Never> {
    Ok(match &self.0 {
      Inner::Empty => None,
      Inner::Some(repr) => Some(&repr.offsets),
    })
  }
}

impl Debug for Dict<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    let mut debug_struct = f.debug_struct("Dict");

    let Ok(explicit_value) = self.offsets();

    match explicit_value {
      Some(offsets) => {
        let Ok(data) = self.data();
        let Ok(mut reader) = Reader::new(data);
        let Ok(context) = ReaderContext::dummy();

        let mut entries: Vec<_> = offsets.iter().collect();
        entries.sort_by_key(|(_key, offset)| **offset);

        for (key, offset) in entries {
          let Ok(position) = index(*offset);
          let Ok(()) = reader.jump(position);
          let Ok(name) = key.as_str();
          let shown = format!("{name:?}");

          match reader.read_with_context::<MaybeRef<Object<'_>>>(&context) {
            Some(value) => {
              let _debug_builder = debug_struct.field(&shown, &value);
            }
            None => {
              let _debug_builder = debug_struct.field(&shown, &Option::<Object<'_>>::None);
            }
          }
        }
      }
      None => {}
    }

    debug_struct.finish()
  }
}

impl Skippable for Dict<'_> {
  fn skip(r: &mut Reader<'_>, mode: ObjectReadMode) -> Option<()> {
    let Ok(opened) = r.forward_tag(b"<<");

    opened?;

    let Ok(tail) = r.tail();
    let remaining = tail?;

    for _byte in remaining {
      r.skip_white_spaces_and_comments();

      let Ok(explicit_value) = r.forward_tag(b">>");

      match explicit_value {
        Some(()) => {
          return Some(());
        }
        None => {
          let () = match r.skip::<Name<'_>>(mode) {
            Some(_) => (),
            None => {
              r.skip::<Object<'_>>(mode)?;
              continue;
            }
          };

          r.skip_white_spaces_and_comments();

          match mode {
            ObjectReadMode::ContentStream => {
              r.skip::<Object<'_>>(mode)?;
            }
            ObjectReadMode::Document => {
              r.skip::<MaybeRef<Object<'_>>>(mode)?;
            }
          }
        }
      }
    }

    None
  }
}

impl<'a> Readable<'a> for Dict<'a> {
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    let Ok(inner) = read_inner(r, ctx, Some(b"<<"), b">>");

    inner
  }
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The dictionary index and read context are immutable and can then be cloned without copying entries"
  )
)]
fn read_inner<'a>(
  r: &mut Reader<'a>,
  ctx: &ReaderContext<'a>,
  start_tag: Option<&[u8]>,
  end_tag: &[u8],
) -> core::result::Result<Option<Dict<'a>>, console_core_never::Never> {
  #[cfg(feature = "std")]
  let mut offsets = FixedMap::with_capacity_and_hasher(8, Default::default());
  #[cfg(not(feature = "std"))]
  let mut offsets = FixedMap::new();

  let Ok(parsed) = parse_dict_with(
    r,
    ctx,
    start_tag,
    end_tag,
    &mut offsets,
    |entries, name, offset| {
      let _previous_offset = entries.insert(name, offset);

      Ok(())
    },
  );
  let data = match parsed {
    Some(data) => data,
    None => return Ok(None),
  };

  Ok(Some(Dict(Inner::Some(Arc::new(Repr {
    data,
    offsets,
    ctx: ctx.clone(),
  })))))
}

pub(crate) struct DictProbe<'a> {
  pub(crate) data: &'a [u8],
  pub(crate) root_key: crate::ContainsKeyAnswer,
  pub(crate) type_key: crate::ContainsKeyAnswer,
}

pub(crate) fn probe_dict<'a>(
  r: &mut Reader<'a>,
  ctx: &ReaderContext<'a>,
  start_tag: Option<&[u8]>,
  end_tag: &[u8],
) -> core::result::Result<Option<DictProbe<'a>>, console_core_never::Never> {
  let mut probe = DictProbe {
    data: &[],
    root_key: crate::ContainsKeyAnswer::No,
    type_key: crate::ContainsKeyAnswer::No,
  };
  let Ok(parsed) = parse_dict_with(
    r,
    ctx,
    start_tag,
    end_tag,
    &mut probe,
    |probe_2, name, _offset| {
      match name.as_ref() {
        keys::ROOT => probe_2.root_key = crate::ContainsKeyAnswer::Yes,
        keys::TYPE => probe_2.type_key = crate::ContainsKeyAnswer::Yes,
        _other_key => {}
      }

      Ok(())
    },
  );

  match parsed {
    Some(data) => {
      probe.data = data;

      Ok(Some(probe))
    }
    None => Ok(None),
  }
}

fn parse_dict_with<'a, State, F>(
  r: &mut Reader<'a>,
  ctx: &ReaderContext<'a>,
  start_tag: Option<&[u8]>,
  end_tag: &[u8],
  state: &mut State,
  mut on_entry: F,
) -> core::result::Result<Option<&'a [u8]>, console_core_never::Never>
where
  F: FnMut(&mut State, Name<'a>, u64) -> core::result::Result<(), console_core_never::Never>,
{
  let Ok(tail) = r.tail();
  let dict_data = match tail {
    Some(data) => data,
    None => return Ok(None),
  };
  let Ok(start_offset) = r.fixed_offset();

  match start_tag {
    Some(tag) => {
      let Ok(opened) = r.forward_tag(tag);

      match opened {
        Some(()) => {}
        None => return Ok(None),
      }
    }
    None => {}
  }

  let Ok(mode) = ctx.read_mode();

  for _byte in dict_data {
    r.skip_white_spaces_and_comments();

    let Ok(closed) = r.forward_tag(end_tag);

    match closed {
      Some(()) => {
        let Ok(end) = r.fixed_offset();
        let length = match end.checked_sub(start_offset) {
          Some(length) => length,
          None => return Ok(None),
        };
        let Ok(position) = index(length);
        return Ok(dict_data.get(..position));
      }
      None => {}
    }

    let name = match r.read_without_context::<Name<'_>>() {
      Some(name) => name,
      None => match start_tag {
        Some(_tag) => match r.read::<Object<'_>>(ctx) {
          Some(_unexpected_object) => continue,
          None => return Ok(None),
        },
        None => return Ok(None),
      },
    };

    r.skip_white_spaces_and_comments();

    let Ok(position) = r.fixed_offset();
    let offset = match position.checked_sub(start_offset) {
      Some(offset) => offset,
      None => return Ok(None),
    };
    let Ok(()) = on_entry(state, name, offset);
    let skipped = match mode {
      ObjectReadMode::ContentStream => r.skip::<Object<'_>>(mode),
      ObjectReadMode::Document => r.skip::<MaybeRef<Object<'_>>>(mode),
    };

    match skipped {
      Some(_value) => {}
      None => return Ok(None),
    }
  }

  Ok(None)
}

object!(Dict<'a>, Dict);

struct Repr<'a> {
  data: &'a [u8],
  offsets: FixedMap<Name<'a>, u64>,
  ctx: ReaderContext<'a>,
}

pub(crate) struct InlineImageDict<'a>(Dict<'a>);

impl<'a> InlineImageDict<'a> {
  pub(crate) fn get_dict(&self) -> core::result::Result<&Dict<'a>, console_core_never::Never> {
    Ok(&self.0)
  }
}

impl<'a> Readable<'a> for InlineImageDict<'a> {
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    let Ok(inner) = read_inner(r, ctx, None, b"ID");

    let dictionary = inner?;
    Some(Self(dictionary))
  }
}

#[doc = "PDF names used as dictionary keys."]
pub mod keys {
  #[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(
      explicit080_macro_budget,
      reason = "Each declaration maps a Rust identifier to its exact PDF byte name without repeating the visibility and type"
    )
  )]
  macro_rules! key {
    ($i:ident, $e:expr) => {
      #[doc = concat!("PDF key ", stringify!($i), ".")]
      pub const $i: &'static [u8] = $e;
    };
  }

  key!(A, b"A");
  key!(AA, b"AA");
  key!(ABSOLUTE_COLORIMETRIC, b"AbsoluteColorimetric");
  key!(AC, b"AC");
  key!(ACRO_FORM, b"AcroForm");
  key!(ACTUAL_TEXT, b"ActualText");
  key!(ADBE, b"ADBE");
  key!(ADBE_PKCS7_DETACHED, b"adbe.pkcs7.detached");
  key!(ADBE_PKCS7_SHA1, b"adbe.pkcs7.sha1");
  key!(ADBE_X509_RSA_SHA1, b"adbe.x509.rsa_sha1");
  key!(ADOBE_PPKLITE, b"Adobe.PPKLite");
  key!(AESV2, b"AESV2");
  key!(AESV3, b"AESV3");
  key!(AF, b"AF");
  key!(AF_RELATIONSHIP, b"AFRelationship");
  key!(AFTER, b"After");
  key!(AI_META_DATA, b"AIMetaData");
  key!(AIS, b"AIS");
  key!(ALL_OFF, b"AllOff");
  key!(ALL_ON, b"AllOn");
  key!(ALT, b"Alt");
  key!(ALPHA, b"Alpha");
  key!(ALTERNATE, b"Alternate");
  key!(ANNOT, b"Annot");
  key!(ANNOTS, b"Annots");
  key!(ANTI_ALIAS, b"AntiAlias");
  key!(ANY_OFF, b"AnyOff");
  key!(ANY_ON, b"AnyOn");
  key!(AP, b"AP");
  key!(AP_REF, b"APRef");
  key!(APP, b"App");
  key!(ART_BOX, b"ArtBox");
  key!(ARTIFACT, b"Artifact");
  key!(AS, b"AS");
  key!(ASCENT, b"Ascent");
  key!(ASCII_HEX_DECODE, b"ASCIIHexDecode");
  key!(ASCII_HEX_DECODE_ABBREVIATION, b"AHx");
  key!(ASCII85_DECODE, b"ASCII85Decode");
  key!(ASCII85_DECODE_ABBREVIATION, b"A85");
  key!(ATTACHED, b"Attached");
  key!(AUTHOR, b"Author");
  key!(AVG_WIDTH, b"AvgWidth");

  key!(B, b"B");
  key!(BACKGROUND, b"Background");
  key!(BASE_ENCODING, b"BaseEncoding");
  key!(BASE_FONT, b"BaseFont");
  key!(BASE_STATE, b"BaseState");
  key!(BASE_VERSION, b"BaseVersion");
  key!(BBOX, b"BBox");
  key!(BC, b"BC");
  key!(BE, b"BE");
  key!(BEAD, b"BEAD");
  key!(BEFORE, b"Before");
  key!(BG, b"BG");
  key!(BITS_PER_COMPONENT, b"BitsPerComponent");
  key!(BITS_PER_COORDINATE, b"BitsPerCoordinate");
  key!(BITS_PER_FLAG, b"BitsPerFlag");
  key!(BITS_PER_SAMPLE, b"BitsPerSample");
  key!(BL, b"Bl");
  key!(BLACK_IS_1, b"BlackIs1");
  key!(BLACK_POINT, b"BlackPoint");
  key!(BLEED_BOX, b"BleedBox");
  key!(BM, b"BM");
  key!(BORDER, b"Border");
  key!(BOUNDS, b"Bounds");
  key!(BPC, b"BPC");
  key!(BS, b"BS");
  key!(BTN, b"Btn");
  key!(BYTERANGE, b"ByteRange");

  key!(C, b"C");
  key!(C0, b"C0");
  key!(C1, b"C1");
  key!(CA, b"CA");
  key!(CA_NS, b"ca");
  key!(CALGRAY, b"CalGray");
  key!(CALRGB, b"CalRGB");
  key!(CALCMYK, b"CalCMYK");
  key!(CAP, b"Cap");
  key!(CAP_HEIGHT, b"CapHeight");
  key!(CATALOG, b"Catalog");
  key!(CCITTFAX_DECODE, b"CCITTFaxDecode");
  key!(CCITTFAX_DECODE_ABBREVIATION, b"CCF");
  key!(CENTER_WINDOW, b"CenterWindow");
  key!(CERT, b"Cert");
  key!(CERTS, b"Certs");
  key!(CF, b"CF");
  key!(CFM, b"CFM");
  key!(CH, b"Ch");
  key!(CHAR_PROCS, b"CharProcs");
  key!(CHAR_SET, b"CharSet");
  key!(CHECK_SUM, b"CheckSum");
  key!(CI, b"CI");
  key!(CICI_SIGNIT, b"CICI.SignIt");
  key!(CID_FONT_TYPE0, b"CIDFontType0");
  key!(CID_FONT_TYPE0C, b"CIDFontType0C");
  key!(CID_FONT_TYPE2, b"CIDFontType2");
  key!(CID_TO_GID_MAP, b"CIDToGIDMap");
  key!(CID_SET, b"CIDSet");
  key!(CIDSYSTEMINFO, b"CIDSystemInfo");
  key!(CL, b"CL");
  key!(CLASS_MAP, b"ClassMap");
  key!(CLR_F, b"ClrF");
  key!(CLR_FF, b"ClrFf");
  key!(CMAP, b"CMap");
  key!(CMAPNAME, b"CMapName");
  key!(CMYK, b"CMYK");
  key!(CO, b"CO");
  key!(COLOR, b"Color");
  key!(COLLECTION, b"Collection");
  key!(COLLECTION_ITEM, b"CollectionItem");
  key!(COLLECTION_FIELD, b"CollectionField");
  key!(COLLECTION_SCHEMA, b"CollectionSchema");
  key!(COLLECTION_SORT, b"CollectionSort");
  key!(COLLECTION_SUBITEM, b"CollectionSubitem");
  key!(COLOR_BURN, b"ColorBurn");
  key!(COLOR_DODGE, b"ColorDodge");
  key!(COLORANTS, b"Colorants");
  key!(COLORS, b"Colors");
  key!(COLORSPACE, b"ColorSpace");
  key!(COLOR_TRANSFORM, b"ColorTransform");
  key!(COLUMNS, b"Columns");
  key!(COMPATIBLE, b"Compatible");
  key!(COMPONENTS, b"Components");
  key!(CONTACT_INFO, b"ContactInfo");
  key!(CONTENTS, b"Contents");
  key!(COORDS, b"Coords");
  key!(COUNT, b"Count");
  key!(CP, b"CP");
  key!(CREATION_DATE, b"CreationDate");
  key!(CREATOR, b"Creator");
  key!(CRL, b"CRL");
  key!(CRLS, b"CRLS");
  key!(CROP_BOX, b"CropBox");
  key!(CRYPT, b"Crypt");
  key!(CS, b"CS");
  key!(CYX, b"CYX");

  key!(D, b"D");
  key!(DA, b"DA");
  key!(DARKEN, b"Darken");
  key!(DATE, b"Date");
  key!(DCT_DECODE, b"DCTDecode");
  key!(DCT_DECODE_ABBREVIATION, b"DCT");
  key!(DECODE, b"Decode");
  key!(DECODE_PARMS, b"DecodeParms");
  key!(DEFAULT, b"default");
  key!(DEFAULT_CMYK, b"DefaultCMYK");
  key!(DEFAULT_CRYPT_FILTER, b"DefaultCryptFilter");
  key!(DEFAULT_GRAY, b"DefaultGray");
  key!(DEFAULT_RGB, b"DefaultRGB");
  key!(DESC, b"Desc");
  key!(DESCENDANT_FONTS, b"DescendantFonts");
  key!(DESCENT, b"Descent");
  key!(DEST, b"Dest");
  key!(DEST_OUTPUT_PROFILE, b"DestOutputProfile");
  key!(DESTS, b"Dests");
  key!(DEVICE_CMYK, b"DeviceCMYK");
  key!(DEVICE_GRAY, b"DeviceGray");
  key!(DEVICE_N, b"DeviceN");
  key!(DEVICE_RGB, b"DeviceRGB");
  key!(DI, b"Di");
  key!(DIFFERENCE, b"Difference");
  key!(DIFFERENCES, b"Differences");
  key!(DIGEST_METHOD, b"DigestMethod");
  key!(DIGEST_RIPEMD160, b"RIPEMD160");
  key!(DIGEST_SHA1, b"SHA1");
  key!(DIGEST_SHA256, b"SHA256");
  key!(DIGEST_SHA384, b"SHA384");
  key!(DIGEST_SHA512, b"SHA512");
  key!(DIRECTION, b"Direction");
  key!(DISPLAY_DOC_TITLE, b"DisplayDocTitle");
  key!(DL, b"DL");
  key!(DM, b"Dm");
  key!(DOC, b"Doc");
  key!(DOC_CHECKSUM, b"DocChecksum");
  key!(DOC_TIME_STAMP, b"DocTimeStamp");
  key!(DOCMDP, b"DocMDP");
  key!(DOCUMENT, b"Document");
  key!(DOMAIN, b"Domain");
  key!(DOS, b"DOS");
  key!(DP, b"DP");
  key!(DR, b"DR");
  key!(DS, b"DS");
  key!(DSS, b"DSS");
  key!(DUPLEX, b"Duplex");
  key!(DUR, b"Dur");
  key!(DV, b"DV");
  key!(DW, b"DW");
  key!(DW2, b"DW2");

  key!(E, b"E");
  key!(EARLY_CHANGE, b"EarlyChange");
  key!(EF, b"EF");
  key!(EMBEDDED_FDFS, b"EmbeddedFDFs");
  key!(EMBEDDED_FILE, b"EmbeddedFile");
  key!(EMBEDDED_FILES, b"EmbeddedFiles");
  key!(EMPTY, b"");
  key!(ENCODE, b"Encode");
  key!(ENCODED_BYTE_ALIGN, b"EncodedByteAlign");
  key!(ENCODING, b"Encoding");
  key!(ENCODING_90MS_RKSJ_H, b"90ms-RKSJ-H");
  key!(ENCODING_90MS_RKSJ_V, b"90ms-RKSJ-V");
  key!(ENCODING_ETEN_B5_H, b"ETen-B5-H");
  key!(ENCODING_ETEN_B5_V, b"ETen-B5-V");
  key!(ENCRYPT, b"Encrypt");
  key!(ENCRYPT_META_DATA, b"EncryptMetadata");
  key!(ENCRYPTED_PAYLOAD, b"EncryptedPayload");
  key!(END_OF_BLOCK, b"EndOfBlock");
  key!(END_OF_LINE, b"EndOfLine");
  key!(ENTRUST_PPKEF, b"Entrust.PPKEF");
  key!(EXCLUSION, b"Exclusion");
  key!(EXTENSIONS, b"Extensions");
  key!(EXTENSION_LEVEL, b"ExtensionLevel");
  key!(EX_DATA, b"ExData");
  key!(EXPORT, b"Export");
  key!(EXPORT_STATE, b"ExportState");
  key!(EXT_G_STATE, b"ExtGState");
  key!(EXTEND, b"Extend");
  key!(EXTENDS, b"Extends");

  key!(F, b"F");
  key!(F_DECODE_PARMS, b"FDecodeParms");
  key!(F_FILTER, b"FFilter");
  key!(FB, b"FB");
  key!(FDF, b"FDF");
  key!(FF, b"Ff");
  key!(FIELDS, b"Fields");
  key!(FILESPEC, b"Filespec");
  key!(FILTER, b"Filter");
  key!(FIRST, b"First");
  key!(FIRST_CHAR, b"FirstChar");
  key!(FIT_WINDOW, b"FitWindow");
  key!(FL, b"FL");
  key!(FLAGS, b"Flags");
  key!(FLATE_DECODE, b"FlateDecode");
  key!(FLATE_DECODE_ABBREVIATION, b"Fl");
  key!(FO, b"Fo");
  key!(FOLDERS, b"Folders");
  key!(FONT, b"Font");
  key!(FONT_BBOX, b"FontBBox");
  key!(FONT_DESC, b"FontDescriptor");
  key!(FONT_FAMILY, b"FontFamily");
  key!(FONT_FILE, b"FontFile");
  key!(FONT_FILE2, b"FontFile2");
  key!(FONT_FILE3, b"FontFile3");
  key!(FONT_MATRIX, b"FontMatrix");
  key!(FONT_NAME, b"FontName");
  key!(FONT_STRETCH, b"FontStretch");
  key!(FONT_WEIGHT, b"FontWeight");
  key!(FORM, b"Form");
  key!(FORMTYPE, b"FormType");
  key!(FRM, b"FRM");
  key!(FS, b"FS");
  key!(FT, b"FT");
  key!(FUNCTION, b"Function");
  key!(FUNCTION_TYPE, b"FunctionType");
  key!(FUNCTIONS, b"Functions");

  key!(G, b"G");
  key!(GAMMA, b"Gamma");
  key!(GROUP, b"Group");
  key!(GTS_PDFA1, b"GTS_PDFA1");

  key!(H, b"H");
  key!(HARD_LIGHT, b"HardLight");
  key!(HEIGHT, b"Height");
  key!(HELV, b"Helv");
  key!(HIDE_MENUBAR, b"HideMenubar");
  key!(HIDE_TOOLBAR, b"HideToolbar");
  key!(HIDE_WINDOWUI, b"HideWindowUI");
  key!(HUE, b"Hue");

  key!(I, b"I");
  key!(IC, b"IC");
  key!(ICC_BASED, b"ICCBased");
  key!(ID, b"ID");
  key!(ID_TREE, b"IDTree");
  key!(IDENTITY, b"Identity");
  key!(IDENTITY_H, b"Identity-H");
  key!(IDENTITY_V, b"Identity-V");
  key!(IF, b"IF");
  key!(ILLUSTRATOR, b"Illustrator");
  key!(IM, b"IM");
  key!(IMAGE, b"Image");
  key!(IMAGE_MASK, b"ImageMask");
  key!(INDEX, b"Index");
  key!(INDEXED, b"Indexed");
  key!(INFO, b"Info");
  key!(INKLIST, b"InkList");
  key!(INTENT, b"Intent");
  key!(INTERPOLATE, b"Interpolate");
  key!(IRT, b"IRT");
  key!(IT, b"IT");
  key!(ITALIC_ANGLE, b"ItalicAngle");
  key!(ISSUER, b"Issuer");
  key!(IX, b"IX");

  key!(JAVA_SCRIPT, b"JavaScript");
  key!(JBIG2_DECODE, b"JBIG2Decode");
  key!(JBIG2_GLOBALS, b"JBIG2Globals");
  key!(JPX_DECODE, b"JPXDecode");
  key!(JS, b"JS");

  key!(K, b"K");
  key!(KEYWORDS, b"Keywords");
  key!(KEY_USAGE, b"KeyUsage");
  key!(KIDS, b"Kids");

  key!(L, b"L");
  key!(LAB, b"Lab");
  key!(LANG, b"Lang");
  key!(LAST, b"Last");
  key!(LAST_CHAR, b"LastChar");
  key!(LAST_MODIFIED, b"LastModified");
  key!(LC, b"LC");
  key!(LE, b"LE");
  key!(LEADING, b"Leading");
  key!(LEGAL_ATTESTATION, b"LegalAttestation");
  key!(LENGTH, b"Length");
  key!(LENGTH1, b"Length1");
  key!(LENGTH2, b"Length2");
  key!(LENGTH3, b"Length3");
  key!(LIGHTEN, b"Lighten");
  key!(LIMITS, b"Limits");
  key!(LINEARIZED, b"Linearized");
  key!(LJ, b"LJ");
  key!(LL, b"LL");
  key!(LLE, b"LLE");
  key!(LLO, b"LLO");
  key!(LOCATION, b"Location");
  key!(LUMINOSITY, b"Luminosity");
  key!(LW, b"LW");
  key!(LZW_DECODE, b"LZWDecode");
  key!(LZW_DECODE_ABBREVIATION, b"LZW");

  key!(M, b"M");
  key!(MAC, b"Mac");
  key!(MAC_EXPERT_ENCODING, b"MacExpertEncoding");
  key!(MAC_ROMAN_ENCODING, b"MacRomanEncoding");
  key!(MARK_INFO, b"MarkInfo");
  key!(MASK, b"Mask");
  key!(MATRIX, b"Matrix");
  key!(MATTE, b"Matte");
  key!(MAX_LEN, b"MaxLen");
  key!(MAX_WIDTH, b"MaxWidth");
  key!(MCID, b"MCID");
  key!(MDP, b"MDP");
  key!(MEDIA_BOX, b"MediaBox");
  key!(MEASURE, b"Measure");
  key!(METADATA, b"Metadata");
  key!(MISSING_WIDTH, b"MissingWidth");
  key!(MIX, b"Mix");
  key!(MK, b"MK");
  key!(ML, b"ML");
  key!(MM_TYPE1, b"MMType1");
  key!(MOD_DATE, b"ModDate");
  key!(MULTIPLY, b"Multiply");

  key!(N, b"N");
  key!(NAME, b"Name");
  key!(NAMES, b"Names");
  key!(NAVIGATOR, b"Navigator");
  key!(NEED_APPEARANCES, b"NeedAppearances");
  key!(NEW_WINDOW, b"NewWindow");
  key!(NEXT, b"Next");
  key!(NM, b"NM");
  key!(NON_EFONT_NO_WARN, b"NonEFontNoWarn");
  key!(NON_FULL_SCREEN_PAGE_MODE, b"NonFullScreenPageMode");
  key!(NONE, b"None");
  key!(NORMAL, b"Normal");
  key!(NUMS, b"Nums");

  key!(O, b"O");
  key!(OBJ, b"Obj");
  key!(OBJR, b"OBJR");
  key!(OBJ_STM, b"ObjStm");
  key!(OC, b"OC");
  key!(OCG, b"OCG");
  key!(OCGS, b"OCGs");
  key!(OCMD, b"OCMD");
  key!(OCPROPERTIES, b"OCProperties");
  key!(OCSP, b"OCSP");
  key!(OCSPS, b"OCSPs");
  key!(OE, b"OE");
  key!(OID, b"OID");
  key!(OFF, b"OFF");
  key!(ON, b"ON");
  key!(OP, b"OP");
  key!(OP_NS, b"op");
  key!(OPEN_ACTION, b"OpenAction");
  key!(OPEN_TYPE, b"OpenType");
  key!(OPI, b"OPI");
  key!(OPM, b"OPM");
  key!(OPT, b"Opt");
  key!(ORDER, b"Order");
  key!(ORDERING, b"Ordering");
  key!(OS, b"OS");
  key!(OUTLINES, b"Outlines");
  key!(OUTPUT_CONDITION, b"OutputCondition");
  key!(OUTPUT_CONDITION_IDENTIFIER, b"OutputConditionIdentifier");
  key!(OUTPUT_INTENT, b"OutputIntent");
  key!(OUTPUT_INTENTS, b"OutputIntents");
  key!(OVERLAY, b"Overlay");

  key!(P, b"P");
  key!(PA, b"PA");
  key!(PAGE, b"Page");
  key!(PAGE_LABELS, b"PageLabels");
  key!(PAGE_LAYOUT, b"PageLayout");
  key!(PAGE_MODE, b"PageMode");
  key!(PAGES, b"Pages");
  key!(PAINT_TYPE, b"PaintType");
  key!(PANOSE, b"Panose");
  key!(PARAMS, b"Params");
  key!(PARENT, b"Parent");
  key!(PARENT_TREE, b"ParentTree");
  key!(PARENT_TREE_NEXT_KEY, b"ParentTreeNextKey");
  key!(PART, b"Part");
  key!(PATH, b"Path");
  key!(PATTERN, b"Pattern");
  key!(PATTERN_TYPE, b"PatternType");
  key!(PC, b"PC");
  key!(PDF_DOC_ENCODING, b"PDFDocEncoding");
  key!(PERMS, b"Perms");
  key!(PERCEPTUAL, b"Perceptual");
  key!(PIECE_INFO, b"PieceInfo");
  key!(PG, b"Pg");
  key!(PI, b"PI");
  key!(PO, b"PO");
  key!(POPUP, b"Popup");
  key!(PRE_RELEASE, b"PreRelease");
  key!(PREDICTOR, b"Predictor");
  key!(PREV, b"Prev");
  key!(PRINT, b"Print");
  key!(PRINT_AREA, b"PrintArea");
  key!(PRINT_CLIP, b"PrintClip");
  key!(PRINT_SCALING, b"PrintScaling");
  key!(PRINT_STATE, b"PrintState");
  key!(PRIVATE, b"Private");
  key!(PROC_SET, b"ProcSet");
  key!(PROCESS, b"Process");
  key!(PRODUCER, b"Producer");
  key!(PROP_BUILD, b"Prop_Build");
  key!(PROPERTIES, b"Properties");
  key!(PS, b"PS");
  key!(PT_DATA, b"PtData");
  key!(PUB_SEC, b"PubSec");
  key!(PV, b"PV");

  key!(Q, b"Q");
  key!(QUADPOINTS, b"QuadPoints");

  key!(R, b"R");
  key!(RANGE, b"Range");
  key!(RC, b"RC");
  key!(RD, b"RD");
  key!(REASON, b"Reason");
  key!(REASONS, b"Reasons");
  key!(RECIPIENTS, b"Recipients");
  key!(RECT, b"Rect");
  key!(REF, b"Ref");
  key!(REFERENCE, b"Reference");
  key!(REGISTRY, b"Registry");
  key!(REGISTRY_NAME, b"RegistryName");
  key!(RELATIVE_COLORIMETRIC, b"RelativeColorimetric");
  key!(RENAME, b"Rename");
  key!(REPEAT, b"Repeat");
  key!(RES_FORK, b"ResFork");
  key!(RESOURCES, b"Resources");
  key!(RGB, b"RGB");
  key!(RI, b"RI");
  key!(ROLE_MAP, b"RoleMap");
  key!(ROOT, b"Root");
  key!(ROTATE, b"Rotate");
  key!(ROWS, b"Rows");
  key!(RT, b"RT");
  key!(RUN_LENGTH_DECODE, b"RunLengthDecode");
  key!(RUN_LENGTH_DECODE_ABBREVIATION, b"RL");
  key!(RV, b"RV");

  key!(S, b"S");
  key!(SA, b"SA");
  key!(SATURATION, b"Saturation");
  key!(SCHEMA, b"Schema");
  key!(SCREEN, b"Screen");
  key!(SE, b"SE");
  key!(SEPARATION, b"Separation");
  key!(SET_F, b"SetF");
  key!(SET_FF, b"SetFf");
  key!(SHADING, b"Shading");
  key!(SHADING_TYPE, b"ShadingType");
  key!(SIG, b"Sig");
  key!(SIG_FLAGS, b"SigFlags");
  key!(SIG_REF, b"SigRef");
  key!(SIZE, b"Size");
  key!(SM, b"SM");
  key!(SMASK, b"SMask");
  key!(SMASK_IN_DATA, b"SMaskInData");
  key!(SOFT_LIGHT, b"SoftLight");
  key!(SORT, b"Sort");
  key!(SOUND, b"Sound");
  key!(SPLIT, b"Split");
  key!(SS, b"SS");
  key!(ST, b"St");
  key!(STANDARD_ENCODING, b"StandardEncoding");
  key!(STATE, b"State");
  key!(STATE_MODEL, b"StateModel");
  key!(STATUS, b"Status");
  key!(STD_CF, b"StdCF");
  key!(STEM_H, b"StemH");
  key!(STEM_V, b"StemV");
  key!(STM_F, b"StmF");
  key!(STR_F, b"StrF");
  key!(STRUCT_ELEM, b"StructElem");
  key!(STRUCT_PARENT, b"StructParent");
  key!(STRUCT_PARENTS, b"StructParents");
  key!(STRUCT_TREE_ROOT, b"StructTreeRoot");
  key!(STYLE, b"Style");
  key!(SUB_FILTER, b"SubFilter");
  key!(SUBJ, b"Subj");
  key!(SUBJECT, b"Subject");
  key!(SUBJECT_DN, b"SubjectDN");
  key!(SUBTYPE, b"Subtype");
  key!(SUPPLEMENT, b"Supplement");
  key!(SV, b"SV");
  key!(SV_CERT, b"SVCert");
  key!(SW, b"SW");
  key!(SY, b"Sy");
  key!(SYNCHRONOUS, b"Synchronous");
  key!(T, b"T");
  key!(TARGET, b"Target");
  key!(TEMPLATES, b"Templates");
  key!(THREAD, b"Thread");
  key!(THREADS, b"Threads");
  key!(THREE_DD, b"3DD");
  key!(THUMB, b"Thumb");
  key!(TI, b"TI");
  key!(TILING_TYPE, b"TilingType");
  key!(TIME_STAMP, b"TimeStamp");
  key!(TITLE, b"Title");
  key!(TK, b"TK");
  key!(TM, b"TM");
  key!(TO_UNICODE, b"ToUnicode");
  key!(TR, b"TR");
  key!(TR2, b"TR2");
  key!(TRAPPED, b"Trapped");
  key!(TRANS, b"Trans");
  key!(TRANSFORM_METHOD, b"TransformMethod");
  key!(TRANSFORM_PARAMS, b"TransformParams");
  key!(TRANSPARENCY, b"Transparency");
  key!(TREF, b"TRef");
  key!(TRIM_BOX, b"TrimBox");
  key!(TRUE_TYPE, b"TrueType");
  key!(TRUSTED_MODE, b"TrustedMode");
  key!(TU, b"TU");
  key!(TX, b"Tx");
  key!(TYPE, b"Type");
  key!(TYPE0, b"Type0");
  key!(TYPE1, b"Type1");
  key!(TYPE1C, b"Type1C");
  key!(TYPE3, b"Type3");

  key!(U, b"U");
  key!(UE, b"UE");
  key!(UF, b"UF");
  key!(UNCHANGED, b"Unchanged");
  key!(UNIX, b"Unix");
  key!(URI, b"URI");
  key!(URL, b"URL");
  key!(URL_TYPE, b"URLType");
  key!(USAGE, b"Usage");
  key!(USE_CMAP, b"UseCMap");
  key!(USER_UNIT, b"UserUnit");

  key!(V, b"V");
  key!(VE, b"VE");
  key!(VERISIGN_PPKVS, b"VeriSign.PPKVS");
  key!(VERSION, b"Version");
  key!(VERTICES, b"Vertices");
  key!(VERTICES_PER_ROW, b"VerticesPerRow");
  key!(VIEW, b"View");
  key!(VIEW_AREA, b"ViewArea");
  key!(VIEW_CLIP, b"ViewClip");
  key!(VIEW_STATE, b"ViewState");
  key!(VIEWER_PREFERENCES, b"ViewerPreferences");
  key!(VOLUME, b"Volume");
  key!(VP, b"VP");
  key!(VRI, b"VRI");

  key!(W, b"W");
  key!(W2, b"W2");
  key!(WC, b"WC");
  key!(WHITE_POINT, b"WhitePoint");
  key!(WIDGET, b"Widget");
  key!(WIDTH, b"Width");
  key!(WIDTHS, b"Widths");
  key!(WIN, b"Win");
  key!(WIN_ANSI_ENCODING, b"WinAnsiEncoding");
  key!(WMODE, b"WMode");
  key!(WP, b"WP");
  key!(WS, b"WS");

  key!(X, b"X");
  key!(XFA, b"XFA");
  key!(X_STEP, b"XStep");
  key!(XHEIGHT, b"XHeight");
  key!(XOBJECT, b"XObject");
  key!(XREF, b"XRef");
  key!(XREF_STM, b"XRefStm");

  key!(Y, b"Y");
  key!(Y_STEP, b"YStep");
  key!(YES, b"Yes");

  key!(ZA_DB, b"ZaDb");
}

#[cfg(test)]
mod tests {
  use crate::ContainsKeyAnswer;
  use crate::object::Number;
  use crate::object::dict::keys::{COLORSPACE, EXT_G_STATE, FONT, PROC_SET};
  use crate::object::dict::{Dict, InlineImageDict};
  use crate::object::string;
  use crate::object::{Name, ObjRef};
  use crate::reader::Reader;
  use crate::reader::{ReaderContext, ReaderExt};
  use crate::test_support::{Required, TestFailure};
  use alloc::format;

  fn dict_impl(data: &[u8]) -> Result<Dict<'_>, TestFailure> {
    let Ok(mut reader) = Reader::new(data);
    let Ok(context) = ReaderContext::dummy();

    reader.read_with_context::<Dict<'_>>(&context).required()
  }

  #[test]
  fn debug_dictionaries_close_their_fields() -> Result<(), TestFailure> {
    let empty = dict_impl(b"<<>>")?;
    let dict = dict_impl(b"<</Enabled true>>")?;

    assert_eq!(format!("{empty:?}"), "Dict");
    assert_eq!(format!("{dict:?}"), "Dict { \"Enabled\": Boolean(true) }");

    Ok(())
  }

  fn name(bytes: &[u8]) -> Result<Name<'_>, console_core_never::Never> {
    Name::new_unescaped(bytes)
  }

  #[test]
  fn empty_dict_1() -> Result<(), TestFailure> {
    let dict = dict_impl(b"<<>>")?;

    assert_eq!(dict.len(), Ok(0));

    Ok(())
  }

  #[test]
  fn empty_dict_2() -> Result<(), TestFailure> {
    let dict = dict_impl(b"<<   \n >>")?;

    assert_eq!(dict.len(), Ok(0));

    Ok(())
  }

  #[test]
  fn dict_1() -> Result<(), TestFailure> {
    let dict = dict_impl(b"<<  /Hi 34.0 >>")?;
    let Ok(hi) = name(b"Hi");

    assert_eq!(dict.len(), Ok(1));
    assert!(matches!(dict.get::<Number>(hi), Ok(Some(_))));

    Ok(())
  }

  #[test]
  fn dict_2() -> Result<(), TestFailure> {
    let dict = dict_impl(b"<<  /Hi \n 34.0 /Second true >>")?;
    let (Ok(hi), Ok(second)) = (name(b"Hi"), name(b"Second"));

    assert_eq!(dict.len(), Ok(2));
    assert!(matches!(dict.get::<Number>(hi), Ok(Some(_))));
    assert!(matches!(
      dict.get::<crate::object::PdfBoolean>(second),
      Ok(Some(_))
    ));

    Ok(())
  }

  #[test]
  fn dict_complex() -> Result<(), TestFailure> {
    let data = "<< /Type /Example
/Subtype /DictionaryExample
/Version 0.01
/IntegerItem 12
/StringItem ( a string )
/Subdictionary << /Item1 0.4
                /Item2 true
                /LastItem ( not ! )
                /VeryLastItem ( OK )
                >>
>>";

    let dict = dict_impl(data.as_bytes())?;
    assert_eq!(dict.len(), Ok(6));
    assert!(matches!(dict.get::<Name<'_>>(b"Type"), Ok(Some(_))));
    assert!(matches!(dict.get::<Name<'_>>(b"Subtype"), Ok(Some(_))));
    assert!(matches!(dict.get::<Number>(b"Version"), Ok(Some(_))));
    assert!(matches!(dict.get::<i32>(b"IntegerItem"), Ok(Some(_))));
    assert!(matches!(
      dict.get::<string::String<'_>>(b"StringItem"),
      Ok(Some(_))
    ));
    assert!(matches!(
      dict.get::<Dict<'_>>(b"Subdictionary"),
      Ok(Some(_))
    ));

    Ok(())
  }

  #[test]
  fn dict_with_trailing() -> Result<(), TestFailure> {
    let dict = dict_impl(b"<<  /Hi 67.0  >>trailing data")?;

    assert_eq!(dict.len(), Ok(1));

    Ok(())
  }

  #[test]
  fn dict_with_comment() -> Result<(), TestFailure> {
    let dict = dict_impl(b"<<  /Hi % A comment \n 67.0 % Another comment \n >>")?;

    assert_eq!(dict.len(), Ok(1));

    Ok(())
  }

  #[test]
  fn inline_dict() -> Result<(), TestFailure> {
    let dict_data = b"/W 17 /H 17 /CS /RGB /BPC 8 /F [ /A85 /LZW ] ID ";

    let Ok(mut reader) = Reader::new(dict_data);
    let Ok(context) = ReaderContext::dummy();
    let dict = reader
      .read_with_context::<InlineImageDict<'_>>(&context)
      .required()?;
    let Ok(held) = dict.get_dict();

    assert_eq!(held.len(), Ok(5));

    Ok(())
  }

  #[test]
  fn dict_with_escaped_name() -> Result<(), TestFailure> {
    let dict = dict_impl(b"<< /PANTONE#20104#20C 234 >>")?;

    assert_eq!(
      dict.contains_key("PANTONE 104 C"),
      Ok(ContainsKeyAnswer::Yes)
    );

    Ok(())
  }

  #[test]
  fn garbage_in_between() -> Result<(), TestFailure> {
    let dict_data = b"<<
/ProcSet [ /PDF /Text ]
/Font << /F4 31 0 R /F6 23 0 R >>
/ExtGState << /GS2 14 0 R
2000
 /GS3 15 0 R >>
/ColorSpace << /Cs5 13 0 R >>
>> ";
    let dict = dict_impl(dict_data)?;

    assert_eq!(dict.contains_key(PROC_SET), Ok(ContainsKeyAnswer::Yes));
    assert_eq!(dict.contains_key(FONT), Ok(ContainsKeyAnswer::Yes));
    assert_eq!(dict.contains_key(EXT_G_STATE), Ok(ContainsKeyAnswer::Yes));
    assert_eq!(dict.contains_key(COLORSPACE), Ok(ContainsKeyAnswer::Yes));

    let Ok(found) = dict.get::<Dict<'_>>(EXT_G_STATE);
    let ext_g_state = found.required()?;
    let (Ok(gs2), Ok(gs3)) = (ObjRef::new(14, 0), ObjRef::new(15, 0));

    assert_eq!(ext_g_state.get_ref("GS2"), Ok(Some(gs2)));
    assert_eq!(ext_g_state.get_ref("GS3"), Ok(Some(gs3)));

    Ok(())
  }
}
