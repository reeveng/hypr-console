//! A function written again somewhere else in the tree, found before it lands.
//!
//! The crate rule says a literal spelled in two crates means one of them is
//! guessing, and the same is true of a function. `clock` was written three
//! times, once each in the music player, the media viewer and the downloads,
//! and the walk along `PATH` that asks whether a program is installed was
//! written four times and gave four answers to an unset `PATH`. None of them
//! looked wrong in the diff that brought it, because the first copy was never
//! in that diff.
//!
//! `cargo dylint` cannot ask this for the reason `the_families` gives: a lint
//! sees one crate. So every function in `crates/` is read here with `syn`, and
//! what it says is reduced to its shape -- a local name is any local name and a
//! string is any string, while a method, a path and a type keep their names,
//! because those are what the function does rather than what it calls things.
//! Two shapes that share most of their runs of tokens are one function in two
//! places, and the fault names both.
//!
//! What is not compared is what the rules dictate. A trait impl has the shape
//! its trait gave it, and a test is a question asked in the shape every test
//! asks one, so neither is read. A body too short to be worth calling is not
//! read either, because a line of it is cheaper to write than to import.
//!
//! `ALIKE` is the groups that are alike and stay apart, each with the reason.
//! `NOT_YET` is the groups that were already in the tree the day this was
//! written, which is the ratchet the lints use: a new copy is red at once, and
//! folding an old one is deleting its line. A group on either that no longer
//! matches is a line to delete, the way an edge in `the_families` is.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use syn::visit::Visit;

use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const SHORTEST: u32 = 60;

const RUN: u32 = 6;

const MET: u32 = 8;

const ALIKE_ENOUGH: f64 = 0.7;

