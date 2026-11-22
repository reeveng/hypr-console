use console_core_audio_decoding_interfaces::formats::util::{
  SeekIndex, SeekPoint, SeekSearchResult,
};

#[test]
fn a_search_answers_with_the_points_around_the_timestamp() {
  let Ok(mut index) = SeekIndex::new();

  assert_eq!(index.search(5), Ok(SeekSearchResult::Stream));

  let Ok(()) = index.insert(479232, 706812, 1152);
  let Ok(()) = index.insert(959616, 1421536, 1152);
  let Ok(()) = index.insert(1919232, 2833241, 1152);
  let Ok(()) = index.insert(2399616, 3546987, 1152);
  let Ok(()) = index.insert(2880000, 4259455, 1152);

  let Ok(value_3) = index.search(0);

  assert_eq!(
    value_3,
    SeekSearchResult::Upper(SeekPoint {
      frame_ts: 479232,
      byte_offset: 706812,
      n_frames: 1152
    })
  );
  let Ok(value_4) = index.search(3000000);

  assert_eq!(
    value_4,
    SeekSearchResult::Lower(SeekPoint {
      frame_ts: 2880000,
      byte_offset: 4259455,
      n_frames: 1152
    })
  );
  let Ok(value_5) = index.search(959616);

  assert_eq!(
    value_5,
    SeekSearchResult::Range(
      SeekPoint {
        frame_ts: 959616,
        byte_offset: 1421536,
        n_frames: 1152
      },
      SeekPoint {
        frame_ts: 1919232,
        byte_offset: 2833241,
        n_frames: 1152
      }
    )
  );

  let Ok(()) = index.insert(1440000, 2132419, 1152);

  let Ok(value_6) = index.search(1000000);

  assert_eq!(
    value_6,
    SeekSearchResult::Range(
      SeekPoint {
        frame_ts: 959616,
        byte_offset: 1421536,
        n_frames: 1152
      },
      SeekPoint {
        frame_ts: 1440000,
        byte_offset: 2132419,
        n_frames: 1152
      }
    )
  );

  let Ok(()) = index.insert(3359232, 0, 0);

  let Ok(value_7) = index.search(3359232);

  assert_eq!(
    value_7,
    SeekSearchResult::Lower(SeekPoint {
      frame_ts: 2880000,
      byte_offset: 4259455,
      n_frames: 1152
    })
  );
}
