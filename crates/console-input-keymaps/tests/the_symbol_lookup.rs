use console_input_keymaps::{
  Context, Diagnostic, GroupIndex, KeyRepeat, Keycode, Keysym, ModifierMask,
};

const KEYMAP: &str = r#"
xkb_keymap {
    xkb_keycodes "test" {
        minimum = 8; maximum = 255;
        <AC01> = 38;
        <AC02> = 39;
    };
    xkb_types "test" {
        type "TWO_LEVEL" {
            modifiers = Shift;
            map[Shift] = Level2;
            level_name[Level1] = "Base";
            level_name[Level2] = "Shift";
        };
    };
    xkb_compatibility "test" {};
    xkb_symbols "test" {
        key <AC01> { type[Group1] = "TWO_LEVEL", symbols[Group1] = [ a, A ], repeat = yes };
        key <AC02> { type[Group1] = "TWO_LEVEL", symbols[Group1] = [ Greek_alpha, Greek_ALPHA ], repeat = no };
    };
};
"#;

#[test]
fn compiled_keymaps_return_the_shared_symbol_type_under_modifiers()
-> Result<(), Box<dyn std::error::Error>> {
  let context = Context::default();
  let mut diagnostics = Vec::<Diagnostic>::new();
  let keymap = context.keymap_from_bytes(&mut diagnostics, None, KEYMAP)?;
  let Ok(builder) = keymap.to_builder();
  let Ok(table) = builder.build_lookup_table();

  for (evdev, base, shifted, repeat) in [
    (30, Keysym::a, Keysym::A, KeyRepeat::Repeats),
    (
      31,
      Keysym::Greek_alpha,
      Keysym::Greek_ALPHA,
      KeyRepeat::DoesNotRepeat,
    ),
  ] {
    let Ok(key) = Keycode::from_evdev(evdev);
    assert_eq!(table.repeat(key), Ok(repeat));

    for (modifiers, expected) in [(ModifierMask(0), base), (ModifierMask(1), shifted)] {
      let Ok(found) = table.lookup(GroupIndex(0), modifiers, key);
      let symbols: Vec<console_input_key_symbols::Keysym> = found.collect();
      assert_eq!(symbols, vec![expected]);
    }
  }

  Ok(())
}
