use std::collections::HashSet;
use std::fmt::Write;
use std::path::{Component, Path, PathBuf};

use console_core_audio_metadata::{
  FileParseError, ItemKey, MimeType, Picture, PictureType, Tag, TagType, TaggedFile,
};
use console_core_never::Never;

type Failure = Box<dyn std::error::Error>;

const SONGS: [&str; 131] = [
  "UPPER.MP3",
  "alaw.wav",
  "ape-and-v1.mp3",
  "ape-v1.mp3",
  "ape.aac",
  "ape.mp3",
  "bad-size.mp3",
  "bad-version.mp3",
  "both.mp3",
  "cbr-no-xing.mp3",
  "compressed23.mp3",
  "compressed24.mp3",
  "eight-bit.wav",
  "empty.flac",
  "empty.mp3",
  "empty.wav",
  "experimental-flag.mp3",
  "extended23.mp3",
  "extended24.mp3",
  "float.wav",
  "footer24.mp3",
  "frame-unsynchronised.mp3",
  "garbage.aac",
  "garbage.flac",
  "garbage.m4a",
  "garbage.mp3",
  "garbage.ogg",
  "garbage.opus",
  "garbage.wav",
  "genre.m4a",
  "handmade-info.wav",
  "id3-lower.wav",
  "id3-upper.wav",
  "id3.flac",
  "id3v1-unknown-genre.mp3",
  "id3v1.mp3",
  "id3v22.mp3",
  "id3v23.mp3",
  "id3v24.mp3",
  "info.wav",
  "junk.mp3",
  "missing.mp3",
  "odd-comments.ogg",
  "odd22-com-desc.mp3",
  "odd22-pic-unknown-format.mp3",
  "odd22-tye-tda-tim.mp3",
  "odd22-unknown-id.mp3",
  "odd23-compressed-flag-bad.mp3",
  "odd23-encrypted.mp3",
  "odd23-grouping.mp3",
  "odd23-tcon-number.mp3",
  "odd23-tcon-refinement.mp3",
  "odd23-tcon-twice.mp3",
  "odd23-tdat-no-tyer.mp3",
  "odd23-tory.mp3",
  "odd23-trda.mp3",
  "odd23-truncated-frame.mp3",
  "odd23-tsiz.mp3",
  "odd23-txxx-desc.mp3",
  "odd23-tyer-bad.mp3",
  "odd23-tyer-only.mp3",
  "odd23-unknown-id.mp3",
  "odd23-v24-only-id.mp3",
  "odd24-apic-no-mime.mp3",
  "odd24-apic-truncated.mp3",
  "odd24-apic-type-out-of-range.mp3",
  "odd24-apic-unknown-mime.mp3",
  "odd24-bad-encoding.mp3",
  "odd24-duplicate-title.mp3",
  "odd24-empty-comment.mp3",
  "odd24-empty-frame.mp3",
  "odd24-frame-flags-encrypted.mp3",
  "odd24-frame-flags-grouping.mp3",
  "odd24-frame-flags-length.mp3",
  "odd24-frame-flags-status.mp3",
  "odd24-genres.mp3",
  "odd24-latin1-trailing-nul.mp3",
  "odd24-lowercase-id.mp3",
  "odd24-odd-language.mp3",
  "odd24-only-encoding.mp3",
  "odd24-padding-garbage.mp3",
  "odd24-popm.mp3",
  "odd24-short-comment.mp3",
  "odd24-tdor-partial.mp3",
  "odd24-tdrc-not-a-date.mp3",
  "odd24-tdrc-utf16.mp3",
  "odd24-tdrc-year.mp3",
  "odd24-tipl.mp3",
  "odd24-tmcl.mp3",
  "odd24-trck-bad.mp3",
  "odd24-txxx-empty-value.mp3",
  "odd24-ufid.mp3",
  "odd24-unknown-id.mp3",
  "odd24-utf16-big-bom.mp3",
  "odd24-utf16-no-bom.mp3",
  "odd24-utf16-no-terminator.mp3",
  "odd24-utf8-invalid.mp3",
  "odd24-wxxx.mp3",
  "opus-named.ogg",
  "oversized-frame.mp3",
  "picture-in-comment.flac",
  "plain.aac",
  "plain.flac",
  "plain.m4a",
  "plain.ogg",
  "plain.opus",
  "plain.wav",
  "rate11-24bit.wav",
  "rate22.mp3",
  "rate44-no-xing.mp3",
  "rate44.aac",
  "rate44.flac",
  "rate44.m4a",
  "rate44.mp3",
  "rate44.ogg",
  "rate48.opus",
  "song.oga",
  "song.webm",
  "song.wma",
  "tagged.aac",
  "tagged.flac",
  "tags.m4a",
  "tags.ogg",
  "tags.opus",
  "truncated.flac",
  "truncated.m4a",
  "truncated.mp3",
  "unknown-tag-flags.mp3",
  "unsynchronised.mp3",
  "untagged.mp3",
  "v22-compressed.mp3",
];

