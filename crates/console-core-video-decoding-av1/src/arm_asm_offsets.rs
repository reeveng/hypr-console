//! Check the Rust ABI against the offsets used by dav1d ARM assembly.
//!
//! Copyright © 2021, VideoLAN and dav1d authors
//! All rights reserved.
//!
//! Redistribution and use in source and binary forms, with or without
//! modification, are permitted provided that the following conditions are met:
//!
//! 1. Redistributions of source code must retain the above copyright notice, this
//!    list of conditions and the following disclaimer.
//!
//! 2. Redistributions in binary form must reproduce the above copyright notice,
//!    this list of conditions and the following disclaimer in the documentation
//!    and/or other materials provided with the distribution.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
//! ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
//! WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
//! DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR
//! ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
//! (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
//! LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
//! ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
//! (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
//! SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.


use crate::include::dav1d::headers::Dav1dFilmGrainData;

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one field list pairs the historical dav1d assembly layout with the Rust ABI"
  )
)]
macro_rules! check_offsets {
  ($ty:ty; $($name:ident = $expected:literal => $field:ident),+ $(,)?) => {
    $(const _: () = assert!(
      core::mem::offset_of!($ty, $field) == $expected,
      concat!(stringify!($name), " does not match Rust layout"),
    );)+
  };
}

check_offsets! { Dav1dFilmGrainData;
    FGD_SEED = 0 => seed,
    FGD_AR_COEFF_LAG = 92 => ar_coeff_lag,
    FGD_AR_COEFFS_Y = 96 => ar_coeffs_y,
    FGD_AR_COEFFS_UV = 120 => ar_coeffs_uv,
    FGD_AR_COEFF_SHIFT = 176 => ar_coeff_shift,
    FGD_GRAIN_SCALE_SHIFT = 184 => grain_scale_shift,
    FGD_SCALING_SHIFT = 88 => scaling_shift,
    FGD_UV_MULT = 188 => uv_mult,
    FGD_UV_LUMA_MULT = 196 => uv_luma_mult,
    FGD_UV_OFFSET = 204 => uv_offset,
    FGD_CLIP_TO_RESTRICTED_RANGE = 216 => clip_to_restricted_range,
}

#[cfg(target_arch = "aarch64")]
const _: () = {
  use crate::src::refmvs::AsmRefMvsFrame;
  check_offsets! { AsmRefMvsFrame<'static>;
      RMVSF_IW8 = 16 => iw8,
      RMVSF_IH8 = 20 => ih8,
      RMVSF_MFMV_REF = 53 => mfmv_ref,
      RMVSF_MFMV_REF2CUR = 56 => mfmv_ref2cur,
      RMVSF_MFMV_REF2REF = 68 => mfmv_ref2ref,
      RMVSF_N_MFMVS = 152 => n_mfmvs,
      RMVSF_RP_REF = 168 => rp_ref,
      RMVSF_RP_PROJ = 176 => rp_proj,
      RMVSF_RP_STRIDE = 184 => rp_stride,
      RMVSF_N_TILE_THREADS = 200 => n_tile_threads,
  }
};
