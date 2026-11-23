//! The [feature name](https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6feat.html) table.

include!("../../generated/generated_feat.rs");

impl Feat<'_> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Feat::find` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn find(&self, feature: u16) -> Option<FeatureName> {
    let Ok(names) = self.names();

    match names.binary_search_by(|name| {
      let Ok(name_feature) = name.feature();
      name_feature.cmp(&feature)
    }) {
      Ok(position) => names.get(position).copied(),
      Err(_insertion_position) => None,
    }
  }
}

impl FeatureName {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `FeatureName::is_exclusive` method returns `bool` directly in the existing parser interface"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(
      explicit007_no_bool_return,
      reason = "the published `FeatureName::is_exclusive` method retains the Boolean result used by existing predicate callers and implementations"
    )
  )]
  pub fn is_exclusive(&self) -> bool {
    let Ok(feature_flags) = self.feature_flags();

    feature_flags & 0x8000 != 0
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `FeatureName::default_setting_index` method returns `u16` directly in the existing parser interface"
    )
  )]
  pub fn default_setting_index(&self) -> u16 {
    let Ok(feature_flags) = self.feature_flags();

    match ::core::primitive::bool::then_some(feature_flags & 0x4000 != 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(own_feature_flags) = self.feature_flags();

        own_feature_flags & 0xFF
      }
      ::core::option::Option::None => 0,
    }
  }
}

#[cfg(test)]
mod tests {
  use font_test_data::bebuffer::BeBuffer;

  use super::*;

  #[test]
  fn feat_example() -> ::core::result::Result<(), crate::read::ReadError> {
    let ::core::result::Result::Ok(feat_data) = build_feat_example();
    let feat = Feat::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*feat_data))?;
    let Ok(names) = feat.names();
    let Ok(name_id) = NameId::new(260);
    #[rustfmt::skip]
        let expected_name_fields = [

            (0, 1, 0, name_id, false, 0),
            (1, 1, 0, { let Ok(value) = NameId::new(256); value }, false, 0),
            (3, 3, 0x8000, { let Ok(value) = NameId::new(262); value }, true, 0),
            (6, 2, 0xC001, { let Ok(value) = NameId::new(258); value }, true, 1),
        ];
    let name_fields = names
      .iter()
      .map(|name| {
        let Ok(feature) = name.feature();
        let Ok(n_settings) = name.n_settings();
        let Ok(feature_flags) = name.feature_flags();
        let Ok(name_index) = name.name_index();
        (
          feature,
          n_settings,
          feature_flags,
          name_index,
          name.is_exclusive(),
          name.default_setting_index(),
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(name_fields, expected_name_fields);
    #[rustfmt::skip]
        let expected_setting_names: [&[(u16, NameId)]; 4] = [
            &[(0, ({ let Ok(explicit_value) = NameId::new(261); explicit_value }))],
            &[(2, ({ let Ok(explicit_value) = NameId::new(257); explicit_value }))],
            &[(0, ({ let Ok(explicit_value) = NameId::new(268); explicit_value })), (3, ({ let Ok(explicit_value) = NameId::new(264); explicit_value })), (4, ({ let Ok(explicit_value) = NameId::new(265); explicit_value }))],
            &[(0, ({ let Ok(explicit_value) = NameId::new(259); explicit_value })), (1, ({ let Ok(explicit_value) = NameId::new(260); explicit_value }))],
        ];
    let Ok(offset_data) = feat.offset_data();

    for (name, expected) in names.iter().zip(expected_setting_names) {
      let settings = name.setting_table(offset_data)?;
      let Ok(records) = settings.settings();
      let actual = records.iter().map(|setting| {
        let Ok(value) = setting.setting();
        let Ok(name_idx) = setting.name_index();
        (value, name_idx)
      });
      assert!(actual.eq(expected.iter().copied()));
    }

    ::core::result::Result::Ok(())
  }

  #[test]
  fn feat_find() -> ::core::result::Result<(), crate::test_support::Failure> {
    let ::core::result::Result::Ok(feat_data) = build_feat_example();
    let feat = crate::test_support::fixture_value(
      Feat::read(<crate::font_data::FontData<'_> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(&*feat_data)),
      "fixture value",
    )?;

    let valid_features = [0, 1, 3, 6];

    for i in 0..10 {
      let is_valid = valid_features.contains(&i);
      let name = feat.find(i);

      match ::core::primitive::bool::then_some(is_valid, ()) {
        ::core::option::Option::Some(()) => {
          let fixture_value = crate::test_support::fixture_value(name, "fixture value")?;
          assert_eq!(fixture_value.feature(), Ok(i));
        }
        ::core::option::Option::None => {
          assert!(name.is_none());
        }
      }
    }

    Ok(())
  }

  fn build_feat_example() -> ::core::result::Result<BeBuffer, ::console_core_never::Never> {
    let mut buf = font_test_data::bebuffer::BeBuffer::default();

    {
      buf.extend([0x00010000u32]);
      buf.extend([4u16, 0, 0, 0]);
    };

    {
      buf.extend([0u16, 1]);
      buf.extend([60u32]);
      buf.extend([0u16, 260]);
    };

    {
      buf.extend([1u16, 1]);
      buf.extend([64u32]);
      buf.extend([0u16, 256]);
    };

    {
      buf.extend([3u16, 3]);
      buf.extend([68u32]);
      buf.extend([0x8000u16, 262]);
    };

    {
      buf.extend([6u16, 2]);
      buf.extend([80u32]);
      buf.extend([0xC001u16, 258]);
    };

    buf.extend([0u16, 261, 2, 257, 0, 268, 3, 264, 4, 265, 0, 259, 1, 260]);
    ::core::result::Result::Ok(buf)
  }
}