const ALIKE: [(&[&str], &str); 23] = [
  (
    &[
      "crates/console-home-screen/src/bin/console-home.rs::worth_asking_after",
      "crates/console-input-controller/src/binds.rs::worth_asking_after",
      "crates/console-status-bar/src/watch.rs::surface_worth_asking_after",
      "crates/console-wallpaper/src/covered.rs::worth_waking_for",
    ],
    "each listener decides which compositor events matter to it, and naming every event with no wildcard is what makes a new one get considered by each",
  ),
  (
    &[
      "crates/console-panel/src/surface.rs::looked_at",
      "crates/console-panel/src/surface.rs::still_of",
    ],
    "two questions over Picture, whether it is a file on screen and whether it is a still, and the no-wildcard rule writes the variants out once per question",
  ),
  (
    &[
      "crates/console-program-contract/src/effect.rs::as_spawn",
      "crates/console-program-contract/src/effect.rs::as_file_write",
    ],
    "one accessor per variant of Effect, and the no-wildcard rule writes the other variants out in each",
  ),
  (
    &[
      "crates/console-input-alphabets/src/wearing.rs::every",
      "crates/console-input-bindings/src/active.rs::read",
    ],
    "each reads its own file into its own type with its own default; the reading they share is console_core_atomic_writes::read already",
  ),
  (
    &[
      "crates/console-bus/src/messages.rs::listed",
      "crates/console-bus/src/messages.rs::text",
      "crates/console-bus/src/messages.rs::truth",
    ],
    "each walks every variant of one value, and a walk over an enum with no wildcard arm is written out in full once per question asked of it",
  ),
  (
    &[
      "crates/console-test-stages/src/checking.rs::desktop",
      "crates/console-test-stages/src/checking.rs::here",
    ],
    "the stage's type decides which Body variant is asked for, and a function over two types is two functions",
  ),
  (
    &[
      "crates/console-core-color/src/palette.rs::out_of",
      "crates/console-status-bar/src/showing.rs::out_of",
    ],
    "each surface's Wearing names the colours it spends and no others, and the reading they share is find_color already",
  ),
  (
    &[
      "crates/console-panel/src/frames.rs::a_red_picture",
      "crates/console-pictures/src/lib.rs::make_picture",
    ],
    "each writes a PNG of one colour for its own crate's tests, and a fixture is the thing that is meant to be spelled out where it is used",
  ),
  (
    &[
      "crates/console-core-webp-files/src/forward.rs::walsh_row",
      "crates/console-core-webp-files/src/inverse.rs::walsh_column",
    ],
    "the encoder's Walsh-Hadamard butterfly and the decoder's pair the four points differently and hand back their sums in a different order, as libwebp's two do; one written through the other is a permutation on each side of a call",
  ),
  (
    &[
      "crates/console-input-keymaps/src/xkb/kccgst/expr.rs::brace_list_of",
      "crates/console-input-keymaps/src/xkb/kccgst/expr.rs::bracket_list_of",
      "crates/console-input-keymaps/src/xkb/kccgst/expr.rs::call_of",
      "crates/console-input-keymaps/src/xkb/kccgst/expr.rs::key_name_of",
      "crates/console-input-keymaps/src/xkb/kccgst/expr.rs::path_of",
      "crates/console-input-keymaps/src/xkb/kccgst/expr.rs::string_of",
    ],
    "one accessor per variant of Expr, and the no-wildcard rule writes the other variants out in each",
  ),
  (
    &[
      "crates/console-input-keymaps/src/xkb/kccgst/parser.rs::parse_ident",
      "crates/console-input-keymaps/src/xkb/kccgst/parser.rs::parse_keyname",
      "crates/console-input-keymaps/src/xkb/kccgst/parser.rs::parse_string",
    ],
    "one per kind of token that carries a name, and the no-wildcard rule writes the other kinds out in each",
  ),
  (
    &[
      "crates/console-input-keymaps/src/xkb/kccgst/parser.rs::consume_token",
      "crates/console-input-keymaps/src/xkb/rmlvo/parser.rs::consume_token",
    ],
    "the keymap grammar and the rules grammar each have their own tokens and punctuation, and a step over two token types is two steps",
  ),
  (
    &[
      "crates/console-input-keymaps/src/xkb/kccgst/parser/error.rs::write_actual",
      "crates/console-input-keymaps/src/xkb/rmlvo/parser/error.rs::write_actual",
    ],
    "each names the tokens of its own grammar, and the rules grammar has group names where the keymap grammar has strings, key names and numbers",
  ),
  (
    &[
      "crates/console-input-keymaps/src/xkb/meaning/generated.rs::lowercase",
      "crates/console-input-keymaps/src/xkb/meaning/generated.rs::name",
    ],
    "generated from one list of words, the name each is written with and the lowercase it is matched by, and lowercasing the first at run time would hand back a String where a static was",
  ),
  (
    &[
      "crates/console-core-text-layout/src/buffer.rs::cursor_from_glyph_left",
      "crates/console-core-text-layout/src/buffer.rs::cursor_from_glyph_right",
    ],
    "the two edges of one glyph, each the other's mirror, and which byte and which affinity an edge takes turns on the run's direction",
  ),
  (
    &[
      "crates/console-core-text-layout/src/buffer.rs::draw",
      "crates/console-core-text-layout/src/buffer.rs::render",
    ],
    "draw hands each glyph to a callback through a renderer that holds the font system, which render takes beside its renderer, so one calling the other would lend the font system twice",
  ),
  (
    &[
      "crates/console-core-text-layout/src/font/fallback/mod.rs::common_fallback",
      "crates/console-core-text-layout/src/font/fallback/mod.rs::forbidden_fallback",
    ],
    "two lists cut out of one table by two ranges, and one function taking the range would hand every caller a range to get wrong",
  ),
  (
    &[
      "crates/console-core-video-decoding-advanced-video-coding/src/cabac.rs::new",
      "crates/console-core-video-decoding-advanced-video-coding/src/cabac_mb.rs::cbf_chroma_ac_cond",
      "crates/console-core-video-decoding-advanced-video-coding/src/cabac_mb.rs::cbf_luma_cond",
      "crates/console-core-video-decoding-advanced-video-coding/src/decoder.rs::decode",
      "crates/console-core-video-decoding-advanced-video-coding/src/decoder.rs::default_threads",
      "crates/console-core-video-decoding-advanced-video-coding/src/decoder.rs::dispatch",
      "crates/console-core-video-decoding-advanced-video-coding/src/decoder.rs::flush",
      "crates/console-core-video-decoding-advanced-video-coding/src/decoder.rs::output_meta",
      "crates/console-core-video-decoding-advanced-video-coding/src/picture.rs::copy_clamped",
      "crates/console-core-video-decoding-advanced-video-coding/src/slicedec.rs::add_luma_block4",
      "crates/console-core-video-decoding-advanced-video-coding/src/slicedec.rs::add_luma_block8",
      "crates/console-core-video-decoding-av1/include/dav1d/picture.rs::narrow_guard",
      "crates/console-core-video-decoding-av1/include/dav1d/picture.rs::narrow_guard_mut",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part02_col_1d_avx512_pmaddwd.rs::adst16x16_cols_simd",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part02_col_1d_avx512_pmaddwd.rs::dct16_cols_avx512",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part02_col_1d_avx512_pmaddwd.rs::dct32_cols_avx512",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part02_col_1d_avx512_pmaddwd.rs::dct4_cols_avx512",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part02_col_1d_avx512_pmaddwd.rs::dct8_cols_avx512",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part02_col_1d_avx512_pmaddwd.rs::flipadst16x16_cols_simd",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part02_col_1d_avx512_pmaddwd.rs::row_dct16_8bpc_block",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part02_col_1d_avx512_pmaddwd.rs::row_dct32_8bpc_block",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part02_col_1d_avx512_pmaddwd.rs::row_dct8_8bpc_block",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part03_adst16_and_small_rect.rs::inv_txfm_add_dct_dct_16x16_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part04_rect_idtx_aspect.rs::inv_txfm_add_dct_dct_32x16_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part04_rect_idtx_aspect.rs::inv_txfm_add_dct_dct_32x8_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part04_rect_idtx_aspect.rs::inv_txfm_add_dct_dct_64x16_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part04_rect_idtx_aspect.rs::inv_txfm_add_dct_dct_64x32_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part04_rect_idtx_aspect.rs::simd_row_adst16_8bpc_8rows",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part04_rect_idtx_aspect.rs::simd_row_adst8_8bpc_8rows",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part04_rect_idtx_aspect.rs::simd_row_dct16_8bpc_8rows",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part04_rect_idtx_aspect.rs::simd_row_dct32_8bpc_8rows",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part04_rect_idtx_aspect.rs::simd_row_dct8_8bpc_8rows",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part06_adst_flipadst_vh.rs::inv_txfm_add_adst_adst_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part06_adst_flipadst_vh.rs::inv_txfm_add_adst_dct_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part06_adst_flipadst_vh.rs::inv_txfm_add_adst_flipadst_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part06_adst_flipadst_vh.rs::inv_txfm_add_dct_adst_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part06_adst_flipadst_vh.rs::inv_txfm_add_dct_flipadst_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part06_adst_flipadst_vh.rs::inv_txfm_add_dct_identity_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part06_adst_flipadst_vh.rs::inv_txfm_add_flipadst_adst_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part06_adst_flipadst_vh.rs::inv_txfm_add_flipadst_dct_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part06_adst_flipadst_vh.rs::inv_txfm_add_flipadst_flipadst_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part06_adst_flipadst_vh.rs::inv_txfm_add_h_adst_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part06_adst_flipadst_vh.rs::inv_txfm_add_h_flipadst_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part06_adst_flipadst_vh.rs::inv_txfm_add_identity_dct_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part06_adst_flipadst_vh.rs::inv_txfm_add_v_adst_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part06_adst_flipadst_vh.rs::inv_txfm_add_v_flipadst_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part07_32x32_64x64_dct.rs::add_32x32_to_dst",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part07_32x32_64x64_dct.rs::add_32x32_to_dst_16bpc",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part07_32x32_64x64_dct.rs::add_64x64_to_dst",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part07_32x32_64x64_dct.rs::add_64x64_to_dst_16bpc",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part07_32x32_64x64_dct.rs::inv_txfm_add_dct_dct_32x32_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part07_32x32_64x64_dct.rs::inv_txfm_add_dct_dct_64x64_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part08_rect_dct_adst_16bpc.rs::inv_txfm_add_dct_dct_16x32_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part08_rect_dct_adst_16bpc.rs::inv_txfm_add_dct_dct_16x4_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part08_rect_dct_adst_16bpc.rs::inv_txfm_add_dct_dct_16x8_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part08_rect_dct_adst_16bpc.rs::inv_txfm_add_dct_dct_32x16_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part08_rect_dct_adst_16bpc.rs::inv_txfm_add_dct_dct_32x8_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part08_rect_dct_adst_16bpc.rs::inv_txfm_add_dct_dct_4x16_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part08_rect_dct_adst_16bpc.rs::inv_txfm_add_dct_dct_4x8_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part08_rect_dct_adst_16bpc.rs::inv_txfm_add_dct_dct_64x16_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part08_rect_dct_adst_16bpc.rs::inv_txfm_add_dct_dct_64x32_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part08_rect_dct_adst_16bpc.rs::inv_txfm_add_dct_dct_8x16_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part08_rect_dct_adst_16bpc.rs::inv_txfm_add_dct_dct_8x32_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part08_rect_dct_adst_16bpc.rs::inv_txfm_add_dct_dct_8x4_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part09_identity_hybrid_16bpc.rs::inv_txfm_add_identity_identity_16x32_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part09_identity_hybrid_16bpc.rs::inv_txfm_add_identity_identity_16x8_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part09_identity_hybrid_16bpc.rs::inv_txfm_add_identity_identity_32x32_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part09_identity_hybrid_16bpc.rs::inv_txfm_add_identity_identity_4x16_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part09_identity_hybrid_16bpc.rs::inv_txfm_add_identity_identity_4x8_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part09_identity_hybrid_16bpc.rs::inv_txfm_add_identity_identity_8x16_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part09_identity_hybrid_16bpc.rs::inv_txfm_add_identity_identity_8x32_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/itx/part09_identity_hybrid_16bpc.rs::inv_txfm_add_identity_identity_8x4_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/include/safe_simd/loopfilter_packed16.rs::transpose8",
      "crates/console-core-video-decoding-av1/include/safe_simd/loopfilter_packed6.rs::transpose8",
      "crates/console-core-video-decoding-av1/src/dav1d_api.rs::dav1d_data_props_unref",
      "crates/console-core-video-decoding-av1/src/dav1d_api.rs::dav1d_data_unref",
      "crates/console-core-video-decoding-av1/src/dav1d_api.rs::dav1d_get_decode_error_data_props",
      "crates/console-core-video-decoding-av1/src/dav1d_api.rs::dav1d_get_event_flags",
      "crates/console-core-video-decoding-av1/src/decode.rs::splat_intrabc_mv",
      "crates/console-core-video-decoding-av1/src/decode.rs::splat_intraref",
      "crates/console-core-video-decoding-av1/src/filmgrain.rs::get_random_number",
      "crates/console-core-video-decoding-av1/src/filmgrain.rs::row_seed",
      "crates/console-core-video-decoding-av1/src/getbits.rs::get_bits",
      "crates/console-core-video-decoding-av1/src/getbits.rs::get_sbits",
      "crates/console-core-video-decoding-av1/src/ipred.rs::dc_gen_left",
      "crates/console-core-video-decoding-av1/src/ipred.rs::dc_gen_top",
      "crates/console-core-video-decoding-av1/src/ipred.rs::get_filter_strength",
      "crates/console-core-video-decoding-av1/src/ipred.rs::ipred_smooth_h_rust",
      "crates/console-core-video-decoding-av1/src/ipred.rs::ipred_smooth_rust",
      "crates/console-core-video-decoding-av1/src/ipred.rs::ipred_smooth_v_rust",
      "crates/console-core-video-decoding-av1/src/lf_mask.rs::rav1d_create_lf_mask_inter",
      "crates/console-core-video-decoding-av1/src/lf_mask.rs::rav1d_create_lf_mask_intra",
      "crates/console-core-video-decoding-av1/src/loopfilter.rs::iclip_diff",
      "crates/console-core-video-decoding-av1/src/looprestoration.rs::eight_neighbors",
      "crates/console-core-video-decoding-av1/src/looprestoration.rs::sgr_3x3_rust",
      "crates/console-core-video-decoding-av1/src/looprestoration.rs::sgr_5x5_rust",
      "crates/console-core-video-decoding-av1/src/looprestoration.rs::sgr_mix_rust",
      "crates/console-core-video-decoding-av1/src/looprestoration.rs::six_neighbors",
      "crates/console-core-video-decoding-av1/src/msac.rs::rav1d_msac_decode_bool_equi_rust",
      "crates/console-core-video-decoding-av1/src/msac.rs::rav1d_msac_decode_bool_rust",
      "crates/console-core-video-decoding-av1/src/owned_recon.rs::for_rows",
      "crates/console-core-video-decoding-av1/src/owned_recon.rs::for_rows_mut",
      "crates/console-core-video-decoding-av1/src/owned_recon.rs::get",
      "crates/console-core-video-decoding-av1/src/owned_recon.rs::pixel_stride",
      "crates/console-core-video-decoding-av1/src/owned_recon.rs::px",
      "crates/console-core-video-decoding-av1/src/owned_recon.rs::px_mut",
      "crates/console-core-video-decoding-av1/src/owned_recon.rs::slice",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef.rs::cdef_filter_4x4_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef.rs::cdef_filter_4x4_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef.rs::cdef_filter_4x8_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef.rs::cdef_filter_4x8_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef.rs::cdef_filter_8x8_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef.rs::cdef_filter_8x8_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef.rs::cdef_filter_block_scalar_16bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef.rs::cdef_filter_block_scalar_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef.rs::cdef_find_dir_simd_16bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef.rs::cdef_find_dir_simd_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef.rs::padding_16bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef.rs::padding_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef_arm.rs::cdef_filter_block_16bpc_dispatch",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef_arm.rs::cdef_filter_block_8bpc_dispatch",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef_wasm.rs::cdef_filter_4x4_16bpc_wasm_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef_wasm.rs::cdef_filter_4x4_8bpc_wasm_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef_wasm.rs::cdef_filter_4x8_16bpc_wasm_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef_wasm.rs::cdef_filter_4x8_8bpc_wasm_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef_wasm.rs::cdef_filter_8x8_16bpc_wasm_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/cdef_wasm.rs::cdef_filter_8x8_8bpc_wasm_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/filmgrain.rs::generate_grain_uv_inner_16bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/filmgrain.rs::generate_grain_uv_inner_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/filmgrain.rs::generate_grain_y_inner_16bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/filmgrain.rs::generate_grain_y_inner_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/filmgrain.rs::get_random_number",
      "crates/console-core-video-decoding-av1/src/safe_simd/filmgrain.rs::row_seed",
      "crates/console-core-video-decoding-av1/src/safe_simd/filmgrain_arm.rs::fguv_row_neon_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/filmgrain_arm.rs::fgy_inner_16bpc_rows",
      "crates/console-core-video-decoding-av1/src/safe_simd/filmgrain_arm.rs::fgy_inner_8bpc_rows",
      "crates/console-core-video-decoding-av1/src/safe_simd/filmgrain_arm.rs::fgy_row_neon_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/filmgrain_arm.rs::generate_grain_uv_inner_16bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/filmgrain_arm.rs::generate_grain_uv_inner_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/filmgrain_arm.rs::generate_grain_y_inner_16bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/filmgrain_arm.rs::generate_grain_y_inner_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::cfl_ac_420_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::cfl_ac_422_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::cfl_ac_444_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::cfl_pred_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::cfl_pred_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::get_filter_strength_simple",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_128_16bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_128_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_128_8bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_128_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_16bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_8bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_left_16bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_left_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_left_8bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_left_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_top_16bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_top_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_top_8bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_dc_top_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_h_16bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_h_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_h_8bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_h_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_smooth_16bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_smooth_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_smooth_8bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_smooth_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_smooth_h_16bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_smooth_h_8bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_smooth_h_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_smooth_v_16bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_smooth_v_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_smooth_v_8bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_smooth_v_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_v_16bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_v_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_v_8bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::ipred_v_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::run_z1",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::run_z2",
      "crates/console-core-video-decoding-av1/src/safe_simd/ipred.rs::run_z3",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::identity_rect_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::identity_rect_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_adst_adst_4x4_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_adst_adst_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_adst_adst_8x8_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_adst_adst_8x8_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_adst_dct_4x4_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_adst_dct_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_adst_dct_8x8_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_adst_dct_8x8_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_adst_flipadst_4x4_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_adst_flipadst_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_adst_identity_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_adst_4x4_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_adst_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_adst_8x8_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_adst_8x8_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_16x16_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_16x16_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_16x32_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_16x32_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_16x4_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_16x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_16x64_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_16x64_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_16x8_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_16x8_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_32x16_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_32x16_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_32x32_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_32x32_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_32x64_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_32x64_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_32x8_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_32x8_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_4x16_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_4x16_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_4x4_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_4x8_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_4x8_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_64x16_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_64x16_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_64x32_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_64x32_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_64x64_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_64x64_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_8x16_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_8x16_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_8x32_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_8x32_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_8x4_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_8x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_8x8_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_dct_8x8_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_flipadst_4x4_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_flipadst_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_dct_identity_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_flipadst_adst_4x4_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_flipadst_adst_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_flipadst_dct_4x4_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_flipadst_dct_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_flipadst_flipadst_4x4_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_flipadst_flipadst_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_flipadst_identity_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_generic_16bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_generic_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_identity_adst_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_identity_dct_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_identity_flipadst_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_identity_identity_16x16_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_identity_identity_16x16_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_identity_identity_4x4_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_identity_identity_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_identity_identity_8x8_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_identity_identity_8x8_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_wht_wht_4x4_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm.rs::inv_txfm_add_wht_wht_4x4_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_16x16.rs::add_to_dst_8x16_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_16x16.rs::apply_tx16",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_16x16.rs::dc_only_16x16_16bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_16x16.rs::dc_only_16x16_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_16x16.rs::inv_txfm_add_16x16_16bpc_neon",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_16x16.rs::inv_txfm_add_16x16_8bpc_neon",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_16x16.rs::scalar_dct4_1d",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_16x16.rs::scalar_dct8_1d",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_32.rs::dc_only_32x32_16bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_32.rs::dc_only_32x32_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_32.rs::horz_dct_32x8",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_32.rs::identity_32x32_8bpc_impl",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_32.rs::scalar_dct16_1d",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_32.rs::scalar_dct_dct_32x32_16bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_8x8.rs::add_to_dst_8x8_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_8x8.rs::dc_only_8x8_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_8x8.rs::scalar_adst_adst_8x8",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_8x8.rs::scalar_dct4_1d",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_8x8.rs::scalar_dct8_1d",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_8x8.rs::scalar_dct_dct_8x8",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_8x8.rs::smull_smlal_q",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_8x8.rs::smull_smlsl_q",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_common.rs::transpose_4x4h",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_neon_wht.rs::transpose_4x4h",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_parity.rs::finish",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_parity.rs::run_cell",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_parity.rs::run_cell_16",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_parity.rs::sweep16",
      "crates/console-core-video-decoding-av1/src/safe_simd/itx_arm_parity.rs::sweep_with",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::iclip_diff",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::loop_filter_4_8bpc_narrow_simd_v",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::loop_filter_4_8bpc_narrow_simd_v_x8",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::loop_filter_4_8bpc_wd16_simd_v",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::loop_filter_4_8bpc_wd16_simd_v_x8",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::loop_filter_4_8bpc_wd6_simd_h",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::loop_filter_4_8bpc_wd6_simd_v",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::loop_filter_4_8bpc_wd8_simd_h",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::loop_filter_4_8bpc_wd8_simd_v",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::loop_filter_4_8bpc_wd8_simd_v_x8",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::lpf_h_sb_uv_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::lpf_h_sb_uv_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::lpf_h_sb_y_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::lpf_v_sb_uv_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::lpf_v_sb_uv_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/loopfilter.rs::lpf_v_sb_y_16bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/looprestoration.rs::boxsum3_h_16bpc_avx512",
      "crates/console-core-video-decoding-av1/src/safe_simd/looprestoration.rs::boxsum3_h_avx2",
      "crates/console-core-video-decoding-av1/src/safe_simd/looprestoration.rs::boxsum5_h_16bpc_avx512",
      "crates/console-core-video-decoding-av1/src/safe_simd/looprestoration.rs::wiener_filter7_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/looprestoration.rs::wiener_filter7_8bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/looprestoration_arm.rs::boxsum_16bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/looprestoration_arm.rs::boxsum_8bpc",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::avg_16bpc_avx2_safe",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::avg_16bpc_avx512_safe",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::avg_dispatch",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::avg_dispatch_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::blend_16bpc_avx2_safe",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::blend_8bpc_avx2_safe",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::blend_dir_dispatch",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::blend_dispatch",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::blend_h_16bpc_avx2_safe",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::blend_h_8bpc_avx2_safe",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::blend_v_16bpc_avx2_safe",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::blend_v_8bpc_avx2_safe",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::h_bilin_16bpc_prep_direct_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::h_bilin_16bpc_prep_direct_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::h_bilin_16bpc_put_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::h_bilin_16bpc_put_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::h_bilin_8bpc_put_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::h_bilin_8bpc_put_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::h_filter_8tap_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::h_filter_8tap_16bpc_prep_direct_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::h_filter_8tap_16bpc_put_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::h_filter_8tap_8bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::h_filter_8tap_8bpc_put_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::h_filter_bilin_8bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::mask_dispatch",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::mask_dispatch_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::prep_8tap_16bpc_avx2_impl_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::prep_8tap_16bpc_avx512_impl_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::put_8tap_8bpc_avx2_impl_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::put_8tap_8bpc_avx512_impl_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_bilin_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_bilin_16bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_bilin_16bpc_direct_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_bilin_16bpc_direct_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_bilin_16bpc_prep_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_bilin_16bpc_prep_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_bilin_16bpc_prep_direct_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_bilin_16bpc_prep_direct_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_bilin_8bpc_direct_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_bilin_8bpc_direct_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_bilin_8bpc_prep_direct_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_filter_8tap_16bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_filter_8tap_16bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_filter_8tap_16bpc_direct_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_filter_8tap_16bpc_direct_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_filter_8tap_16bpc_prep_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_filter_8tap_16bpc_prep_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_filter_8tap_16bpc_prep_direct_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_filter_8tap_16bpc_prep_direct_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_filter_8tap_8bpc_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_filter_8tap_8bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_filter_8tap_to_i16_avx2_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_filter_8tap_to_i16_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_filter_bilin_8bpc_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::v_filter_bilin_8bpc_prep_avx512_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc.rs::w_avg_dispatch",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_arm.rs::h_filter_8tap_8bpc_dotprod",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_arm.rs::h_filter_8tap_8bpc_i8mm",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_arm.rs::h_filter_8tap_8bpc_neon",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_arm.rs::h_filter_8tap_8bpc_put_neon",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_arm.rs::mask_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_arm.rs::mc_put_dispatch",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_arm.rs::v_filter_8tap_16bpc_neon",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_arm.rs::v_filter_8tap_16bpc_to_i16_neon",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_arm.rs::w_avg_8bpc_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_arm.rs::w_mask_dispatch",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_arm_prep_parity.rs::finish",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_wasm.rs::avg_16bpc_wasm128",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_wasm.rs::avg_dispatch",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_wasm.rs::avg_dispatch_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_wasm.rs::mask_dispatch",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_wasm.rs::mask_dispatch_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_wasm.rs::w_avg_16bpc_wasm128",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_wasm.rs::w_avg_dispatch",
      "crates/console-core-video-decoding-av1/src/safe_simd/mc_wasm.rs::w_avg_dispatch_inner",
      "crates/console-core-video-decoding-av1/src/safe_simd/pixel_access.rs::reinterpret_ref",
      "crates/console-core-video-decoding-av1/src/safe_simd/pixel_access.rs::reinterpret_slice",
      "crates/console-core-video-decoding-high-efficiency-video-coding/src/cabac.rs::new",
      "crates/console-core-video-decoding-high-efficiency-video-coding/src/decoder.rs::decode",
      "crates/console-core-video-decoding-high-efficiency-video-coding/src/decoder.rs::default_threads",
      "crates/console-core-video-decoding-high-efficiency-video-coding/src/decoder.rs::dispatch",
      "crates/console-core-video-decoding-high-efficiency-video-coding/src/decoder.rs::flush",
      "crates/console-core-video-decoding-high-efficiency-video-coding/src/decoder.rs::output_meta",
      "crates/console-core-video-decoding-high-efficiency-video-coding/src/picture.rs::chroma_window",
      "crates/console-core-video-decoding-high-efficiency-video-coding/src/picture.rs::copy_line",
      "crates/console-core-video-decoding-high-efficiency-video-coding/src/picture.rs::luma_window",
      "crates/console-core-video-decoding-vp9/src/decoder.rs::default_threads",
      "crates/console-core-video-decoding-vp9/src/frame.rs::take",
      "crates/console-core-video-decoding-vp9/src/frame.rs::take_any",
      "crates/console-core-video-decoding-vp9/src/inter.rs::filter_h",
      "crates/console-core-video-decoding-vp9/src/inter.rs::filter_h_n",
      "crates/console-core-video-decoding-vp9/src/inter.rs::filter_v",
      "crates/console-core-video-decoding-vp9/src/inter.rs::filter_v_n",
      "crates/console-core-video-decoding-vp9/src/tile.rs::clamp_mv_col",
      "crates/console-core-video-decoding-vp9/src/tile.rs::clamp_mv_row",
    ],
    "AV1 codec kernels repeat the same shape per bit depth, instruction set, and transform size; each variant is a separate function and must stay apart.",
  ),
  (
    &[
      "crates/console-core-font-reading/generated/generated_aat.rs::class_array_byte_range",
      "crates/console-core-font-reading/generated/generated_aat.rs::data_byte_range",
      "crates/console-core-font-reading/generated/generated_aat.rs::dyn_inner",
      "crates/console-core-font-reading/generated/generated_aat.rs::entries_data_byte_range",
      "crates/console-core-font-reading/generated/generated_aat.rs::format",
      "crates/console-core-font-reading/generated/generated_aat.rs::offset_data",
      "crates/console-core-font-reading/generated/generated_aat.rs::segments_data_byte_range",
      "crates/console-core-font-reading/generated/generated_aat.rs::value_array_byte_range",
      "crates/console-core-font-reading/generated/generated_aat.rs::values_data_byte_range",
      "crates/console-core-font-reading/generated/generated_avar.rs::axis_index_map_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_avar.rs::var_store_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_base.rs::item_var_store_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_bitmap.rs::sbit_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_cblc.rs::bitmap_sizes_byte_range",
      "crates/console-core-font-reading/generated/generated_cff.rs::data_byte_range",
      "crates/console-core-font-reading/generated/generated_cff.rs::fds_byte_range",
      "crates/console-core-font-reading/generated/generated_cff.rs::glyph_byte_range",
      "crates/console-core-font-reading/generated/generated_cff.rs::offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_cff.rs::ranges_byte_range",
      "crates/console-core-font-reading/generated/generated_cff.rs::trailing_data_byte_range",
      "crates/console-core-font-reading/generated/generated_cff2.rs::data_byte_range",
      "crates/console-core-font-reading/generated/generated_cff2.rs::offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_cff2.rs::trailing_data_byte_range",
      "crates/console-core-font-reading/generated/generated_cmap.rs::dyn_inner",
      "crates/console-core-font-reading/generated/generated_cmap.rs::end_code_byte_range",
      "crates/console-core-font-reading/generated/generated_cmap.rs::format",
      "crates/console-core-font-reading/generated/generated_cmap.rs::glyph_id_array_byte_range",
      "crates/console-core-font-reading/generated/generated_cmap.rs::groups_byte_range",
      "crates/console-core-font-reading/generated/generated_cmap.rs::id_range_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_cmap.rs::is32_byte_range",
      "crates/console-core-font-reading/generated/generated_cmap.rs::offset_data",
      "crates/console-core-font-reading/generated/generated_cmap.rs::start_code_byte_range",
      "crates/console-core-font-reading/generated/generated_cmap.rs::sub_header_keys_byte_range",
      "crates/console-core-font-reading/generated/generated_colr.rs::base_glyph_list_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_colr.rs::clip_list_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_colr.rs::color_stops_byte_range",
      "crates/console-core-font-reading/generated/generated_colr.rs::format",
      "crates/console-core-font-reading/generated/generated_colr.rs::item_variation_store_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_colr.rs::layer_list_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_colr.rs::offset_data",
      "crates/console-core-font-reading/generated/generated_colr.rs::var_index_map_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_cpal.rs::palette_entry_labels_array_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_cpal.rs::palette_labels_array",
      "crates/console-core-font-reading/generated/generated_cpal.rs::palette_labels_array_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_cpal.rs::palette_types_array",
      "crates/console-core-font-reading/generated/generated_cpal.rs::palette_types_array_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_eblc.rs::bitmap_sizes_byte_range",
      "crates/console-core-font-reading/generated/generated_gdef.rs::attach_point_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_gdef.rs::item_var_store_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_gdef.rs::mark_glyph_sets_def_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_glyf.rs::component_data_byte_range",
      "crates/console-core-font-reading/generated/generated_glyf.rs::glyph_data_byte_range",
      "crates/console-core-font-reading/generated/generated_gpos.rs::base_records_byte_range",
      "crates/console-core-font-reading/generated/generated_gpos.rs::feature_variations_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_gpos.rs::ligature_attach_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_gsub.rs::alternate_glyph_ids_byte_range",
      "crates/console-core-font-reading/generated/generated_gsub.rs::backtrack_coverage_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_gsub.rs::feature_variations_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_gsub.rs::ligature_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_gsub.rs::lookahead_coverage_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_gsub.rs::substitute_glyph_ids_byte_range",
      "crates/console-core-font-reading/generated/generated_ift.rs::brotli_stream_byte_range",
      "crates/console-core-font-reading/generated/generated_ift.rs::cff2_charstrings_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_ift.rs::cff_charstrings_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_ift.rs::data_byte_range",
      "crates/console-core-font-reading/generated/generated_ift.rs::design_space_count_byte_range",
      "crates/console-core-font-reading/generated/generated_ift.rs::design_space_segments_byte_range",
      "crates/console-core-font-reading/generated/generated_ift.rs::entry_data_byte_range",
      "crates/console-core-font-reading/generated/generated_ift.rs::entry_map_data_byte_range",
      "crates/console-core-font-reading/generated/generated_ift.rs::feature_count_byte_range",
      "crates/console-core-font-reading/generated/generated_ift.rs::feature_tags_byte_range",
      "crates/console-core-font-reading/generated/generated_ift.rs::id_data_byte_range",
      "crates/console-core-font-reading/generated/generated_ift.rs::trailing_data_byte_range",
      "crates/console-core-font-reading/generated/generated_ift.rs::url_template_byte_range",
      "crates/console-core-font-reading/generated/generated_kern.rs::data_byte_range",
      "crates/console-core-font-reading/generated/generated_kern.rs::left_class_byte_range",
      "crates/console-core-font-reading/generated/generated_kern.rs::offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_kern.rs::pairs_byte_range",
      "crates/console-core-font-reading/generated/generated_kern.rs::right_class_byte_range",
      "crates/console-core-font-reading/generated/generated_kern.rs::subtable_data_byte_range",
      "crates/console-core-font-reading/generated/generated_kerx.rs::data_byte_range",
      "crates/console-core-font-reading/generated/generated_kerx.rs::pairs_byte_range",
      "crates/console-core-font-reading/generated/generated_layout.rs::backtrack_coverage_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_layout.rs::backtrack_sequence_byte_range",
      "crates/console-core-font-reading/generated/generated_layout.rs::class_value_array_byte_range",
      "crates/console-core-font-reading/generated/generated_layout.rs::condition_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_layout.rs::coverage_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_layout.rs::glyph_array_byte_range",
      "crates/console-core-font-reading/generated/generated_layout.rs::input_sequence_byte_range",
      "crates/console-core-font-reading/generated/generated_layout.rs::lookahead_coverage_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_layout.rs::lookahead_sequence_byte_range",
      "crates/console-core-font-reading/generated/generated_layout.rs::seq_lookup_records_byte_range",
      "crates/console-core-font-reading/generated/generated_layout.rs::subtable_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_morx.rs::data_byte_range",
      "crates/console-core-font-reading/generated/generated_name.rs::lang_tag_count_byte_range",
      "crates/console-core-font-reading/generated/generated_os2.rs::panose_10_byte_range",
      "crates/console-core-font-reading/generated/generated_os2.rs::s_cap_height_byte_range",
      "crates/console-core-font-reading/generated/generated_os2.rs::sx_height_byte_range",
      "crates/console-core-font-reading/generated/generated_os2.rs::ul_code_page_range_1_byte_range",
      "crates/console-core-font-reading/generated/generated_os2.rs::ul_code_page_range_2_byte_range",
      "crates/console-core-font-reading/generated/generated_os2.rs::us_break_char_byte_range",
      "crates/console-core-font-reading/generated/generated_os2.rs::us_default_char_byte_range",
      "crates/console-core-font-reading/generated/generated_os2.rs::us_lower_optical_point_size_byte_range",
      "crates/console-core-font-reading/generated/generated_os2.rs::us_max_context_byte_range",
      "crates/console-core-font-reading/generated/generated_os2.rs::us_upper_optical_point_size_byte_range",
      "crates/console-core-font-reading/generated/generated_post.rs::num_glyphs_byte_range",
      "crates/console-core-font-reading/generated/generated_sbix.rs::data_byte_range",
      "crates/console-core-font-reading/generated/generated_stat.rs::elided_fallback_name_id_byte_range",
      "crates/console-core-font-reading/generated/generated_test_conditions.rs::bar_byte_range",
      "crates/console-core-font-reading/generated/generated_test_conditions.rs::foo_byte_range",
      "crates/console-core-font-reading/generated/generated_test_conditions.rs::if_11_byte_range",
      "crates/console-core-font-reading/generated/generated_test_conditions.rs::if_20_byte_range",
      "crates/console-core-font-reading/generated/generated_test_count_all.rs::remainder_byte_range",
      "crates/console-core-font-reading/generated/generated_test_generic_group.rs::subtable_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_test_generic_group.rs::values_byte_range",
      "crates/console-core-font-reading/generated/generated_test_offsets_arrays.rs::nonnullable_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_test_offsets_arrays.rs::nullable_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_test_offsets_arrays.rs::scalars_byte_range",
      "crates/console-core-font-reading/generated/generated_test_offsets_arrays.rs::versioned_nonnullable_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_test_offsets_arrays.rs::versioned_nonnullable_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_test_offsets_arrays.rs::versioned_nullable_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_test_offsets_arrays.rs::versioned_nullable_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_test_offsets_arrays.rs::versioned_nullable_record_array_offset_byte_range",
      "crates/console-core-font-reading/generated/generated_test_offsets_arrays.rs::versioned_records_byte_range",
      "crates/console-core-font-reading/generated/generated_test_offsets_arrays.rs::versioned_scalars_byte_range",
      "crates/console-core-font-reading/generated/generated_test_read_args.rs::base_records_byte_range",
      "crates/console-core-font-reading/generated/generated_test_read_args.rs::face_records_byte_range",
      "crates/console-core-font-reading/generated/generated_test_records.rs::data_byte_range",
      "crates/console-core-font-reading/generated/generated_varc.rs::condition_offsets_byte_range",
      "crates/console-core-font-reading/generated/generated_varc.rs::raw_delta_sets_byte_range",
      "crates/console-core-font-reading/generated/generated_varc.rs::region_indices_byte_range",
      "crates/console-core-font-reading/generated/generated_variations.rs::intermediate_end_tuple_byte_range",
      "crates/console-core-font-reading/generated/generated_variations.rs::intermediate_start_tuple_byte_range",
      "crates/console-core-font-reading/generated/generated_variations.rs::map_data_byte_range",
      "crates/console-core-font-reading/generated/generated_variations.rs::peak_tuple_byte_range",
      "crates/console-core-font-reading/generated/generated_variations.rs::region_indexes_byte_range",
      "crates/console-core-font-reading/src/tables/colr/closure.rs::v1_closure",
    ],
    "font table accessors are generated from one template, one function per table, each returning its own byte range or inner value.",
  ),
  (
    &[
      "crates/console-core-font-hinting/src/truetype/engine/control_flow.rs::op_else",
      "crates/console-core-font-hinting/src/truetype/engine/control_flow.rs::op_if",
      "crates/console-core-font-hinting/src/truetype/engine/data.rs::set_test_vectors",
      "crates/console-core-font-hinting/src/truetype/engine/graphics.rs::op_sfvfs",
      "crates/console-core-font-hinting/src/truetype/engine/graphics.rs::op_spvfs",
      "crates/console-core-font-hinting/src/truetype/engine/logical.rs::op_even",
      "crates/console-core-font-hinting/src/truetype/engine/logical.rs::op_odd",
      "crates/console-core-font-hinting/src/truetype/engine/outline.rs::set_test_vectors",
      "crates/console-core-font-hinting/src/truetype/projection.rs::dual_project",
      "crates/console-core-font-hinting/src/truetype/projection.rs::project",
      "crates/console-core-font-hinting/src/truetype/zone.rs::touch",
      "crates/console-core-font-hinting/src/truetype/zone.rs::untouch",
    ],
    "TrueType opcode handlers mirror each other across 16 and 8 bit paths and across conditional and unconditional forms.",
  ),
  (
    &[
      "crates/console-core-font-database/src/ttf_parser/language.rs::primary_language",
      "crates/console-core-font-database/src/ttf_parser/language.rs::region",
      "crates/console-core-font-database/src/ttf_parser/parser.rs::get",
      "crates/console-core-font-database/src/ttf_parser/parser.rs::read_array16",
      "crates/console-core-font-database/src/ttf_parser/parser.rs::read_array32",
    ],
    "font name parsers repeat the same read pattern per language, region, and array width.",
  ),
  (
    &[
      "crates/console-core-font-metrics/src/outline/cff/mod.rs::new",
      "crates/console-core-font-metrics/src/outline/metrics.rs::advance_width",
      "crates/console-core-font-metrics/src/outline/metrics.rs::lsb",
      "crates/console-core-font-metrics/src/outline/varc/mod.rs::apply_transform_variations",
      "crates/console-core-font-metrics/src/outline/varc/mod.rs::apply_transform_variations_reference",
      "crates/console-core-font-reading/src/ps/cff/font.rs::new",
    ],
    "font metric accessors share a shape across CFF and TrueType sources and across advance and side-bearing.",
  ),
  (
    &[
      "crates/console-core-audio-decoding-interfaces/src/audio.rs::copy_interleaved_ref",
      "crates/console-core-audio-decoding-matroska-container/src/demuxer.rs::read_block_group",
      "crates/console-core-audio-decoding-matroska-container/src/demuxer.rs::read_simple_block",
      "crates/console-core-audio-decoding-ogg-container/src/mappings/mod.rs::codec_params",
      "crates/console-core-audio-decoding-ogg-container/src/mappings/mod.rs::codec_params_mut",
      "crates/console-core-audio-decoding-uncompressed/tests/the_pcm.rs::shown_ref",
      "crates/console-core-audio-decoding-vorbis/src/reading.rs::span",
      "crates/console-core-audio-decoding-vorbis/src/reading.rs::span_mut",
      "crates/console-core-audio-decoding-vorbis/src/setup.rs::read_floors",
      "crates/console-core-audio-decoding-vorbis/src/setup.rs::read_residues",
      "crates/console-core-audio-decoding-wave-container/tests/the_aiffs.rs::chunk",
      "crates/console-core-audio-decoding-wave-container/tests/the_waves.rs::chunk",
      "crates/console-core-controls/src/picker.rs::lines",
      "crates/console-core-controls/src/picker.rs::texts",
      "crates/console-core-font-reading/src/directory.rs::dsig_length_byte_range",
      "crates/console-core-font-reading/src/directory.rs::dsig_offset_byte_range",
      "crates/console-core-font-reading/src/directory.rs::dsig_tag_byte_range",
      "crates/console-core-font-reading/src/font_data.rs::read_computed_array",
      "crates/console-core-font-reading/src/font_data.rs::read_with_args",
      "crates/console-core-font-reading/src/model/font/instance.rs::set_named_instance",
      "crates/console-core-font-reading/src/model/font/instance.rs::set_named_instance_with_overrides",
      "crates/console-core-font-reading/src/model/font/instance.rs::set_variations",
      "crates/console-core-font-reading/src/offset_array.rs::get",
      "crates/console-core-font-reading/src/ps/cff/font.rs::draw",
      "crates/console-core-font-reading/src/ps/cff/font.rs::subfont",
      "crates/console-core-font-reading/src/ps/cff/font.rs::subfont_hinted",
      "crates/console-core-font-reading/src/ps/cff/index.rs::get",
      "crates/console-core-font-reading/src/ps/cff/index.rs::size_in_bytes",
      "crates/console-core-font-reading/src/ps/type1.rs::draw",
      "crates/console-core-font-reading/src/tables/aat.rs::entries",
      "crates/console-core-font-reading/src/tables/aat.rs::segments",
      "crates/console-core-font-reading/src/tables/bitmap.rs::image_data_offset",
      "crates/console-core-font-reading/src/tables/bitmap.rs::image_format",
      "crates/console-core-font-reading/src/tables/bitmap.rs::index_format",
      "crates/console-core-font-reading/src/tables/cmap.rs::find_cmap12",
      "crates/console-core-font-reading/src/tables/cmap.rs::find_cmap14",
      "crates/console-core-font-reading/src/tables/cmap.rs::find_cmap4",
      "crates/console-core-font-reading/src/tables/colr/closure.rs::v0_closure_glyphs",
      "crates/console-core-font-reading/src/tables/colr/closure.rs::v0_closure_palette_indices",
      "crates/console-core-font-reading/src/tables/glyf.rs::component_glyphs_and_flags",
      "crates/console-core-font-reading/src/tables/glyf.rs::components",
      "crates/console-core-font-reading/src/tables/gpos/closure.rs::closure_lookups",
      "crates/console-core-font-reading/src/tables/gpos/closure.rs::collect_features",
      "crates/console-core-font-reading/src/tables/gpos/closure.rs::collect_lookups",
      "crates/console-core-font-reading/src/tables/gsub/closure.rs::closure_lookups",
      "crates/console-core-font-reading/src/tables/gsub/closure.rs::collect_features",
      "crates/console-core-font-reading/src/tables/gsub/closure.rs::collect_lookups",
      "crates/console-core-font-reading/src/tables/layout.rs::population",
      "crates/console-core-font-reading/src/tables/layout/closure.rs::coverage",
      "crates/console-core-font-reading/src/tables/layout/closure.rs::intersects",
      "crates/console-core-font-reading/src/tables/layout/feature.rs::get",
      "crates/console-core-font-reading/src/tables/layout/script.rs::get",
      "crates/console-core-font-reading/src/tables/layout/script.rs::index_for_tag",
      "crates/console-core-font-reading/src/tables/layout/script.rs::lang_sys_index_for_tag",
      "crates/console-core-font-reading/src/tables/stat.rs::axis_index",
      "crates/console-core-font-reading/src/tables/stat.rs::value",
      "crates/console-core-font-reading/src/tables/trak.rs::per_size_values",
      "crates/console-core-font-reading/src/tables/trak.rs::size_table",
      "crates/console-core-font-reading/src/tables/variations.rs::active_tuples_at",
      "crates/console-core-font-reading/src/tables/variations.rs::intermediate_end_tuple",
      "crates/console-core-font-reading/src/tables/variations.rs::intermediate_start_tuple",
      "crates/console-core-font-reading/src/tables/variations.rs::tuples",
      "crates/console-core-font-reading/src/traversal.rs::offset",
      "crates/console-core-font-reading/src/traversal.rs::offset_to_array_of_scalars",
      "crates/console-core-jpeg-2000-files/src/j2c/progression.rs::component_position_resolution_layer_progression",
      "crates/console-core-jpeg-2000-files/src/j2c/progression.rs::layer_resolution_component_position_progression",
      "crates/console-core-jpeg-2000-files/src/j2c/progression.rs::position_component_resolution_layer_progression",
      "crates/console-core-jpeg-2000-files/src/j2c/progression.rs::resolution_layer_component_position_progression",
      "crates/console-core-jpeg-2000-files/src/j2c/progression.rs::resolution_position_component_layer_progression",
      "crates/console-core-jpeg-2000-files/src/j2c/tag_tree.rs::top_left_height",
      "crates/console-core-jpeg-2000-files/src/j2c/tag_tree.rs::top_left_width",
      "crates/console-core-jpeg-2000-files/src/j2c/tile.rs::code_block_height",
      "crates/console-core-jpeg-2000-files/src/j2c/tile.rs::code_block_width",
      "crates/console-core-jpeg-2000-files/src/j2c/tile.rs::num_precincts_x",
      "crates/console-core-jpeg-2000-files/src/j2c/tile.rs::num_precincts_y",
      "crates/console-core-jpeg-2000-files/src/math.rs::floor",
      "crates/console-core-jpeg-2000-files/src/math.rs::max",
      "crates/console-core-jpeg-2000-files/src/math.rs::min",
      "crates/console-core-jpeg-2000-files/src/math.rs::mul_add",
      "crates/console-core-jpeg-2000-files/src/reader.rs::read_byte",
      "crates/console-core-jpeg-2000-files/src/reader.rs::read_bytes",
      "crates/console-core-path-rasterizer/src/color.rs::to_color_u8",
      "crates/console-core-path-rasterizer/src/edge.rs::new",
      "crates/console-core-path-rasterizer/src/line_clipper.rs::contains_no_empty_check",
      "crates/console-core-path-rasterizer/src/line_clipper.rs::pin_unsorted_f32",
      "crates/console-core-path-rasterizer/src/line_clipper.rs::pin_unsorted_f64",
      "crates/console-core-path-rasterizer/src/mask.rs::fill_path",
      "crates/console-core-path-rasterizer/src/mask.rs::submask",
      "crates/console-core-path-rasterizer/src/mask.rs::subpixmap",
      "crates/console-core-path-rasterizer/src/math.rs::left_shift",
      "crates/console-core-path-rasterizer/src/math.rs::left_shift64",
      "crates/console-core-path-rasterizer/src/painter.rs::fill_path",
      "crates/console-core-path-rasterizer/src/path64/line_cubic_intersections.rs::horizontal_intersect",
      "crates/console-core-path-rasterizer/src/path64/line_cubic_intersections.rs::vertical_intersect",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::color_k",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::gamma_compress_22",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::gamma_compress_srgb",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::gamma_expand_22",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::gamma_expand_srgb",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::hue_k",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::lerp_1_float",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::load_8888_tail",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::load_dst",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::load_dst_tail",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::luminosity_k",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::mask_2pt_conical_degenerates",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::mask_2pt_conical_nan",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::next_stage",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::reflect",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::repeat",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::sampler_2x2",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::sampler_4x4",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::saturation_k",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::source_over_rgba",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::source_over_rgba_tail",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::start",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::store",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::store_tail",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::xy_to_2pt_conical_greater",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::xy_to_2pt_conical_smaller",
      "crates/console-core-path-rasterizer/src/pipeline/highp.rs::xy_to_2pt_conical_well_behaved",
      "crates/console-core-path-rasterizer/src/pipeline/lowp.rs::lerp_1_float",
      "crates/console-core-path-rasterizer/src/pipeline/lowp.rs::load_8888_tail",
      "crates/console-core-path-rasterizer/src/pipeline/lowp.rs::load_dst",
      "crates/console-core-path-rasterizer/src/pipeline/lowp.rs::load_dst_tail",
      "crates/console-core-path-rasterizer/src/pipeline/lowp.rs::next_stage",
      "crates/console-core-path-rasterizer/src/pipeline/lowp.rs::pad_x1",
      "crates/console-core-path-rasterizer/src/pipeline/lowp.rs::repeat_x1",
      "crates/console-core-path-rasterizer/src/pipeline/lowp.rs::source_over_rgba",
      "crates/console-core-path-rasterizer/src/pipeline/lowp.rs::source_over_rgba_tail",
      "crates/console-core-path-rasterizer/src/pipeline/lowp.rs::start",
      "crates/console-core-path-rasterizer/src/pipeline/lowp.rs::store",
      "crates/console-core-path-rasterizer/src/pipeline/lowp.rs::store_tail",
      "crates/console-core-path-rasterizer/src/pixmap.rs::from_bytes",
      "crates/console-core-path-rasterizer/src/pixmap.rs::pixel",
      "crates/console-core-path-rasterizer/src/pixmap.rs::subpixmap",
      "crates/console-core-path-rasterizer/src/scan/hairline.rs::compute_nocheck_cubic_bounds",
      "crates/console-core-path-rasterizer/src/scan/hairline.rs::compute_nocheck_quad_bounds",
      "crates/console-core-path-rasterizer/src/scan/hairline.rs::geometric_contains",
      "crates/console-core-path-rasterizer/src/wide/f32x8_t.rs::blend",
      "crates/console-core-path-rasterizer/src/wide/f32x8_t.rs::cmp_eq",
      "crates/console-core-path-rasterizer/src/wide/f32x8_t.rs::cmp_ge",
      "crates/console-core-path-rasterizer/src/wide/f32x8_t.rs::cmp_gt",
      "crates/console-core-path-rasterizer/src/wide/f32x8_t.rs::cmp_le",
      "crates/console-core-path-rasterizer/src/wide/f32x8_t.rs::cmp_lt",
      "crates/console-core-path-rasterizer/src/wide/f32x8_t.rs::cmp_ne",
      "crates/console-core-path-rasterizer/src/wide/i32x8_t.rs::blend",
      "crates/console-core-path-rasterizer/src/wide/i32x8_t.rs::cmp_eq",
      "crates/console-core-path-rasterizer/src/wide/i32x8_t.rs::cmp_gt",
      "crates/console-core-path-rasterizer/src/wide/u32x8_t.rs::shl",
      "crates/console-core-path-rasterizer/src/wide/u32x8_t.rs::shr",
      "crates/console-core-paths/src/f32x2_t.rs::max",
      "crates/console-core-paths/src/f32x2_t.rs::min",
      "crates/console-core-paths/src/path_builder.rs::cubic_to",
      "crates/console-core-paths/src/path_builder.rs::quad_to",
      "crates/console-core-paths/src/path_geometry.rs::eval_cubic_pos_at",
      "crates/console-core-paths/src/path_geometry.rs::eval_quad_at",
      "crates/console-core-paths/src/rect.rs::bbox_transform",
      "crates/console-core-paths/src/rect.rs::from_ltrb",
      "crates/console-core-paths/src/rect.rs::round",
      "crates/console-core-paths/src/rect.rs::round_out",
      "crates/console-core-paths/src/transform.rs::is_skew",
      "crates/console-core-paths/src/transform.rs::is_translate",
      "crates/console-core-paths/src/transform.rs::map_point",
      "crates/console-core-paths/src/transform.rs::map_points",
      "crates/console-core-portable-document-interpreter/src/color.rs::from_rgb8",
      "crates/console-core-portable-document-interpreter/src/color.rs::from_rgba8",
      "crates/console-core-portable-document-interpreter/src/font/standard_font.rs::as_str",
      "crates/console-core-portable-document-interpreter/src/font/standard_font.rs::postscript_name",
      "crates/console-core-portable-document-interpreter/src/font/true_type.rs::map_code",
      "crates/console-core-portable-document-interpreter/src/font/true_type.rs::outline_glyph",
      "crates/console-core-portable-document-interpreter/src/font/type1.rs::glyph_width",
      "crates/console-core-portable-document-interpreter/src/font/type1.rs::map_code",
      "crates/console-core-portable-document-interpreter/src/font/type1.rs::outline_glyph",
      "crates/console-core-portable-document-interpreter/src/shading.rs::interpolate",
      "crates/console-core-portable-document-interpreter/src/shading.rs::to_triangles",
      "crates/console-core-portable-document-syntax/src/bit_reader.rs::align",
      "crates/console-core-portable-document-syntax/src/content/mod.rs::empty",
      "crates/console-core-portable-document-syntax/src/content/mod.rs::new",
      "crates/console-core-portable-document-syntax/src/crypto/aes.rs::aes_128_block_test",
      "crates/console-core-portable-document-syntax/src/crypto/aes.rs::aes_128_cbc_test",
      "crates/console-core-portable-document-syntax/src/crypto/aes.rs::aes_256_block_test",
      "crates/console-core-portable-document-syntax/src/crypto/aes.rs::aes_256_cbc_test",
      "crates/console-core-portable-document-syntax/src/crypto/aes.rs::inv_shift_rows",
      "crates/console-core-portable-document-syntax/src/crypto/aes.rs::shift_rows",
      "crates/console-core-portable-document-syntax/src/object/array.rs::array_impl",
      "crates/console-core-portable-document-syntax/src/object/array.rs::array_ref_impl",
      "crates/console-core-portable-document-syntax/src/object/dict.rs::get_raw",
      "crates/console-core-portable-document-syntax/src/object/dict.rs::get_ref",
      "crates/console-core-portable-document-syntax/src/page.rs::get_color_space",
      "crates/console-core-portable-document-syntax/src/page.rs::get_ext_g_state",
      "crates/console-core-portable-document-syntax/src/page.rs::get_font",
      "crates/console-core-portable-document-syntax/src/page.rs::get_pattern",
      "crates/console-core-portable-document-syntax/src/page.rs::get_shading",
      "crates/console-core-portable-document-text/src/lib.rs::from_upright",
      "crates/console-core-portable-document-text/src/lib.rs::to_upright",
      "crates/console-core-postscript-tokens/src/lib.rs::parse_name",
      "crates/console-core-postscript-tokens/src/lib.rs::parse_number",
      "crates/console-core-postscript-tokens/src/lib.rs::parse_string",
      "crates/console-core-scalable-vector-graphics-rasterizer/src/filter/box_blur.rs::box_blur_horz",
      "crates/console-core-scalable-vector-graphics-rasterizer/src/filter/box_blur.rs::box_blur_vert",
      "crates/console-core-scalable-vector-graphics-rasterizer/src/filter/mod.rs::apply_diffuse_lighting",
      "crates/console-core-scalable-vector-graphics-rasterizer/src/filter/mod.rs::apply_specular_lighting",
      "crates/console-core-scalable-vector-graphics-values/src/filter_functions.rs::parse_filter_length",
      "crates/console-core-scalable-vector-graphics-values/src/filter_functions.rs::parse_filter_positive_length",
      "crates/console-core-scalable-vector-graphics-values/src/font.rs::parse_font_families",
      "crates/console-core-scalable-vector-graphics-values/src/funciri.rs::from_str",
      "crates/console-core-scalable-vector-graphics-values/src/length.rs::parse_list_length",
      "crates/console-core-scalable-vector-graphics-values/src/number.rs::parse_list_number",
      "crates/console-core-scalable-vector-graphics-values/src/stream.rs::parse_list_number_or_percent",
      "crates/console-core-scalable-vector-graphics/src/parser/converter.rs::gen_clip_path_id",
      "crates/console-core-scalable-vector-graphics/src/parser/converter.rs::gen_filter_id",
      "crates/console-core-scalable-vector-graphics/src/parser/converter.rs::gen_image_id",
      "crates/console-core-scalable-vector-graphics/src/parser/converter.rs::gen_linear_gradient_id",
      "crates/console-core-scalable-vector-graphics/src/parser/converter.rs::gen_mask_id",
      "crates/console-core-scalable-vector-graphics/src/parser/converter.rs::gen_pattern_id",
      "crates/console-core-scalable-vector-graphics/src/parser/converter.rs::gen_radial_gradient_id",
      "crates/console-core-scalable-vector-graphics/src/parser/filter.rs::find_filter_with_primitives",
      "crates/console-core-scalable-vector-graphics/src/parser/paint_server.rs::find_pattern_with_children",
      "crates/console-core-scalable-vector-graphics/src/parser/paint_server.rs::process_fill",
      "crates/console-core-scalable-vector-graphics/src/parser/paint_server.rs::process_stroke",
      "crates/console-core-scalable-vector-graphics/src/parser/paint_server.rs::resolve_filter_attr",
      "crates/console-core-scalable-vector-graphics/src/parser/paint_server.rs::resolve_lg_attr",
      "crates/console-core-scalable-vector-graphics/src/parser/paint_server.rs::resolve_pattern_attr",
      "crates/console-core-scalable-vector-graphics/src/parser/paint_server.rs::resolve_rg_attr",
      "crates/console-core-scalable-vector-graphics/src/parser/shapes.rs::convert_circle",
      "crates/console-core-scalable-vector-graphics/src/parser/shapes.rs::convert_ellipse",
      "crates/console-core-scalable-vector-graphics/src/parser/shapes.rs::convert_polygon",
      "crates/console-core-scalable-vector-graphics/src/parser/shapes.rs::convert_polyline",
      "crates/console-core-scalable-vector-graphics/src/text/colr.rs::paint_linear_gradient",
      "crates/console-core-scalable-vector-graphics/src/text/colr.rs::paint_radial_gradient",
      "crates/console-core-scalable-vector-graphics/src/tree/geom.rs::to_non_zero_rect",
      "crates/console-core-scalable-vector-graphics/src/tree/geom.rs::to_rect",
      "crates/console-core-scalable-vector-graphics/src/tree/mod.rs::abs_bounding_box",
      "crates/console-core-scalable-vector-graphics/src/tree/mod.rs::abs_stroke_bounding_box",
      "crates/console-core-scalable-vector-graphics/src/tree/mod.rs::abs_transform",
      "crates/console-core-scalable-vector-graphics/src/tree/mod.rs::bounding_box",
      "crates/console-core-scalable-vector-graphics/src/tree/mod.rs::collect_clip_paths",
      "crates/console-core-scalable-vector-graphics/src/tree/mod.rs::collect_masks",
      "crates/console-core-scalable-vector-graphics/src/tree/mod.rs::stroke_bounding_box",
      "crates/console-core-scalable-vector-graphics/src/tree/mod.rs::subroots",
      "crates/console-core-shapes/src/lib.rs::panels",
      "crates/console-core-shapes/src/lib.rs::texts",
      "crates/console-core-text-shaping/src/hb/aat/layout_kerx_table.rs::apply_state_machine_kerning",
      "crates/console-core-text-shaping/src/hb/aat/layout_kerx_table.rs::collect_initial_glyphs",
      "crates/console-core-text-shaping/src/hb/aat/layout_kerx_table.rs::collect_start_end_safe_to_break",
      "crates/console-core-text-shaping/src/hb/aat/layout_morx_table.rs::collect_initial_glyphs",
      "crates/console-core-text-shaping/src/hb/aat/layout_morx_table.rs::collect_start_end_safe_to_break",
      "crates/console-core-text-shaping/src/hb/buffer.rs::merge_clusters",
      "crates/console-core-text-shaping/src/hb/buffer.rs::merge_grapheme_clusters",
      "crates/console-core-text-shaping/src/hb/buffer.rs::merge_out_clusters",
      "crates/console-core-text-shaping/src/hb/buffer.rs::merge_out_grapheme_clusters",
      "crates/console-core-text-shaping/src/hb/kerning.rs::apply_state_machine_kerning",
      "crates/console-core-text-shaping/src/hb/ot_layout.rs::set_modified_combining_class",
      "crates/console-core-text-shaping/src/hb/ot_layout.rs::set_unicode_space_fallback_type",
      "crates/console-core-text-shaping/src/hb/ot_shaper_indic.rs::setup_syllables",
      "crates/console-core-text-shaping/src/hb/ot_shaper_indic_machine.rs::found_syllable",
      "crates/console-core-text-shaping/src/hb/ot_shaper_khmer.rs::setup_syllables",
      "crates/console-core-text-shaping/src/hb/ot_shaper_khmer_machine.rs::found_syllable",
      "crates/console-core-text-shaping/src/hb/ot_shaper_myanmar.rs::setup_syllables",
      "crates/console-core-text-shaping/src/hb/ot_shaper_myanmar_machine.rs::found_syllable",
      "crates/console-core-text-shaping/src/hb/text_parser.rs::consume_f32",
      "crates/console-core-text-shaping/src/hb/text_parser.rs::consume_i32",
    ],
    "audio container, control, shape, and codec helper functions repeat the same read/write pattern per format and per codec.",
  ),
];

