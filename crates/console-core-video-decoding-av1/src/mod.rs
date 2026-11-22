pub mod ablate;

pub(crate) mod align;
pub(crate) mod c_arc;
pub(crate) mod c_box;

pub(crate) mod cpu;
pub(crate) mod disjoint_mut;
mod ffi_safe;
mod in_range;
pub(super) mod internal;
mod intra_edge;
#[cfg_attr(not(feature = "c-ffi"), deny(unsafe_code))]
pub(crate) mod log;
#[cfg(feature = "c-ffi")]
pub mod send_sync_non_null;
mod tables;

mod data;
#[cfg_attr(not(feature = "c-ffi"), deny(unsafe_code))]
mod picture;

mod cdef;
mod filmgrain;
mod ipred;
mod itx;
mod lf_mask;
mod loopfilter;
mod looprestoration;
mod mc;
mod pal;
mod recon;
mod refmvs;

mod msac;

pub(crate) mod safe_simd;

#[cfg_attr(not(feature = "c-ffi"), deny(unsafe_code))]
#[path = "lib.rs"]
pub(crate) mod lib;

#[cfg(feature = "c-ffi")]
pub mod dav1d_api;

mod cdef_apply;
mod cdf;
mod const_fn;
mod ctx;
mod cursor;
mod decode;
mod dequant_tables;
pub(crate) mod enum_map;
mod env;
pub(crate) mod error;
mod extensions;
mod fg_apply;
mod getbits;
mod ipred_prepare;
mod iter;
mod itx_1d;
pub(crate) mod levels;
mod lf_apply;
mod lr_apply;
pub(crate) mod mem;
mod obu;
pub(crate) mod owned_recon;
pub(crate) mod pic_or_buf;

mod qm;
pub(crate) mod relaxed_atomic;
mod scan;
pub(crate) mod strided;

mod thread_task;
mod warpmv;
mod wedge;
pub(crate) mod with_offset;
pub(crate) mod wrap_fn_ptr;

#[cfg(test)]
mod decode_test;

#[cfg(test)]
mod arm_asm_offsets;

pub mod managed;
