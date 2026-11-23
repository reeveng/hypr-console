//! A module used to test codegen.
//!
//! This imports a single codegen output; while modifying the codegen crate,
//! this file can be regenerated to check that changes compile, without needing
//! to rebuild everything.
//!
//! To rebuild this input and test it, run:
//!
//! $ cargo run --bin=codegen resources/test_plan.toml && cargo test

pub mod records {
  include!("../generated/generated_test_records.rs");
}

pub mod formats {
  include!("../generated/generated_test_formats.rs");
}

pub mod read_args {
  include!("../generated/generated_test_read_args.rs");
}

pub mod offsets_arrays {

  include!("../generated/generated_test_offsets_arrays.rs");

  pub struct VarSizeDummy<'a> {
    pub bytes: &'a [u8],
  }

  impl VarSize for VarSizeDummy<'_> {
    type Size = u16;
  }

  impl<'a> FontRead<'a> for VarSizeDummy<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
      let count: u16 = data.read_at(0)?;
      let bytes = ({
        let Ok(explicit_value) = data.as_bytes();
        explicit_value
      })
      .get({
        let Ok(explicit_index) = console_core_number_conversion::indices(
          2..::core::primitive::u64::wrapping_add(2, {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(count);
            migrated_value
          }),
        );
        explicit_index
      })
      .ok_or(ReadError::OutOfBounds)?;
      Ok(Self { bytes })
    }
  }

  #[test]
  fn array_offsets() -> ::core::result::Result<(), crate::read::ReadError> {
    let builder = font_test_data::be_buffer! { (MajorMinor::VERSION_1_0), (12_u16), (0u16), (2u16), (12u16), (..([0xdead_u16, 0xbeef])) };

    let table =
      KindsOfOffsets::read(<crate::font_data::FontData<'_> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(&*builder))?;
    assert_eq!(
      match table.nonnullable() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      }
      .value(),
      Ok(0xdead)
    );

    let array = table.array()?;
    assert_eq!(array, &[0xdead, 0xbeef]);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn var_len_array_empty() -> ::core::result::Result<(), crate::read::ReadError> {
    let builder = font_test_data::be_buffer! { (0u16), (0xdeadbeef_u32) };

    let table = VarLenHaver::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*builder))?;
    assert_eq!(table.other_field(), Ok(0xdeadbeef));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn var_len_array_some() -> ::core::result::Result<(), crate::test_support::Failure> {
    let builder = font_test_data::be_buffer! { (3u16), (0u16), (2u16), (..([1u8, 1])), (5u16), (..([7u8, 7, 7, 7, 7])), (0xdeadbeef_u32) };

    let table = crate::test_support::fixture_value(
      VarLenHaver::read(<crate::font_data::FontData<'_> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(&*builder)),
      "fixture value",
    )?;
    let Ok(var_len) = table.var_len();
    let Ok(value_1) = var_len.iter();

    let kids = value_1.collect::<Result<Vec<_>, _>>()?;
    let (first, second, third) = match kids.as_slice() {
      [first, second, third, ..] => (first, second, third),
      _short_array => {
        return Err(crate::test_support::Failure::from(
          "expected three variable-length records",
        ));
      }
    };
    assert!(first.bytes.is_empty());
    assert_eq!(second.bytes, &[1, 1]);
    assert_eq!(third.bytes, &[7, 7, 7, 7, 7]);

    assert_eq!(table.other_field(), Ok(0xdeadbeef));
    Ok(())
  }

  #[test]
  #[cfg(feature = "experimental_traverse")]
  fn array_offsets_traverse() -> ::core::result::Result<(), crate::test_support::Failure> {
    let mut builder = font_test_data::be_buffer! { (MajorMinor::VERSION_1_1), (22_u16), (0u16), (2u16), (26u16), (30u16), (0u16), (42u16), (0u32) };

    let Ok(data_start) = builder.len();
    assert_eq!(data_start, 22);

    {
      builder.extend([0xf00u16, 0xba4]);
      builder.extend([69u16, 70]);
      builder.extend([3u16]);
      builder.extend([9u32]);
      builder.extend([5u16]);
      builder.extend([0xdead_beefu32]);
      builder.extend([0xb01du16, 0xface]);
    };

    let table =
      KindsOfOffsets::read(<crate::font_data::FontData<'_> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(&*builder))?;

    let _ = format!("{table:?}");
    let fixture_value = crate::test_support::fixture_value(
      match table.versioned_nonnullable() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      },
      "fixture value",
    )?;
    assert_eq!(fixture_value.value(), Ok(0xb01d));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn versioned_array_bad_data() -> ::core::result::Result<(), crate::read::ReadError> {
    let buf = font_test_data::be_buffer! { (1u16), (1u16), (2u16), (3u16), (4u32) };
    let table = KindsOfArrays::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;
    assert!(matches!(table.versioned_scalars(), Ok(None)));
    ::core::result::Result::Ok(())
  }
}

pub mod flags {
  include!("../generated/generated_test_flags.rs");

  #[test]
  fn basics() {
    let Ok(all) = ValueFormat::all();
    let Ok(none) = ValueFormat::empty();
    assert_eq!(
      all.contains(ValueFormat::X_PLACEMENT),
      Ok(Presence::Present)
    );
    assert_eq!(
      all.contains(ValueFormat::Y_PLACEMENT),
      Ok(Presence::Present)
    );
    assert_eq!(
      none.contains(ValueFormat::X_PLACEMENT),
      Ok(Presence::Absent)
    );
    assert_eq!(
      none.contains(ValueFormat::Y_PLACEMENT),
      Ok(Presence::Absent)
    );
    assert_eq!(none, ValueFormat::default());
  }

  #[test]
  fn formatting() {
    let Ok(all) = ValueFormat::all();
    assert_eq!(format!("{all:?}"), "X_PLACEMENT | Y_PLACEMENT");
    let Ok(none) = ValueFormat::empty();
    assert_eq!(format!("{none:?}"), "(empty)");
    let xplace = ValueFormat::X_PLACEMENT;
    assert_eq!(format!("{xplace:?}"), "X_PLACEMENT");
  }

  #[test]
  fn impl_traits() {
    fn impl_check<T: Copy + std::hash::Hash + Eq + Ord>()
    -> core::result::Result<(), console_core_never::Never> {
      {};

      Ok(())
    }

    let Ok(()) = impl_check::<ValueFormat>();
  }
}

pub mod enums {
  include!("../generated/generated_test_enum.rs");
}

pub mod count_all {
  use crate::FontData;

  include!("../generated/generated_test_count_all.rs");

  #[test]
  fn element_size_greater_than_one_with_padding()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let bytes = [0u8; 13];

    let Ok(byte_length) = console_core_number_conversion::fixed_index(bytes.len());
    let remainder_len = byte_length.wrapping_sub(2);
    let Ok(data) = FontData::new(&bytes);

    assert!(remainder_len.wrapping_rem(2) != 0);
    let count16 = CountAll16::read(data)?;
    let Ok(remainder) = count16.remainder();
    let Ok(count16_length) = console_core_number_conversion::fixed_index(remainder.len());
    assert_eq!(count16_length, remainder_len.wrapping_div(2));

    assert!(remainder_len.wrapping_rem(4) != 0);
    let count32 = CountAll32::read(data)?;
    let Ok(count32_remainder) = count32.remainder();
    let Ok(count32_length) = console_core_number_conversion::fixed_index(count32_remainder.len());
    assert_eq!(count32_length, remainder_len.wrapping_div(4));
    ::core::result::Result::Ok(())
  }
}