const NOT_YET: [&[&str]; 4] = [
  &[
    "crates/console-core-places/src/lib.rs::env_var",
    "crates/console-test-desktop/src/lib.rs::env_var",
  ],
  &[
    "crates/console-device/src/bin/console-deploy.rs::today",
    "crates/console-input-dictation/src/bin/voice-compare.rs::stamped",
  ],
  &[
    "crates/console-input-controller/src/reading.rs::path_from_environment",
    "crates/console-panel/src/description.rs::where_to",
  ],
  &[
    "crates/console-notifications/src/serving.rs::inbox_path",
    "crates/console-notifications/src/updating.rs::at",
  ],
];

const KEYWORDS: [&str; 30] = [
  "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "false",
  "fn", "for", "if", "impl", "in", "let", "loop", "match", "move", "mut", "pub", "ref", "return",
  "self", "static", "struct", "super", "true", "while",
];

type Pair = (String, String, f64);

fn root() -> Result<PathBuf, Never> {
  let crate_directory = Path::new(env!("CARGO_MANIFEST_DIR"));
  let workspace = match crate_directory.ancestors().nth(2) {
    Some(parent) => parent,
    None => crate_directory,
  };

  match workspace.canonicalize() {
    Ok(found) => Ok(found),
    Err(_not_there) => Ok(workspace.to_path_buf()),
  }
}