const SHOWN: [ItemKey; 15] = [
  ItemKey::TrackTitle,
  ItemKey::TrackArtist,
  ItemKey::AlbumArtist,
  ItemKey::AlbumTitle,
  ItemKey::TrackSubtitle,
  ItemKey::RecordingDate,
  ItemKey::Year,
  ItemKey::Genre,
  ItemKey::Comment,
  ItemKey::Description,
  ItemKey::Composer,
  ItemKey::Performer,
  ItemKey::Publisher,
  ItemKey::Label,
  ItemKey::CopyrightMessage,
];

const WRITTEN_INTO: [&str; 4] = ["plain.wav", "info.wav", "id3-lower.wav", "id3-upper.wav"];

fn fixtures() -> Result<PathBuf, Never> {
  Ok(
    Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("tests")
      .join("fixtures"),
  )
}

fn fingerprint(bytes: &[u8]) -> Result<u64, Never> {
  Ok(bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
    (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
  }))
}

fn lines(tag: &Tag) -> Result<Vec<String>, Never> {
  let shown: HashSet<ItemKey> = SHOWN.into_iter().collect();
  let mut said = Vec::new();

  for item in &tag.items {
    let text = match shown.get(&item.key) {
      Some(_included) => &item.text,
      None => continue,
    };

    said.push(format!("  {:?} {text:?}", item.key));
  }

  for picture in &tag.pictures {
    let Ok(print) = fingerprint(&picture.data);

    said.push(format!(
      "  picture {:?} {} {print:016x}",
      picture.kind,
      picture.data.len()
    ));
  }

  Ok(said)
}

fn described(tag: &Tag, into: &mut String) -> Result<(), Failure> {
  let Ok(said) = lines(tag);

  writeln!(into, "tag {:?}", tag.tag_type)?;

  for line in said {
    writeln!(into, "{line}")?;
  }

  Ok(())
}

fn described_in_any_order(tag: &Tag, into: &mut String) -> Result<(), Failure> {
  let Ok(mut said) = lines(tag);

  said.sort();

  writeln!(into, "tag {:?}", tag.tag_type)?;

  for line in said {
    writeln!(into, "{line}")?;
  }

  Ok(())
}

fn opened(path: &Path, into: &mut String) -> Result<Option<TaggedFile>, Failure> {
  let read = std::fs::File::open(path)
    .map_err(FileParseError::Open)
    .and_then(|mut reader| console_core_audio_metadata::read(&mut reader, path));
  let file = match read {
    Ok(file) => file,
    Err(fault) => {
      writeln!(into, "unread: {fault}")?;

      return Ok(None);
    }
  };

  writeln!(into, "duration {}", file.duration.as_nanos())?;

  let Ok(primary) = file.primary_tag();

  match primary {
    Some(primary_2) => writeln!(into, "primary {:?}", primary_2.tag_type)?,
    None => writeln!(into, "primary none")?,
  }

  Ok(Some(file))
}

