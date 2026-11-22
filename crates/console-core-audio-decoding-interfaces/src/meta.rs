//! What a stream says about itself, as tags and pictures.
//!
//! A reader collects the tags and the pictures it finds into a revision, and a
//! stream that is read in several passes keeps one revision per pass in a log,
//! so a caller can see each as it was read or ask only for the newest. What a
//! standard key means is decided by the readers that produce it; this crate
//! only holds the list they choose from.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::fmt;
use std::num::NonZeroU32;

pub use console_core_size::Size;

use crate::errors::Result;
use crate::io::MediaSourceStream;

#[derive(Copy, Clone, Debug, Default)]
pub struct MetadataOptions {}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StandardVisualKey {
  FileIcon,
  OtherIcon,
  FrontCover,
  BackCover,
  Leaflet,
  Media,
  LeadArtistPerformerSoloist,
  ArtistPerformer,
  Conductor,
  BandOrchestra,
  Composer,
  Lyricist,
  RecordingLocation,
  RecordingSession,
  Performance,
  ScreenCapture,
  Illustration,
  BandArtistLogo,
  PublisherStudioLogo,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StandardTagKey {
  AcoustidFingerprint,
  AcoustidId,
  Album,
  AlbumArtist,
  Arranger,
  Artist,
  Bpm,
  Comment,
  Compilation,
  Composer,
  Conductor,
  ContentGroup,
  Copyright,
  Date,
  Description,
  DiscNumber,
  DiscSubtitle,
  DiscTotal,
  EncodedBy,
  Encoder,
  EncoderSettings,
  EncodingDate,
  Engineer,
  Ensemble,
  Genre,
  IdentAsin,
  IdentBarcode,
  IdentCatalogNumber,
  IdentEanUpn,
  IdentIsrc,
  IdentPn,
  IdentPodcast,
  IdentUpc,
  Label,
  Language,
  License,
  Lyricist,
  Lyrics,
  MediaFormat,
  MixDj,
  MixEngineer,
  Mood,
  MovementName,
  MovementNumber,
  MusicBrainzAlbumArtistId,
  MusicBrainzAlbumId,
  MusicBrainzArtistId,
  MusicBrainzDiscId,
  MusicBrainzGenreId,
  MusicBrainzLabelId,
  MusicBrainzOriginalAlbumId,
  MusicBrainzOriginalArtistId,
  MusicBrainzRecordingId,
  MusicBrainzReleaseGroupId,
  MusicBrainzReleaseStatus,
  MusicBrainzReleaseTrackId,
  MusicBrainzReleaseType,
  MusicBrainzTrackId,
  MusicBrainzWorkId,
  Opus,
  OriginalAlbum,
  OriginalArtist,
  OriginalDate,
  OriginalFile,
  OriginalWriter,
  Owner,
  Part,
  PartTotal,
  Performer,
  Podcast,
  PodcastCategory,
  PodcastDescription,
  PodcastKeywords,
  Producer,
  PurchaseDate,
  Rating,
  ReleaseCountry,
  ReleaseDate,
  Remixer,
  ReplayGainAlbumGain,
  ReplayGainAlbumPeak,
  ReplayGainTrackGain,
  ReplayGainTrackPeak,
  Script,
  SortAlbum,
  SortAlbumArtist,
  SortArtist,
  SortComposer,
  SortTrackTitle,
  TaggingDate,
  TrackNumber,
  TrackSubtitle,
  TrackTitle,
  TrackTotal,
  TvEpisode,
  TvEpisodeTitle,
  TvNetwork,
  TvSeason,
  TvShowTitle,
  Url,
  UrlArtist,
  UrlCopyright,
  UrlInternetRadio,
  UrlLabel,
  UrlOfficial,
  UrlPayment,
  UrlPodcast,
  UrlPurchase,
  UrlSource,
  Version,
  Writer,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BooleanValue {
  False,
  True,
}

impl From<bool> for BooleanValue {
  fn from(value: bool) -> Self {
    match bool::then_some(value, ()) {
      Some(()) => Self::True,
      None => Self::False,
    }
  }
}

impl fmt::Display for BooleanValue {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::False => f.pad("false"),
      Self::True => f.pad("true"),
    }
  }
}

impl fmt::Debug for BooleanValue {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Display::fmt(self, f)
  }
}

#[derive(Clone, Debug)]
pub enum Value {
  Binary(Box<[u8]>),
  Boolean(BooleanValue),
  Flag,
  Float(f64),
  SignedInt(i64),
  String(String),
  UnsignedInt(u64),
}

impl From<&[u8]> for Value {
  fn from(v: &[u8]) -> Self {
    Value::Binary(Box::from(v))
  }
}

impl From<bool> for Value {
  fn from(v: bool) -> Self {
    Value::Boolean(BooleanValue::from(v))
  }
}

impl From<f32> for Value {
  fn from(v: f32) -> Self {
    Value::Float(f64::from(v))
  }
}

impl From<f64> for Value {
  fn from(v: f64) -> Self {
    Value::Float(v)
  }
}

impl From<i8> for Value {
  fn from(v: i8) -> Self {
    Value::SignedInt(i64::from(v))
  }
}

impl From<i16> for Value {
  fn from(v: i16) -> Self {
    Value::SignedInt(i64::from(v))
  }
}

impl From<i32> for Value {
  fn from(v: i32) -> Self {
    Value::SignedInt(i64::from(v))
  }
}

impl From<i64> for Value {
  fn from(v: i64) -> Self {
    Value::SignedInt(v)
  }
}

impl From<u8> for Value {
  fn from(v: u8) -> Self {
    Value::UnsignedInt(u64::from(v))
  }
}

impl From<u16> for Value {
  fn from(v: u16) -> Self {
    Value::UnsignedInt(u64::from(v))
  }
}