struct Written {
  named: String,
  shape: BTreeSet<u64>,
}

#[derive(Clone, Copy)]
enum Closing {
  End,
  Top,
}

struct Level {
  trees: Vec<TokenTree>,
  at: u32,
  closing: Closing,
}

#[derive(Clone, Copy)]
enum Meaning {
  Named,
  AnyName,
}

fn meaning(
  said: &str,
  (before, after): (Option<&TokenTree>, Option<&TokenTree>),
) -> Result<Meaning, Never> {
  let called = after.is_some_and(|after_2| match after_2 {
    TokenTree::Group(group) => group.delimiter() == Delimiter::Parenthesis,
    TokenTree::Punct(punct) => punct.as_char() == ':' || punct.as_char() == '!',
    TokenTree::Ident(_) | TokenTree::Literal(_) => false,
  });

  let reached = before.is_some_and(|before_2| match before_2 {
    TokenTree::Punct(punct) => punct.as_char() == '.' || punct.as_char() == ':',
    TokenTree::Group(_) | TokenTree::Ident(_) | TokenTree::Literal(_) => false,
  });

  let meant = KEYWORDS.contains(&said)
    || said.starts_with(|first: char| first.is_uppercase())
    || called
    || reached;

  Ok(match ::core::primitive::bool::then_some(meant, ()) {
    ::core::option::Option::Some(()) => Meaning::Named,
    ::core::option::Option::None => Meaning::AnyName,
  })
}