pub mod conditions {
  #[cfg(test)]
  use font_test_data::bebuffer::BeBuffer;
  use font_types::MajorMinor;

  include!("../generated/generated_test_conditions.rs");

  #[test]
  fn majorminor_1() -> ::core::result::Result<(), crate::read::ReadError> {
    let bytes = font_test_data::be_buffer! { (MajorMinor::VERSION_1_0), (0u16) };
    let table =
      MajorMinorVersion::read(<crate::font_data::FontData<'_> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(&*bytes))?;
    assert_eq!(table.always_present(), Ok(0));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn majorminor_1_1() -> ::core::result::Result<(), crate::read::ReadError> {
    let bytes = font_test_data::be_buffer! { (MajorMinor::VERSION_1_1), (0u16) };
    let too_small =
      MajorMinorVersion::read(<crate::font_data::FontData<'_> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(&*bytes))?;

    assert!(matches!(too_small.if_11(), Ok(None)));

    let bytes_2 = font_test_data::be_buffer! { (MajorMinor::VERSION_1_1), (0u16), (1u16) };
    let table =
      MajorMinorVersion::read(<crate::font_data::FontData<'_> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(&*bytes_2))?;
    assert_eq!(table.if_11(), Ok(Some(1)));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn major_minor_2() -> ::core::result::Result<(), crate::read::ReadError> {
    let bytes = font_test_data::be_buffer! { (MajorMinor::VERSION_2_0), (0u16) };
    let too_small =
      MajorMinorVersion::read(<crate::font_data::FontData<'_> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(&*bytes))?;
    assert!(matches!(too_small.if_11(), Ok(None)));
    assert!(matches!(too_small.if_20(), Ok(None)));

    let bytes_2 = font_test_data::be_buffer! { (MajorMinor::VERSION_2_0), (0u16), (2u32) };
    let table =
      MajorMinorVersion::read(<crate::font_data::FontData<'_> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(&*bytes_2))?;
    assert_eq!(table.if_11(), Ok(None));
    assert_eq!(table.if_20(), Ok(Some(2)));
    ::core::result::Result::Ok(())
  }

  #[cfg(test)]
  #[must_use]
  fn make_flag_data(
    flags: GotFlags,
  ) -> core::result::Result<BeBuffer, console_core_never::Never> {
    let mut buf = font_test_data::be_buffer! { (42u16), (flags) };
    let Ok(foo) = flags.contains(GotFlags::FOO);
    let Ok(bar) = flags.contains(GotFlags::BAR);
    let Ok(baz) = flags.contains(GotFlags::BAZ);

    match foo {
      Presence::Present => {
        buf.extend([0xf00_u16]);
      }
      Presence::Absent => {}
    }

    match bar {
      Presence::Present => {
        buf.extend([0xba4_u16]);
      }
      Presence::Absent => {}
    }

    match (foo, baz) {
      (Presence::Absent, Presence::Absent) => {}
      (Presence::Present, Presence::Present)
      | (Presence::Present, Presence::Absent)
      | (Presence::Absent, Presence::Present) => {
        buf.extend([0xba2_u16]);
      }
    }

    Ok(buf)
  }

  #[test]
  fn flags_none() -> ::core::result::Result<(), crate::read::ReadError> {
    let Ok(empty) = GotFlags::empty();
    let Ok(data) = make_flag_data(empty);
    let table = FlagDay::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*data))?;
    assert!(matches!(table.foo(), Ok(None)));
    assert!(matches!(table.bar(), Ok(None)));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn flags_foo() -> ::core::result::Result<(), crate::read::ReadError> {
    let Ok(data) = make_flag_data(GotFlags::FOO);
    let table = FlagDay::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*data))?;
    assert_eq!(table.foo(), Ok(Some(0xf00)));
    assert!(matches!(table.bar(), Ok(None)));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn flags_bar() -> ::core::result::Result<(), crate::read::ReadError> {
    let Ok(data) = make_flag_data(GotFlags::BAR);
    let table = FlagDay::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*data))?;
    assert!(matches!(table.foo(), Ok(None)));
    assert_eq!(table.bar(), Ok(Some(0xba4)));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn flags_foobar() -> ::core::result::Result<(), crate::read::ReadError> {
    let Ok(data) = make_flag_data(GotFlags::BAR | GotFlags::FOO);
    let table = FlagDay::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*data))?;
    assert_eq!(table.foo(), Ok(Some(0xf00)));
    assert_eq!(table.bar(), Ok(Some(0xba4)));
    ::core::result::Result::Ok(())
  }
}

pub mod generic_group {
  include!("../generated/generated_test_generic_group.rs");

  #[cfg(test)]
  use font_test_data::bebuffer::BeBuffer;

  #[cfg(test)]
  #[must_use]
  fn make_lookup_with_format1(
    lookup_type: u16,
  ) -> core::result::Result<BeBuffer, console_core_never::Never> {
    Ok(font_test_data::be_buffer! { (lookup_type), (1u16), (6u16), (1u16), (42u16) })
  }

  #[test]
  fn parse_lookup_group_type_one() -> ::core::result::Result<(), crate::read::ReadError> {
    let Ok(buf) = make_lookup_with_format1(1);
    let group = MyLookupGroup::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;
    assert!(matches!(group, MyLookupGroup::TypeOne(_)));
    let Ok(lookup) = group.of_unit_type();
    assert_eq!(lookup.lookup_type(), Ok(1));
    assert_eq!(lookup.sub_table_count(), Ok(1));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_lookup_group_type_two() -> ::core::result::Result<(), crate::read::ReadError> {
    let Ok(buf) = make_lookup_with_format1(2);
    let group = MyLookupGroup::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;
    assert!(matches!(group, MyLookupGroup::TypeTwo(_)));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_lookup_group_invalid_type() {
    let Ok(buf) = make_lookup_with_format1(99);
    let result = MyLookupGroup::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf));

    assert!(matches!(result, Err(ReadError::InvalidFormat(99))))
  }

  #[test]
  fn parse_subtable_format_dispatch() -> ::core::result::Result<(), crate::test_support::Failure>
  {
    let buf = font_test_data::be_buffer! { (1u16), (42u16) };
    let sub = crate::test_support::fixture_value(
      MySubtable::read(<crate::font_data::FontData<'_> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(&*buf)),
      "fixture value",
    )?;

    match sub {
      MySubtable::Format1(f1) => assert_eq!(f1.value(), Ok(42)),
      MySubtable::Format2(_) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "expected subtable format 1"
        )));
      }
    }

    let buf_2 = font_test_data::be_buffer! { (2u16), (2u16), (..([10u16, 20])) };
    let sub_2 = crate::test_support::fixture_value(
      MySubtable::read(<crate::font_data::FontData<'_> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(&*buf_2)),
      "fixture value",
    )?;

    match sub_2 {
      MySubtable::Format2(f2) => assert_eq!(f2.count(), Ok(2)),
      MySubtable::Format1(_) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "expected subtable format 2"
        )));
      }
    };

    Ok(())
  }
}