impl From<u32> for Value {
  fn from(v: u32) -> Self {
    Value::UnsignedInt(u64::from(v))
  }
}

impl From<u64> for Value {
  fn from(v: u64) -> Self {
    Value::UnsignedInt(v)
  }
}

impl From<&str> for Value {
  fn from(v: &str) -> Self {
    Value::String(String::from(v))
  }
}

impl From<String> for Value {
  fn from(v: String) -> Self {
    Value::String(v)
  }
}

impl From<Cow<'_, str>> for Value {
  fn from(v: Cow<'_, str>) -> Self {
    Value::String(String::from(v))
  }
}

impl fmt::Display for Value {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Value::Binary(buf) => {
        for byte in buf {
          write!(f, "\\0x{byte:02x}")?;
        }

        Ok(())
      }
      Value::Boolean(boolean) => fmt::Display::fmt(boolean, f),
      Value::Flag => write!(f, "<flag>"),
      Value::Float(float) => fmt::Display::fmt(float, f),
      Value::SignedInt(int) => fmt::Display::fmt(int, f),
      Value::String(string) => fmt::Display::fmt(string, f),
      Value::UnsignedInt(uint) => fmt::Display::fmt(uint, f),
    }
  }
}

#[derive(Clone, Debug)]
pub struct Tag {
  pub std_key: Option<StandardTagKey>,
  pub key: String,
  pub value: Value,
}

impl Tag {
  #[must_use]
  pub fn new(
    std_key: Option<StandardTagKey>,
    key: &str,
    value: Value,
  ) -> std::result::Result<Tag, console_core_never::Never> {
    Ok(Tag {
      std_key,
      key: key.to_string(),
      value,
    })
  }
}

impl fmt::Display for Tag {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self.std_key {
      Some(std_key) => {
        write!(
          f,
          "{{ std_key={:?}, key=\"{}\", value={} }}",
          std_key, self.key, self.value
        )
      }
      None => write!(f, "{{ key=\"{}\", value={} }}", self.key, self.value),
    }
  }
}

#[derive(Copy, Clone, Debug)]
pub enum ColorMode {
  Discrete,
  Indexed(NonZeroU32),
}

#[derive(Clone, Debug)]
pub struct Visual {
  pub media_type: String,
  pub dimensions: Option<Size<u32>>,
  pub bits_per_pixel: Option<NonZeroU32>,
  pub color_mode: Option<ColorMode>,
  pub usage: Option<StandardVisualKey>,
  pub tags: Vec<Tag>,
  pub data: Box<[u8]>,
}

#[derive(Clone, Debug)]
pub struct VendorData {
  pub ident: String,
  pub data: Box<[u8]>,
}

#[derive(Clone, Debug, Default)]
pub struct MetadataRevision {
  tags: Vec<Tag>,
  visuals: Vec<Visual>,
  vendor_data: Vec<VendorData>,
}

impl MetadataRevision {
  #[must_use]
  pub fn tags(&self) -> std::result::Result<&[Tag], console_core_never::Never> {
    Ok(&self.tags)
  }

  #[must_use]
  pub fn visuals(&self) -> std::result::Result<&[Visual], console_core_never::Never> {
    Ok(&self.visuals)
  }

  #[must_use]
  pub fn vendor_data(&self) -> std::result::Result<&[VendorData], console_core_never::Never> {
    Ok(&self.vendor_data)
  }
}

#[derive(Clone, Debug, Default)]
pub struct MetadataBuilder {
  metadata: MetadataRevision,
}

impl MetadataBuilder {
  #[must_use]
  pub fn new() -> std::result::Result<Self, console_core_never::Never> {
    Ok(MetadataBuilder {
      metadata: MetadataRevision::default(),
    })
  }

  pub fn add_tag(
    &mut self,
    tag: Tag,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.metadata.tags.push(tag);
    Ok(self)
  }

  pub fn add_visual(
    &mut self,
    visual: Visual,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.metadata.visuals.push(visual);
    Ok(self)
  }

  pub fn add_vendor_data(
    &mut self,
    vendor_data: VendorData,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.metadata.vendor_data.push(vendor_data);
    Ok(self)
  }

  #[must_use]
  pub fn metadata(self) -> std::result::Result<MetadataRevision, console_core_never::Never> {
    Ok(self.metadata)
  }
}

#[derive(Debug)]
pub struct Metadata<'a> {
  revisions: &'a mut VecDeque<MetadataRevision>,
}

impl Metadata<'_> {
  #[must_use]
  pub fn current(
    &self,
  ) -> std::result::Result<Option<&MetadataRevision>, console_core_never::Never> {
    Ok(self.revisions.front())
  }

  pub fn pop(
    &mut self,
  ) -> std::result::Result<Option<MetadataRevision>, console_core_never::Never> {
    Ok(match self.revisions.len() {
      0 | 1 => None,
      _has_next => self.revisions.pop_front(),
    })
  }
}

#[derive(Clone, Debug, Default)]
pub struct MetadataLog {
  revisions: VecDeque<MetadataRevision>,
}

impl MetadataLog {
  pub fn metadata(&mut self) -> std::result::Result<Metadata<'_>, console_core_never::Never> {
    Ok(Metadata {
      revisions: &mut self.revisions,
    })
  }

  pub fn push(
    &mut self,
    rev: MetadataRevision,
  ) -> std::result::Result<(), console_core_never::Never> {
    self.revisions.push_back(rev);

    Ok(())
  }
}

pub trait MetadataReader: Send + Sync {
  fn new(options: &MetadataOptions) -> std::result::Result<Self, console_core_never::Never>
  where
    Self: Sized;

  fn read_all(&mut self, reader: &mut MediaSourceStream) -> Result<MetadataRevision>;
}