fn flatten(stream: TokenStream) -> Result<Vec<String>, Never> {
  let walking = vec![Level {
    trees: stream.into_iter().collect(),
    at: 0,
    closing: Closing::Top,
  }];
  let flattened =
    console_core_iteration::iterate((Vec::new(), walking), |(mut into, mut walking_2)| {
      let top = match walking_2.last_mut() {
        Some(top) => top,
        None => return Ok(Step::Halt(into)),
      };

      let Ok(at) = index(top.at);
      let tree = top.trees.get(at).cloned();
      let before = at
        .checked_sub(1)
        .and_then(|before| top.trees.get(before))
        .cloned();
      let after = top.trees.get(at.saturating_add(1)).cloned();

      top.at = top.at.saturating_add(1);

      match tree {
        None => {
          let closing = top.closing;

          walking_2.pop();

          match closing {
            Closing::End => into.push("end".to_string()),
            Closing::Top => {}
          }
        }
        Some(TokenTree::Group(group)) => {
          into.push(format!("{:?}", group.delimiter()));
          walking_2.push(Level {
            trees: group.stream().into_iter().collect(),
            at: 0,
            closing: Closing::End,
          });
        }
        Some(TokenTree::Punct(punct)) => into.push(punct.as_char().to_string()),
        Some(TokenTree::Literal(literal)) => {
          let said = literal.to_string();

          match ::core::primitive::bool::then_some(
            said.starts_with('"') || said.starts_with('r'),
            (),
          ) {
            ::core::option::Option::Some(()) => into.push("\"\"".to_string()),
            ::core::option::Option::None => into.push(said),
          }
        }
        Some(TokenTree::Ident(ident)) => {
          let said = ident.to_string();
          let Ok(meant) = meaning(&said, (before.as_ref(), after.as_ref()));

          match meant {
            Meaning::Named => into.push(said),
            Meaning::AnyName => into.push("_".to_string()),
          }
        }
      }

      Ok(Step::Again((into, walking_2)))
    });

  Ok(match flattened {
    Ok(into) => into,
    Err(_endless) => Vec::new(),
  })
}

