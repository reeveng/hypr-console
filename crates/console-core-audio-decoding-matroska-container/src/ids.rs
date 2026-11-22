//! The element ids this reader asks for and no others.
//!
//! The upstream table named every element the format defines, with its type, in a
//! map built on first use. A reader that skips what it does not ask for needs
//! only the ids it matches on, which are plain numbers a `match` can take, so
//! the table, the enum beside it and the lazy static that held them are gone.

pub(crate) const EBML: u32 = 0x1A45_DFA3;
pub(crate) const DOC_TYPE: u32 = 0x4282;

pub(crate) const SEGMENT: u32 = 0x1853_8067;
pub(crate) const SEEK_HEAD: u32 = 0x114D_9B74;
pub(crate) const SEEK: u32 = 0x4DBB;
pub(crate) const SEEK_ID: u32 = 0x53AB;
pub(crate) const SEEK_POSITION: u32 = 0x53AC;

pub(crate) const INFO: u32 = 0x1549_A966;
pub(crate) const TIMESTAMP_SCALE: u32 = 0x2A_D7B1;
pub(crate) const DURATION: u32 = 0x4489;

pub(crate) const CLUSTER: u32 = 0x1F43_B675;
pub(crate) const TIMESTAMP: u32 = 0xE7;
pub(crate) const SIMPLE_BLOCK: u32 = 0xA3;
pub(crate) const BLOCK_GROUP: u32 = 0xA0;
pub(crate) const BLOCK: u32 = 0xA1;
pub(crate) const BLOCK_DURATION: u32 = 0x9B;

pub(crate) const TRACKS: u32 = 0x1654_AE6B;
pub(crate) const TRACK_ENTRY: u32 = 0xAE;
pub(crate) const TRACK_NUMBER: u32 = 0xD7;
pub(crate) const TRACK_UID: u32 = 0x73C5;
pub(crate) const DEFAULT_DURATION: u32 = 0x23_E383;
pub(crate) const LANGUAGE: u32 = 0x22_B59C;
pub(crate) const CODEC_ID: u32 = 0x86;
pub(crate) const CODEC_PRIVATE: u32 = 0x63A2;
pub(crate) const AUDIO: u32 = 0xE1;
pub(crate) const SAMPLING_FREQUENCY: u32 = 0xB5;
pub(crate) const CHANNELS: u32 = 0x9F;
pub(crate) const BIT_DEPTH: u32 = 0x6264;

pub(crate) const CUES: u32 = 0x1C53_BB6B;
pub(crate) const CUE_POINT: u32 = 0xBB;
pub(crate) const CUE_TIME: u32 = 0xB3;
pub(crate) const CUE_TRACK_POSITIONS: u32 = 0xB7;
pub(crate) const CUE_TRACK: u32 = 0xF7;
pub(crate) const CUE_CLUSTER_POSITION: u32 = 0xF1;

pub(crate) const TAGS: u32 = 0x1254_C367;
pub(crate) const TAG: u32 = 0x7373;
pub(crate) const SIMPLE_TAG: u32 = 0x67C8;
pub(crate) const TAG_NAME: u32 = 0x45A3;
pub(crate) const TAG_STRING: u32 = 0x4487;
pub(crate) const TAG_BINARY: u32 = 0x4485;
