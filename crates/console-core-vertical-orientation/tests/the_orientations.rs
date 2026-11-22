use console_core_vertical_orientation::{Orientation, orientation};

#[test]
fn latin_turns_with_the_line_and_han_stands_upright() {
  assert_eq!(orientation('A'), Ok(Orientation::Rotated));
  assert_eq!(orientation('本'), Ok(Orientation::Upright));
  assert_eq!(orientation('한'), Ok(Orientation::Upright));
}

#[test]
fn a_glyph_of_its_own_says_which_way_it_falls_back() {
  assert_eq!(orientation('。'), Ok(Orientation::TransformedOrUpright));
  assert_eq!(orientation('「'), Ok(Orientation::TransformedOrRotated));
  assert_eq!(orientation('ー'), Ok(Orientation::TransformedOrRotated));
}

#[test]
fn the_ends_of_a_range_are_in_it_and_the_next_character_is_not() {
  assert_eq!(orientation('\u{00A6}'), Ok(Orientation::Rotated));
  assert_eq!(orientation('\u{00A7}'), Ok(Orientation::Upright));
  assert_eq!(orientation('\u{00A8}'), Ok(Orientation::Rotated));
  assert_eq!(orientation('\u{10FFFD}'), Ok(Orientation::Upright));
  assert_eq!(orientation('\u{10FFFE}'), Ok(Orientation::Rotated));
  assert_eq!(orientation('\u{0}'), Ok(Orientation::Rotated));
}