fn shape(body: &syn::Block) -> Result<Option<BTreeSet<u64>>, Never> {
  let Ok(tokens) = flatten(body.to_token_stream());
  let Ok(long) = fitted::<_, u32>(tokens.len());
  let Ok(run) = index(RUN);

  Ok(
    match ::core::primitive::bool::then_some(long < SHORTEST, ()) {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => Some(
        tokens
          .windows(run.max(1))
          .map(|run_2| {
            let mut hasher = DefaultHasher::new();
            run_2.hash(&mut hasher);
            hasher.finish()
          })
          .collect(),
      ),
    },
  )
}

#[derive(Clone, Copy)]
enum Caller {
  ByATest,
  ByTheTree,
}

fn asked_by(attributes: &[syn::Attribute]) -> Result<Caller, Never> {
  Ok(
    match ::core::primitive::bool::then_some(
      attributes
        .iter()
        .any(|attribute| attribute.path().is_ident("test")),
      (),
    ) {
      ::core::option::Option::Some(()) => Caller::ByATest,
      ::core::option::Option::None => Caller::ByTheTree,
    },
  )
}

struct Reading<'a> {
  file: &'a str,
  found: &'a mut Vec<Written>,
}

impl Reading<'_> {
  fn keep(
    &mut self,
    (ident, attributes, body): (&syn::Ident, &[syn::Attribute], &syn::Block),
  ) -> Result<(), Never> {
    let Ok(line) = fitted::<_, u32>(ident.span().start().line);
    let Ok(asked) = asked_by(attributes);
    let Ok(shape) = shape(body);

    match (asked, shape) {
      (Caller::ByTheTree, Some(shape_2)) => self.found.push(Written {
        named: format!("{}::{ident}:{line}", self.file),
        shape: shape_2,
      }),
      (Caller::ByATest, _) | (Caller::ByTheTree, None) => {}
    }

    Ok(())
  }
}