fn read(path: &Path, into: &mut String) -> Result<(), Failure> {
  let opened = opened(path, into)?;

  for tag in opened.iter().flat_map(|file| &file.tags) {
    described(tag, into)?;
  }

  Ok(())
}

fn read_in_any_order(path: &Path, into: &mut String) -> Result<(), Failure> {
  let opened = opened(path, into)?;

  for tag in opened.iter().flat_map(|file| &file.tags) {
    described_in_any_order(tag, into)?;
  }

  Ok(())
}

fn fixture_path(folder: &Path, name: &str) -> Result<PathBuf, std::io::Error> {
  let mut components = Path::new(name).components();
  let checked = match (components.next(), components.next()) {
    (Some(Component::Normal(component)), None) => component,
    (_, _) => return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput)),
  };

  #[cfg_attr(
    dylint_lib = "explicit086_no_climbing_join",
    expect(
      explicit086_no_climbing_join,
      reason = "the match accepts only one normal path component"
    )
  )]
  let path = folder.join(checked);

  Ok(path)
}

fn every_song() -> Result<String, Failure> {
  let Ok(folder) = fixtures();
  let mut said = String::new();

  for song in SONGS {
    writeln!(said, "== {song}")?;
    let path = fixture_path(&folder, song)?;

    read(&path, &mut said)?;
  }

  Ok(said)
}

fn written_tag() -> Result<Tag, Never> {
  let Ok(mut tag) = Tag::new(TagType::Id3v2);

  for (key, text) in SHOWN.into_iter().zip([
    "Rain",
    "Nujabes",
    "Band",
    "Modal Soul",
    "Sub",
    "2005-11-11",
    "2005",
    "Hip-Hop",
    "Said",
    "Described",
    "Composer",
    "Performer",
    "Publisher",
    "Label",
    "Copyright",
  ]) {
    let Ok(()) = tag.insert_text(key, text.to_string());
  }

  tag.pictures.push(Picture {
    kind: PictureType::CoverBack,
    mime_type: Some(MimeType::Jpeg),
    data: b"the back".to_vec(),
  });
  tag.pictures.push(Picture {
    kind: PictureType::CoverFront,
    mime_type: Some(MimeType::Png),
    data: b"the front".to_vec(),
  });

  Ok(tag)
}

fn every_write() -> Result<String, Failure> {
  let Ok(folder) = fixtures();
  let scratch = console_core_temporary_directories::fresh("audio-metadata-the-songs")?;
  let Ok(tag) = written_tag();
  let mut said = String::new();

  for song in WRITTEN_INTO {
    let at = fixture_path(&scratch, song)?;
    let fixture = fixture_path(&folder, song)?;
    let original = std::fs::read(fixture)?;

    console_core_atomic_writes::whole(&at, &original)?;

    writeln!(said, "== written into {song}")?;

    match console_core_audio_metadata::tagged_wave(&original, &tag) {
      Ok(changed) => console_core_atomic_writes::whole(&at, &changed)?,
      Err(fault) => writeln!(said, "unwritten: {fault}")?,
    }

    let after = std::fs::read(&at)?;

    writeln!(said, "bytes {}", after.len())?;
    read_in_any_order(&at, &mut said)?;
  }

  Ok(said)
}

#[test]
fn every_song_says_what_it_said_before() -> Result<(), Failure> {
  let said = every_song()?;

  assert_eq!(said, include_str!("fixtures/read.txt"));

  Ok(())
}

#[test]
fn a_tag_written_into_a_wave_reads_back_as_it_did_before() -> Result<(), Failure> {
  let said = every_write()?;

  assert_eq!(said, include_str!("fixtures/written.txt"));

  Ok(())
}