impl<'ast> Visit<'ast> for Reading<'_> {
  fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
    let Ok(()) = self.keep((&item.sig.ident, &item.attrs, &item.block));

    syn::visit::visit_item_fn(self, item);
  }

  fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
    match item.trait_ {
      Some(_) => {}
      None => syn::visit::visit_item_impl(self, item),
    }
  }

  fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
    let Ok(()) = self.keep((&item.sig.ident, &item.attrs, &item.block));

    syn::visit::visit_impl_item_fn(self, item);
  }
}

fn read(top: &Path, files: &[PathBuf]) -> Result<Vec<Written>, Never> {
  let mut found = Vec::new();

  for at in files {
    let said = match std::fs::read_to_string(at) {
      Ok(said) => said,
      Err(_unreadable) => continue,
    };
    let parsed = match syn::parse_file(&said) {
      Ok(parsed) => parsed,
      Err(_not_rust_syn_reads) => continue,
    };
    let file = match at.strip_prefix(top) {
      Ok(inside) => inside.display().to_string(),
      Err(_outside_the_tree) => at.display().to_string(),
    };

    Reading {
      file: &file,
      found: &mut found,
    }
    .visit_file(&parsed);
  }

  Ok(found)
}

fn scan_sources() -> Result<Vec<Written>, Never> {
  let Ok(top) = root();
  let Ok(files) = console_repository::sources::under(&top.join("crates"));
  let Ok(hands) =
    fitted::<_, u32>(std::thread::available_parallelism().map_or(1, |hands| hands.get()));
  let Ok(many) = fitted::<_, u32>(files.len());
  let Ok(each) = index(many.div_ceil(hands.max(1)).max(1));
  let shares = files.chunks(each.max(1));
  let mut batches = Vec::new();
  batches.resize_with(shares.len().min(files.len()), Vec::new);

  let (top_2, output) = (&top, &mut batches);

  std::thread::scope(move |scope| {
    for (share, written) in shares.zip(output) {
      #[cfg_attr(
        dylint_lib = "explicit103_no_thread_per_item",
        expect(
          explicit103_no_thread_per_item,
          reason = "this fixture exercises isolated simultaneous repository readers, each with a statically limited share of work"
        )
      )]
      scope.spawn(move || {
        let Ok(read) = read(top_2, share);

        *written = read;
      });
    }
  });

  Ok(batches.into_iter().flatten().collect())
}

fn similar_pairs() -> Result<Vec<Pair>, Never> {
  let Ok(every) = scan_sources();
  let mut holding: BTreeMap<u64, Vec<u32>> = BTreeMap::new();

  for (one, at) in every.iter().zip(0u32..) {
    for run in &one.shape {
      holding.entry(*run).or_default().push(at);
    }
  }

  let mut met: HashMap<(u32, u32), u32> = HashMap::new();

  for holders in holding.values() {
    for (at, one) in holders.iter().enumerate() {
      for other in holders.iter().skip(at.saturating_add(1)) {
        let counted = met.entry((*one, *other)).or_default();
        *counted = counted.saturating_add(1);
      }
    }
  }

  let mut alike: Vec<Pair> = met
    .into_iter()
    .filter(|(_, shared)| *shared >= MET)
    .filter_map(|((one, other), _)| {
      let Ok(one_2) = index(one);
      let Ok(other_2) = index(other);
      let (one_3, other_3) = every.get(one_2).zip(every.get(other_2))?;
      let Ok(shared) = fitted::<_, u32>(one_3.shape.intersection(&other_3.shape).count());
      let Ok(first) = fitted::<_, u32>(one_3.shape.len());
      let Ok(second) = fitted::<_, u32>(other_3.shape.len());
      let either = first.saturating_add(second).saturating_sub(shared).max(1);
      let alike = f64::from(shared) / f64::from(either);

      Some((one_3.named.clone(), other_3.named.clone(), alike))
    })
    .filter(|(_, _, alike)| *alike >= ALIKE_ENOUGH)
    .collect();

  alike.sort_by(|one, other| other.2.total_cmp(&one.2).then_with(|| one.0.cmp(&other.0)));

  Ok(alike)
}

fn without_line(named: &str) -> Result<&str, Never> {
  Ok(
    named
      .rsplit_once(':')
      .map_or(named, |(named_2, _line)| named_2),
  )
}

fn groups(pairs: &[Pair]) -> Result<Vec<BTreeSet<String>>, Never> {
  let mut held: Vec<BTreeSet<String>> = Vec::new();

  for (one, other, _) in pairs {
    let (touching, apart): (Vec<BTreeSet<String>>, Vec<BTreeSet<String>>) = held
      .into_iter()
      .partition(|group| group.contains(one) || group.contains(other));

    let mut joined: BTreeSet<String> = touching.into_iter().flatten().collect();
    joined.insert(one.clone());
    joined.insert(other.clone());

    held = apart;
    held.push(joined);
  }

  held.sort();

  Ok(held)
}

fn let_through() -> Result<impl Iterator<Item = &'static [&'static str]>, Never> {
  Ok(
    ALIKE
      .iter()
      .map(|(group, _why)| *group)
      .chain(NOT_YET.iter().copied()),
  )
}

fn allowed() -> Result<BTreeSet<(&'static str, &'static str)>, Never> {
  let Ok(every) = let_through();

  Ok(
    every
      .flat_map(|group| {
        group
          .iter()
          .flat_map(move |one| group.iter().map(move |other| (*one, *other)))
      })
      .collect(),
  )
}

#[test]
fn no_function_is_written_twice() {
  let Ok(pairs) = similar_pairs();
  let Ok(allowed) = allowed();

  let twice: Vec<Pair> = pairs
    .into_iter()
    .filter(|(one, other, _)| {
      let (Ok(one_2), Ok(other_2)) = (without_line(one), without_line(other));

      !allowed.contains(&(one_2, other_2))
    })
    .collect();

  let Ok(grouped) = groups(&twice);

  let said: Vec<String> = grouped
    .into_iter()
    .map(|group| group.into_iter().collect::<Vec<String>>().join("\n    "))
    .collect();

  assert!(
    said.is_empty(),
    "a function written again is one of them guessing; call the other, or put the group on ALIKE with why:\n\n    {}",
    said.join("\n\n    ")
  );
}

#[test]
fn every_group_let_stay_is_still_alike() {
  let Ok(pairs) = similar_pairs();
  let Ok(allowed) = allowed();
  let Ok(every) = let_through();

  let met: BTreeSet<&str> = pairs
    .iter()
    .filter_map(|(one, other, _)| {
      let (Ok(one_2), Ok(other_2)) = (without_line(one), without_line(other));

      match ::core::primitive::bool::then_some(allowed.contains(&(one_2, other_2)), ()) {
        ::core::option::Option::Some(()) => Some([one_2, other_2]),
        ::core::option::Option::None => None,
      }
    })
    .flatten()
    .collect();

  let stale: Vec<String> = every
    .flatten()
    .filter(|member| !met.contains(**member))
    .map(|member| member.to_string())
    .collect();

  assert!(
    stale.is_empty(),
    "a function no longer alike the rest of its group is a line to delete rather than a permission to keep: {stale:?}"
  );
}
