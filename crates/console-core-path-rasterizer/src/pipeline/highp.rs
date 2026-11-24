/*!
A high precision raster pipeline implementation.

Unlike lowp, this one implements all stages.

Just like Skia, this pipeline is implemented using f32x8.

Bilinear sampling computes the weight fraction and the two tap positions with
separate half-pixel offsets to preserve their rounding.

For some reason, we are almost 2x slower. Maybe because Skia uses clang's vector extensions
and we're using a manual implementation.
*/

use console_core_never::Never;
use console_core_paths::StrictNum;
use console_core_number_conversion::{nearest_f32_from_u64, wrapped};
use super::TileCtx;
use crate::{PremultipliedColorU8, SpreadMode, PixmapRef};

use crate::geom::{PixelPosition, ScreenIntRect};
use crate::math::GammaExponent;
use crate::pixmap::SubPixmapMut;
use crate::wide::{f32x8, i32x8, u32x8};

const STAGE_WIDTH: u32 = 8;
pub type Lanes<T> = [T; 8];

pub type StageFn = fn(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never>;

pub struct Pipeline<'a, 'b: 'a> {
    functions: &'a [StageFn],
    pixmap_src: PixmapRef<'a>,
    pixmap_dst: &'a mut SubPixmapMut<'b>,
    ctx: &'a mut super::Context,
    mask_ctx: super::MaskCtx<'a>,
    aa_mask_ctx: super::AAMaskCtx,
    r: f32x8,
    g: f32x8,
    b: f32x8,
    a: f32x8,
    dr: f32x8,
    dg: f32x8,
    db: f32x8,
    da: f32x8,
    tail: u32,
    position: PixelPosition,
}

impl Pipeline<'_, '_> {
    #[inline(always)]
    fn next_stage(&mut self) -> core::result::Result<(), console_core_never::Never> {
        let remaining = self.functions;

        match remaining.split_first() {
            Some((call, later)) => {
                self.functions = later;
                call(self)
            }
            None => Ok(()),
        }
    }
}

pub(super) fn stage_function(stage: super::Stage) -> Result<StageFn, console_core_never::Never> {
    use super::Stage;

    Ok(match stage {
        Stage::MoveSourceToDestination => move_source_to_destination,
        Stage::MoveDestinationToSource => move_destination_to_source,
        Stage::Clamp0 => clamp_0,
        Stage::ClampA => clamp_a,
        Stage::Premultiply => premultiply,
        Stage::UniformColor => uniform_color,
        Stage::SeedShader => seed_shader,
        Stage::LoadDestination => load_dst,
        Stage::Store => store,
        Stage::LoadDestinationU8 => load_dst_u8,
        Stage::StoreU8 => store_u8,
        Stage::Gather => gather,
        Stage::LoadMaskU8 => load_mask_u8,
        Stage::MaskU8 => mask_u8,
        Stage::ScaleU8 => scale_u8,
        Stage::LerpU8 => lerp_u8,
        Stage::Scale1Float => scale_1_float,
        Stage::Lerp1Float => lerp_1_float,
        Stage::DestinationAtop => destination_atop,
        Stage::DestinationIn => destination_in,
        Stage::DestinationOut => destination_out,
        Stage::DestinationOver => destination_over,
        Stage::SourceAtop => source_atop,
        Stage::SourceIn => source_in,
        Stage::SourceOut => source_out,
        Stage::SourceOver => source_over,
        Stage::Clear => clear,
        Stage::Modulate => modulate,
        Stage::Multiply => multiply,
        Stage::Plus => plus,
        Stage::Screen => screen,
        Stage::Xor => xor,
        Stage::ColorBurn => color_burn,
        Stage::ColorDodge => color_dodge,
        Stage::Darken => darken,
        Stage::Difference => difference,
        Stage::Exclusion => exclusion,
        Stage::HardLight => hard_light,
        Stage::Lighten => lighten,
        Stage::Overlay => overlay,
        Stage::SoftLight => soft_light,
        Stage::Hue => hue,
        Stage::Saturation => saturation,
        Stage::Color => color,
        Stage::Luminosity => luminosity,
        Stage::SourceOverRgba => source_over_rgba,
        Stage::Transform => transform,
        Stage::Reflect => reflect,
        Stage::Repeat => repeat,
        Stage::Bilinear => bilinear,
        Stage::Bicubic => bicubic,
        Stage::PadX1 => pad_x1,
        Stage::ReflectX1 => reflect_x1,
        Stage::RepeatX1 => repeat_x1,
        Stage::Gradient => gradient,
        Stage::EvenlySpaced2StopGradient => evenly_spaced_2_stop_gradient,
        Stage::XYToUnitAngle => xy_to_unit_angle,
        Stage::XYToRadius => xy_to_radius,
        Stage::XYTo2PtConicalFocalOnCircle => xy_to_2pt_conical_focal_on_circle,
        Stage::XYTo2PtConicalWellBehaved => xy_to_2pt_conical_well_behaved,
        Stage::XYTo2PtConicalSmaller => xy_to_2pt_conical_smaller,
        Stage::XYTo2PtConicalGreater => xy_to_2pt_conical_greater,
        Stage::XYTo2PtConicalStrip => xy_to_2pt_conical_strip,
        Stage::Mask2PtConicalNan => mask_2pt_conical_nan,
        Stage::Mask2PtConicalDegenerates => mask_2pt_conical_degenerates,
        Stage::ApplyVectorMask => apply_vector_mask,
        Stage::Alter2PtConicalCompensateFocal => alter_2pt_conical_compensate_focal,
        Stage::Alter2PtConicalUnswap => alter_2pt_conical_unswap,
        Stage::NegateX => negate_x,
        Stage::ApplyConcentricScaleBias => apply_concentric_scale_bias,
        Stage::GammaExpand2 => gamma_expand_2,
        Stage::GammaExpandDestination2 => gamma_expand_dst_2,
        Stage::GammaCompress2 => gamma_compress_2,
        Stage::GammaExpand22 => gamma_expand_22,
        Stage::GammaExpandDestination22 => gamma_expand_dst_22,
        Stage::GammaCompress22 => gamma_compress_22,
        Stage::GammaExpandSrgb => gamma_expand_srgb,
        Stage::GammaExpandDestinationSrgb => gamma_expand_dst_srgb,
        Stage::GammaCompressSrgb => gamma_compress_srgb,
    })
}

#[inline(never)]
#[expect(
  clippy::too_many_arguments,
  reason = "the stage loop is handed every table, context and pixmap it threads into the Pipeline it builds, and a struct to carry them would be that Pipeline"
)]
pub fn start(
  functions: &[StageFn],
  functions_tail: &[StageFn],
  rect: &ScreenIntRect,
  aa_mask_ctx: super::AAMaskCtx,
  mask_ctx: super::MaskCtx,
  ctx: &mut super::Context,
  pixmap_src: PixmapRef,
  pixmap_dst: &mut SubPixmapMut,
) -> core::result::Result<(), console_core_never::Never> {
    let mut p = Pipeline {
        functions: &[],
        pixmap_src,
        pixmap_dst,
        mask_ctx,
        aa_mask_ctx,
        ctx,
        r: f32x8::default(),
        g: f32x8::default(),
        b: f32x8::default(),
        a: f32x8::default(),
        dr: f32x8::default(),
        dg: f32x8::default(),
        db: f32x8::default(),
        da: f32x8::default(),
        tail: 0,
        position: PixelPosition { x: 0, y: 0 },
    };

    let Ok(top) = rect.top();
    let Ok(bottom) = rect.bottom();
    let Ok(left) = rect.left();
    let Ok(right) = rect.right();
    let tail = right.saturating_sub(left).wrapping_rem(STAGE_WIDTH);
    let last_full = right.saturating_sub(tail);

    for y in top..bottom {
        p.tail = STAGE_WIDTH;

        for x in (left..last_full).step_by(8) {
            p.position = PixelPosition { x, y };
            p.functions = functions;
            let Ok(()) = p.next_stage();
        }

        match tail {
            0 => {}
            _ => {
                p.tail = tail;
                p.position = PixelPosition { x: last_full, y };
                p.functions = functions_tail;
                let Ok(()) = p.next_stage();
            }
        }
    }

    Ok(())
}

fn move_source_to_destination(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.dr = p.r;
    p.dg = p.g;
    p.db = p.b;
    p.da = p.a;

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn premultiply(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r *= p.a;
    p.g *= p.a;
    p.b *= p.a;

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn move_destination_to_source(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = p.dr;
    p.g = p.dg;
    p.b = p.db;
    p.a = p.da;

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn clamp_0(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = { let Ok(explicit_value) = p.r.max(f32x8::default()); explicit_value };
    p.g = { let Ok(explicit_value) = p.g.max(f32x8::default()); explicit_value };
    p.b = { let Ok(explicit_value) = p.b.max(f32x8::default()); explicit_value };
    p.a = { let Ok(explicit_value) = p.a.max(f32x8::default()); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn clamp_a(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = { let Ok(explicit_value) = p.r.min({ let Ok(explicit_value) = f32x8::splat(1.0); explicit_value }); explicit_value };
    p.g = { let Ok(explicit_value) = p.g.min({ let Ok(explicit_value) = f32x8::splat(1.0); explicit_value }); explicit_value };
    p.b = { let Ok(explicit_value) = p.b.min({ let Ok(explicit_value) = f32x8::splat(1.0); explicit_value }); explicit_value };
    p.a = { let Ok(explicit_value) = p.a.min({ let Ok(explicit_value) = f32x8::splat(1.0); explicit_value }); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn uniform_color(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ctx = &p.ctx.uniform_color;
    p.r = { let Ok(explicit_value) = f32x8::splat(ctx.r); explicit_value };
    p.g = { let Ok(explicit_value) = f32x8::splat(ctx.g); explicit_value };
    p.b = { let Ok(explicit_value) = f32x8::splat(ctx.b); explicit_value };
    p.a = { let Ok(explicit_value) = f32x8::splat(ctx.a); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn seed_shader(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let iota = f32x8::from([0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5]);

    p.r = ({ let Ok(explicit_value) = f32x8::splat(match ::console_core_number_conversion::nearest_f32_from_u64(u64::from(p.position.x)) { ::core::result::Result::Ok(_converted_number) => _converted_number }); explicit_value }) + iota;
    p.g = { let Ok(explicit_value) = f32x8::splat((match ::console_core_number_conversion::nearest_f32_from_u64(u64::from(p.position.y)) { ::core::result::Result::Ok(_converted_number) => _converted_number }) + 0.5); explicit_value };
    p.b = { let Ok(explicit_value) = f32x8::splat(1.0); explicit_value };
    p.a = f32x8::default();

    p.dr = f32x8::default();
    p.dg = f32x8::default();
    p.db = f32x8::default();
    p.da = f32x8::default();

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

pub fn load_dst(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(found_pixels) = p.pixmap_dst.pixels8(p.position);
    let span = match found_pixels {
        Some(pixels) => pixels,
        None => return Ok(()),
    };

    let Ok(()) = load_8888(span, &mut p.dr, &mut p.dg, &mut p.db, &mut p.da);
    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

pub fn load_dst_tail(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    ({ let Ok(explicit_value) = load_8888_tail(p.tail, { let Ok(explicit_value) = p.pixmap_dst.pixels_from(p.position); explicit_value }, &mut p.dr, &mut p.dg, &mut p.db, &mut p.da); explicit_value });
    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

pub fn store(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(found_pixels) = p.pixmap_dst.pixels8(p.position);
    let span = match found_pixels {
        Some(pixels) => pixels,
        None => return Ok(()),
    };

    let Ok(()) = store_8888(&p.r, &p.g, &p.b, &p.a, span);
    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

pub fn store_tail(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    ({ let Ok(explicit_value) = store_8888_tail(&p.r, &p.g, &p.b, &p.a, p.tail, { let Ok(explicit_value) = p.pixmap_dst.pixels_from(p.position); explicit_value }); explicit_value });
    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

pub fn load_dst_u8(_: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    Ok(())
}

pub fn load_dst_u8_tail(_: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    Ok(())
}

pub fn store_u8(_: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    Ok(())
}

pub fn store_u8_tail(_: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    Ok(())
}

pub fn gather(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ix = { let Ok(explicit_value) = gather_ix(p.pixmap_src, p.r, p.g); explicit_value };
    ({ let Ok(explicit_value) = load_8888(&({ let Ok(explicit_value) = p.pixmap_src.gather(ix); explicit_value }), &mut p.r, &mut p.g, &mut p.b, &mut p.a); explicit_value });

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

#[inline(always)]
fn gather_ix(pixmap: PixmapRef, x: f32x8, y: f32x8) -> Result<u32x8, Never> {
    let Ok(width) = pixmap.width();
    let Ok(height) = pixmap.height();
    let Ok(columns) = nearest_f32_from_u64(u64::from(width));
    let Ok(rows) = nearest_f32_from_u64(u64::from(height));
    let Ok(column) = sample_coordinate(x, columns.to_bits());
    let Ok(row) = sample_coordinate(y, rows.to_bits());
    let Ok(width_i32) = wrapped::<_, i32>(width);
    let Ok(stride) = i32x8::splat(width_i32);

    (row * stride + column).to_u32x8_bitcast()
}

#[inline(always)]
fn sample_coordinate(value: f32x8, bound_bits: u32) -> Result<i32x8, Never> {
    let Ok(last) = f32x8::splat(f32::from_bits(bound_bits.wrapping_sub(1)));
    let Ok(nonnegative) = value.max(f32x8::default());
    let Ok(clamped) = nonnegative.min(last);
    clamped.trunc_int()
}

fn load_mask_u8(_: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    Ok(())
}

fn mask_u8(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(data) = p.mask_ctx.from_position(p.position);
    let Ok(lanes) = console_core_number_conversion::index(p.tail);
    let mut c = [0.0; 8];

    for (channel, &alpha) in c.iter_mut().zip(data).take(lanes) {
        *channel = f32::from(alpha);
    }

    let c_2 = f32x8::from(c) / ({ let Ok(explicit_value) = f32x8::splat(255.0); explicit_value });

    match ::core::primitive::bool::then_some(c_2 == f32x8::default(), ()) { ::core::option::Option::Some(()) => {
        return Ok(());
    }, ::core::option::Option::None => {} }

    p.r *= c_2;
    p.g *= c_2;
    p.b *= c_2;
    p.a *= c_2;

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn scale_u8(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {

    let data = { let Ok(explicit_value) = p.aa_mask_ctx.copy_at(p.position, p.tail); explicit_value };
    let c = f32x8::from([f32::from(match data { [element, ..] => element }), f32::from(match data { [_, element, ..] => element }), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let c_2 = c / ({ let Ok(explicit_value) = f32x8::splat(255.0); explicit_value });

    p.r *= c_2;
    p.g *= c_2;
    p.b *= c_2;
    p.a *= c_2;

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn lerp_u8(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {

    let data = { let Ok(explicit_value) = p.aa_mask_ctx.copy_at(p.position, p.tail); explicit_value };
    let c = f32x8::from([f32::from(match data { [element, ..] => element }), f32::from(match data { [_, element, ..] => element }), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let c_2 = c / ({ let Ok(explicit_value) = f32x8::splat(255.0); explicit_value });

    p.r = { let Ok(explicit_value) = lerp(p.dr, p.r, c_2); explicit_value };
    p.g = { let Ok(explicit_value) = lerp(p.dg, p.g, c_2); explicit_value };
    p.b = { let Ok(explicit_value) = lerp(p.db, p.b, c_2); explicit_value };
    p.a = { let Ok(explicit_value) = lerp(p.da, p.a, c_2); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn scale_1_float(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let c = { let Ok(explicit_value) = f32x8::splat(p.ctx.current_coverage.get()); explicit_value };
    p.r *= c;
    p.g *= c;
    p.b *= c;
    p.a *= c;

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn lerp_1_float(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let c = { let Ok(explicit_value) = f32x8::splat(p.ctx.current_coverage.get()); explicit_value };
    p.r = { let Ok(explicit_value) = lerp(p.dr, p.r, c); explicit_value };
    p.g = { let Ok(explicit_value) = lerp(p.dg, p.g, c); explicit_value };
    p.b = { let Ok(explicit_value) = lerp(p.db, p.b, c); explicit_value };
    p.a = { let Ok(explicit_value) = lerp(p.da, p.a, c); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the compositing modes share the same four lane writes and dispatch, while each testable formula is listed at its call site"
  )
)]
macro_rules! blend_fn {
    ($name:ident, $f:expr) => {
        fn $name(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
            p.r = $f(p.r, p.dr, p.a, p.da);
            p.g = $f(p.g, p.dg, p.a, p.da);
            p.b = $f(p.b, p.db, p.a, p.da);
            p.a = $f(p.a, p.da, p.a, p.da);

            let Ok(()) = p.next_stage();

            Ok(())
        }
    };
}

blend_fn!(clear,            |_, _,  _,  _| f32x8::default());
blend_fn!(source_atop,      |s, d, sa, da| s * da + d * ({ let Ok(explicit_value) = inv(sa); explicit_value }));
blend_fn!(destination_atop, |s, d, sa, da| d * sa + s * ({ let Ok(explicit_value) = inv(da); explicit_value }));
blend_fn!(source_in,        |s, _,  _, da| s * da);
blend_fn!(destination_in,   |_, d, sa,  _| d * sa);
blend_fn!(source_out,       |s, _,  _, da| s * ({ let Ok(explicit_value) = inv(da); explicit_value }));
blend_fn!(destination_out,  |_, d, sa,  _| d * ({ let Ok(explicit_value) = inv(sa); explicit_value }));
blend_fn!(source_over,      |s, d, sa,  _| { let Ok(explicit_value) = mad(d, { let Ok(explicit_value) = inv(sa); explicit_value }, s); explicit_value });
blend_fn!(destination_over, |s, d,  _, da| { let Ok(explicit_value) = mad(s, { let Ok(explicit_value) = inv(da); explicit_value }, d); explicit_value });
blend_fn!(modulate,         |s, d,  _,  _| s * d);
blend_fn!(multiply,         |s, d, sa, da| s * ({ let Ok(explicit_value) = inv(da); explicit_value }) + d * ({ let Ok(explicit_value) = inv(sa); explicit_value }) + s * d);
blend_fn!(screen,           |s, d,  _,  _| s + d - s * d);
blend_fn!(xor,              |s, d, sa, da| s * ({ let Ok(explicit_value) = inv(da); explicit_value }) + d * ({ let Ok(explicit_value) = inv(sa); explicit_value }));

blend_fn!(plus, |s: f32x8, d: f32x8, _, _| { let Ok(explicit_value) = (s + d).min({ let Ok(explicit_value) = f32x8::splat(1.0); explicit_value }); explicit_value });

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the separable modes share the alpha equation and four dispatch writes, with only their blend formula varying"
  )
)]
macro_rules! blend_fn2 {
    ($name:ident, $f:expr) => {
        fn $name(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {

            p.r = $f(p.r, p.dr, p.a, p.da);
            p.g = $f(p.g, p.dg, p.a, p.da);
            p.b = $f(p.b, p.db, p.a, p.da);
            p.a = ({ let Ok(value) = mad(p.da, ({ let Ok(value) = inv(p.a); value }), p.a); value });

            let Ok(()) = p.next_stage();

            Ok(())
        }
    };
}

blend_fn2!(darken,      |s: f32x8, d, sa, da: f32x8| s + d - ({ let Ok(explicit_value) = (s * da).max(d * sa); explicit_value }));
blend_fn2!(lighten,     |s: f32x8, d, sa, da: f32x8| s + d - ({ let Ok(explicit_value) = (s * da).min(d * sa); explicit_value }));
blend_fn2!(difference,  |s: f32x8, d, sa, da: f32x8| s + d - ({ let Ok(explicit_value) = two({ let Ok(explicit_value) = (s * da).min(d * sa); explicit_value }); explicit_value }));
blend_fn2!(exclusion,   |s: f32x8, d,  _,  _| s + d - ({ let Ok(explicit_value) = two(s * d); explicit_value }));

blend_fn2!(color_burn, |s: f32x8, d: f32x8, sa: f32x8, da: f32x8|
    { let Ok(explicit_value) = ({ let Ok(explicit_value) = d.cmp_eq(da); explicit_value }).blend(
        d + s * ({ let Ok(explicit_value) = inv(da); explicit_value }),
        { let Ok(explicit_value) = ({ let Ok(explicit_value) = s.cmp_eq(f32x8::default()); explicit_value }).blend(
            d * ({ let Ok(explicit_value) = inv(sa); explicit_value }),
            sa * (da - ({ let Ok(explicit_value) = da.min((da - d) * sa * ({ let Ok(explicit_value) = s.reciprocal(); explicit_value })); explicit_value })) + s * ({ let Ok(explicit_value) = inv(da); explicit_value }) + d * ({ let Ok(explicit_value) = inv(sa); explicit_value })
        ); explicit_value }
    ); explicit_value }
);

blend_fn2!(color_dodge, |s: f32x8, d: f32x8, sa: f32x8, da: f32x8|
    { let Ok(explicit_value) = ({ let Ok(explicit_value) = d.cmp_eq(f32x8::default()); explicit_value }).blend(
        s * ({ let Ok(explicit_value) = inv(da); explicit_value }),
        { let Ok(explicit_value) = ({ let Ok(explicit_value) = s.cmp_eq(sa); explicit_value }).blend(
            s + d * ({ let Ok(explicit_value) = inv(sa); explicit_value }),
            sa * ({ let Ok(explicit_value) = da.min((d * sa) * ({ let Ok(explicit_value) = (sa - s).reciprocal(); explicit_value })); explicit_value }) + s * ({ let Ok(explicit_value) = inv(da); explicit_value }) + d * ({ let Ok(explicit_value) = inv(sa); explicit_value })
        ); explicit_value }
    ); explicit_value }
);

blend_fn2!(hard_light, |s: f32x8, d: f32x8, sa, da|
    s * ({ let Ok(explicit_value) = inv(da); explicit_value }) + d * ({ let Ok(explicit_value) = inv(sa); explicit_value }) + ({ let Ok(explicit_value) = ({ let Ok(explicit_value) = ({ let Ok(explicit_value) = two(s); explicit_value }).cmp_le(sa); explicit_value }).blend(
        { let Ok(explicit_value) = two(s * d); explicit_value },
        sa * da - ({ let Ok(explicit_value) = two((da - d) * (sa - s)); explicit_value })
    ); explicit_value })
);

blend_fn2!(overlay, |s: f32x8, d: f32x8, sa, da|
    s * ({ let Ok(explicit_value) = inv(da); explicit_value }) + d * ({ let Ok(explicit_value) = inv(sa); explicit_value }) + ({ let Ok(explicit_value) = ({ let Ok(explicit_value) = ({ let Ok(explicit_value) = two(d); explicit_value }).cmp_le(da); explicit_value }).blend(
        { let Ok(explicit_value) = two(s * d); explicit_value },
        sa * da - ({ let Ok(explicit_value) = two((da - d) * (sa - s)); explicit_value })
    ); explicit_value })
);

blend_fn2!(soft_light, |s: f32x8, d: f32x8, sa: f32x8, da: f32x8| {
    let m  = { let Ok(explicit_value) = ({ let Ok(explicit_value) = da.cmp_gt(f32x8::default()); explicit_value }).blend(d / da, f32x8::default()); explicit_value };
    let s2 = { let Ok(explicit_value) = two(s); explicit_value };
    let m4 = { let Ok(explicit_value) = two({ let Ok(explicit_value) = two(m); explicit_value }); explicit_value };

    let dark_src = d * (sa + (s2 - sa) * (({ let Ok(explicit_value) = f32x8::splat(1.0); explicit_value }) - m));
    let dark_dst = (m4 * m4 + m4) * (m - ({ let Ok(explicit_value) = f32x8::splat(1.0); explicit_value })) + ({ let Ok(explicit_value) = f32x8::splat(7.0); explicit_value }) * m;
    let lite_dst = ({ let Ok(explicit_value) = m.sqrt(); explicit_value }) - m;
    let lite_src = d * sa + da * (s2 - sa)
        * ({ let Ok(explicit_value) = ({ let Ok(explicit_value) = ({ let Ok(explicit_value) = two({ let Ok(explicit_value) = two(d); explicit_value }); explicit_value }).cmp_le(da); explicit_value }).blend(dark_dst, lite_dst); explicit_value });

    s * ({ let Ok(explicit_value) = inv(da); explicit_value }) + d * ({ let Ok(explicit_value) = inv(sa); explicit_value }) + ({ let Ok(explicit_value) = ({ let Ok(explicit_value) = s2.cmp_le(sa); explicit_value }).blend(dark_src, lite_src); explicit_value })
});

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the nonseparable modes share the same color and alpha setup, compositing, and stage dispatch for three color channels"
  )
)]
macro_rules! blend_fn3 {
    ($name:ident, $f:expr) => {
        fn $name(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
            let (tr, tg, tb, ta) = $f(p.r, p.g, p.b, p.a, p.dr, p.dg, p.db, p.da);
            p.r = tr;
            p.g = tg;
            p.b = tb;
            p.a = ta;

            let Ok(()) = p.next_stage();

            Ok(())
        }
    };
}

blend_fn3!(hue, |argument_0, argument_1, argument_2, argument_3, argument_4, argument_5, argument_6, argument_7| { let Ok(explicit_value) = hue_k(argument_0, argument_1, argument_2, argument_3, argument_4, argument_5, argument_6, argument_7); explicit_value });

#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "a non-separable blend reads the four source and four destination channels, which are the lanes the Pipeline keeps apart")]
fn hue_k(
    r: f32x8, g: f32x8, b: f32x8, a: f32x8,
    dr: f32x8, dg: f32x8, db: f32x8, da: f32x8,
) -> core::result::Result<(f32x8, f32x8, f32x8, f32x8), console_core_never::Never> {
    let rr = &mut (r * a);
    let gg = &mut (g * a);
    let bb = &mut (b * a);

    ({ let Ok(explicit_value) = set_sat(rr, gg, bb, ({ let Ok(explicit_value) = sat(dr, dg, db); explicit_value }) * a); explicit_value });
    ({ let Ok(explicit_value) = set_lum(rr, gg, bb, ({ let Ok(explicit_value) = lum(dr, dg, db); explicit_value }) * a); explicit_value });
    ({ let Ok(explicit_value) = clip_color(rr, gg, bb, a * da); explicit_value });

    let r_2 = r * ({ let Ok(explicit_value) = inv(da); explicit_value }) + dr * ({ let Ok(explicit_value) = inv(a); explicit_value }) + *rr;
    let g_2 = g * ({ let Ok(explicit_value) = inv(da); explicit_value }) + dg * ({ let Ok(explicit_value) = inv(a); explicit_value }) + *gg;
    let b_2 = b * ({ let Ok(explicit_value) = inv(da); explicit_value }) + db * ({ let Ok(explicit_value) = inv(a); explicit_value }) + *bb;
    let a_2 = a + da - a * da;

    Ok((r_2, g_2, b_2, a_2))
}

blend_fn3!(saturation, |argument_0, argument_1, argument_2, argument_3, argument_4, argument_5, argument_6, argument_7| { let Ok(explicit_value) = saturation_k(argument_0, argument_1, argument_2, argument_3, argument_4, argument_5, argument_6, argument_7); explicit_value });

#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "a non-separable blend reads the four source and four destination channels, which are the lanes the Pipeline keeps apart")]
fn saturation_k(
    r: f32x8, g: f32x8, b: f32x8, a: f32x8,
    dr: f32x8, dg: f32x8, db: f32x8, da: f32x8,
) -> core::result::Result<(f32x8, f32x8, f32x8, f32x8), console_core_never::Never> {
    let rr = &mut (dr * a);
    let gg = &mut (dg * a);
    let bb = &mut (db * a);

    ({ let Ok(explicit_value) = set_sat(rr, gg, bb, ({ let Ok(explicit_value) = sat(r, g, b); explicit_value }) * da); explicit_value });
    ({ let Ok(explicit_value) = set_lum(rr, gg, bb, ({ let Ok(explicit_value) = lum(dr, dg, db); explicit_value }) * a); explicit_value });
    ({ let Ok(explicit_value) = clip_color(rr, gg, bb, a * da); explicit_value });

    let r_2 = r * ({ let Ok(explicit_value) = inv(da); explicit_value }) + dr * ({ let Ok(explicit_value) = inv(a); explicit_value }) + *rr;
    let g_2 = g * ({ let Ok(explicit_value) = inv(da); explicit_value }) + dg * ({ let Ok(explicit_value) = inv(a); explicit_value }) + *gg;
    let b_2 = b * ({ let Ok(explicit_value) = inv(da); explicit_value }) + db * ({ let Ok(explicit_value) = inv(a); explicit_value }) + *bb;
    let a_2 = a + da - a * da;

    Ok((r_2, g_2, b_2, a_2))
}

blend_fn3!(color, |argument_0, argument_1, argument_2, argument_3, argument_4, argument_5, argument_6, argument_7| { let Ok(explicit_value) = color_k(argument_0, argument_1, argument_2, argument_3, argument_4, argument_5, argument_6, argument_7); explicit_value });

#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "a non-separable blend reads the four source and four destination channels, which are the lanes the Pipeline keeps apart")]
fn color_k(
    r: f32x8, g: f32x8, b: f32x8, a: f32x8,
    dr: f32x8, dg: f32x8, db: f32x8, da: f32x8,
) -> core::result::Result<(f32x8, f32x8, f32x8, f32x8), console_core_never::Never> {
    let rr = &mut (r * da);
    let gg = &mut (g * da);
    let bb = &mut (b * da);

    ({ let Ok(explicit_value) = set_lum(rr, gg, bb, ({ let Ok(explicit_value) = lum(dr, dg, db); explicit_value }) * a); explicit_value });
    ({ let Ok(explicit_value) = clip_color(rr, gg, bb, a * da); explicit_value });

    let r_2 = r * ({ let Ok(explicit_value) = inv(da); explicit_value }) + dr * ({ let Ok(explicit_value) = inv(a); explicit_value }) + *rr;
    let g_2 = g * ({ let Ok(explicit_value) = inv(da); explicit_value }) + dg * ({ let Ok(explicit_value) = inv(a); explicit_value }) + *gg;
    let b_2 = b * ({ let Ok(explicit_value) = inv(da); explicit_value }) + db * ({ let Ok(explicit_value) = inv(a); explicit_value }) + *bb;
    let a_2 = a + da - a * da;

    Ok((r_2, g_2, b_2, a_2))
}

blend_fn3!(luminosity, |argument_0, argument_1, argument_2, argument_3, argument_4, argument_5, argument_6, argument_7| { let Ok(explicit_value) = luminosity_k(argument_0, argument_1, argument_2, argument_3, argument_4, argument_5, argument_6, argument_7); explicit_value });

#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "a non-separable blend reads the four source and four destination channels, which are the lanes the Pipeline keeps apart")]
fn luminosity_k(
    r: f32x8, g: f32x8, b: f32x8, a: f32x8,
    dr: f32x8, dg: f32x8, db: f32x8, da: f32x8,
) -> core::result::Result<(f32x8, f32x8, f32x8, f32x8), console_core_never::Never> {
    let rr = &mut (dr * a);
    let gg = &mut (dg * a);
    let bb = &mut (db * a);

    ({ let Ok(explicit_value) = set_lum(rr, gg, bb, ({ let Ok(explicit_value) = lum(r, g, b); explicit_value }) * da); explicit_value });
    ({ let Ok(explicit_value) = clip_color(rr, gg, bb, a * da); explicit_value });

    let r_2 = r * ({ let Ok(explicit_value) = inv(da); explicit_value }) + dr * ({ let Ok(explicit_value) = inv(a); explicit_value }) + *rr;
    let g_2 = g * ({ let Ok(explicit_value) = inv(da); explicit_value }) + dg * ({ let Ok(explicit_value) = inv(a); explicit_value }) + *gg;
    let b_2 = b * ({ let Ok(explicit_value) = inv(da); explicit_value }) + db * ({ let Ok(explicit_value) = inv(a); explicit_value }) + *bb;
    let a_2 = a + da - a * da;

    Ok((r_2, g_2, b_2, a_2))
}

#[inline(always)]
fn sat(r: f32x8, g: f32x8, b: f32x8) -> core::result::Result<f32x8, console_core_never::Never> {
    Ok(({ let Ok(explicit_value) = r.max({ let Ok(explicit_value) = g.max(b); explicit_value }); explicit_value }) - ({ let Ok(explicit_value) = r.min({ let Ok(explicit_value) = g.min(b); explicit_value }); explicit_value }))
}

#[inline(always)]
fn lum(r: f32x8, g: f32x8, b: f32x8) -> core::result::Result<f32x8, console_core_never::Never> {
    Ok(r * ({ let Ok(explicit_value) = f32x8::splat(0.30); explicit_value }) + g * ({ let Ok(explicit_value) = f32x8::splat(0.59); explicit_value }) + b * ({ let Ok(explicit_value) = f32x8::splat(0.11); explicit_value }))
}

#[inline(always)]
fn set_sat(r: &mut f32x8, g: &mut f32x8, b: &mut f32x8, s: f32x8) -> core::result::Result<(), console_core_never::Never> {
    let mn  = { let Ok(explicit_value) = r.min({ let Ok(explicit_value) = g.min(*b); explicit_value }); explicit_value };
    let mx  = { let Ok(explicit_value) = r.max({ let Ok(explicit_value) = g.max(*b); explicit_value }); explicit_value };
    let sat = mx - mn;

    let scale = |c| { let Ok(explicit_value) = ({ let Ok(explicit_value) = sat.cmp_eq(f32x8::default()); explicit_value })
                       .blend(f32x8::default(), (c - mn) * s / sat); explicit_value };

    *r = scale(*r);
    *g = scale(*g);
    *b = scale(*b);

    Ok(())
}

#[inline(always)]
fn set_lum(r: &mut f32x8, g: &mut f32x8, b: &mut f32x8, l: f32x8) -> core::result::Result<(), console_core_never::Never> {
    let diff = l - ({ let Ok(explicit_value) = lum(*r, *g, *b); explicit_value });
    *r += diff;
    *g += diff;
    *b += diff;

    Ok(())
}

#[inline(always)]
fn clip_color(r: &mut f32x8, g: &mut f32x8, b: &mut f32x8, a: f32x8) -> core::result::Result<(), console_core_never::Never> {
    let mn = { let Ok(explicit_value) = r.min({ let Ok(explicit_value) = g.min(*b); explicit_value }); explicit_value };
    let mx = { let Ok(explicit_value) = r.max({ let Ok(explicit_value) = g.max(*b); explicit_value }); explicit_value };
    let l  = { let Ok(explicit_value) = lum(*r, *g, *b); explicit_value };

    let clip = |mut c| {
        c = { let Ok(explicit_value) = ({ let Ok(explicit_value) = mx.cmp_ge(f32x8::default()); explicit_value }).blend(c, l + (c - l) * l / (l - mn)); explicit_value };
        c = { let Ok(explicit_value) = ({ let Ok(explicit_value) = mx.cmp_gt(a); explicit_value }).blend(l + (c - l) * (a - l) / (mx - l), c); explicit_value };
        c = { let Ok(explicit_value) = c.max(f32x8::default()); explicit_value };
        c
    };

    *r = clip(*r);
    *g = clip(*g);
    *b = clip(*b);

    Ok(())
}

pub fn source_over_rgba(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(found_pixels) = p.pixmap_dst.pixels8(p.position);
    let pixels = match found_pixels {
        Some(pixels) => pixels,
        None => return Ok(()),
    };

    let Ok(()) = load_8888(pixels, &mut p.dr, &mut p.dg, &mut p.db, &mut p.da);
    p.r = { let Ok(explicit_value) = mad(p.dr, { let Ok(explicit_value) = inv(p.a); explicit_value }, p.r); explicit_value };
    p.g = { let Ok(explicit_value) = mad(p.dg, { let Ok(explicit_value) = inv(p.a); explicit_value }, p.g); explicit_value };
    p.b = { let Ok(explicit_value) = mad(p.db, { let Ok(explicit_value) = inv(p.a); explicit_value }, p.b); explicit_value };
    p.a = { let Ok(explicit_value) = mad(p.da, { let Ok(explicit_value) = inv(p.a); explicit_value }, p.a); explicit_value };
    let Ok(()) = store_8888(&p.r, &p.g, &p.b, &p.a, pixels);

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

pub fn source_over_rgba_tail(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let pixels = { let Ok(explicit_value) = p.pixmap_dst.pixels_from(p.position); explicit_value };
    ({ let Ok(explicit_value) = load_8888_tail(p.tail, pixels, &mut p.dr, &mut p.dg, &mut p.db, &mut p.da); explicit_value });
    p.r = { let Ok(explicit_value) = mad(p.dr, { let Ok(explicit_value) = inv(p.a); explicit_value }, p.r); explicit_value };
    p.g = { let Ok(explicit_value) = mad(p.dg, { let Ok(explicit_value) = inv(p.a); explicit_value }, p.g); explicit_value };
    p.b = { let Ok(explicit_value) = mad(p.db, { let Ok(explicit_value) = inv(p.a); explicit_value }, p.b); explicit_value };
    p.a = { let Ok(explicit_value) = mad(p.da, { let Ok(explicit_value) = inv(p.a); explicit_value }, p.a); explicit_value };
    ({ let Ok(explicit_value) = store_8888_tail(&p.r, &p.g, &p.b, &p.a, p.tail, pixels); explicit_value });

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn transform(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ts = &p.ctx.transform;

    let tr = { let Ok(explicit_value) = mad(p.r, { let Ok(explicit_value) = f32x8::splat(ts.sx); explicit_value }, { let Ok(explicit_value) = mad(p.g, { let Ok(explicit_value) = f32x8::splat(ts.kx); explicit_value }, { let Ok(explicit_value) = f32x8::splat(ts.tx); explicit_value }); explicit_value }); explicit_value };
    let tg = { let Ok(explicit_value) = mad(p.r, { let Ok(explicit_value) = f32x8::splat(ts.ky); explicit_value }, { let Ok(explicit_value) = mad(p.g, { let Ok(explicit_value) = f32x8::splat(ts.sy); explicit_value }, { let Ok(explicit_value) = f32x8::splat(ts.ty); explicit_value }); explicit_value }); explicit_value };
    p.r = tr;
    p.g = tg;

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn reflect(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ctx = &p.ctx.limit_x;
    p.r = { let Ok(explicit_value) = exclusive_reflect(p.r, *ctx); explicit_value };

    let ctx_2 = &p.ctx.limit_y;
    p.g = { let Ok(explicit_value) = exclusive_reflect(p.g, *ctx_2); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

#[inline(always)]
fn exclusive_reflect(v: f32x8, TileCtx { scale: limit, inv_scale: inv_limit }: TileCtx) -> core::result::Result<f32x8, console_core_never::Never> {
    let limit_2 = { let Ok(explicit_value) = f32x8::splat(limit); explicit_value };
    let inv_limit_2 = { let Ok(explicit_value) = f32x8::splat(inv_limit); explicit_value };

    Ok({ let Ok(explicit_value) = ((v - limit_2) - (limit_2 + limit_2)
        * ({ let Ok(explicit_value) = ((v - limit_2) * (inv_limit_2 * ({ let Ok(explicit_value) = f32x8::splat(0.5); explicit_value }))).floor(); explicit_value }) - limit_2).abs(); explicit_value })
}

fn repeat(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ctx = &p.ctx.limit_x;
    p.r = { let Ok(explicit_value) = exclusive_repeat(p.r, *ctx); explicit_value };

    let ctx_2 = &p.ctx.limit_y;
    p.g = { let Ok(explicit_value) = exclusive_repeat(p.g, *ctx_2); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

#[inline(always)]
fn exclusive_repeat(v: f32x8, TileCtx { scale: limit, inv_scale: inv_limit }: TileCtx) -> core::result::Result<f32x8, console_core_never::Never> {
    Ok(v - ({ let Ok(explicit_value) = (v * ({ let Ok(explicit_value) = f32x8::splat(inv_limit); explicit_value })).floor(); explicit_value }) * ({ let Ok(explicit_value) = f32x8::splat(limit); explicit_value }))
}

fn bilinear(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok([r, g, b, a]) = sample_bilinear(p.pixmap_src, &p.ctx.sampler, [p.r, p.g]);
    (p.r, p.g, p.b, p.a) = (r, g, b, a);

    p.next_stage()
}

#[inline(always)]
fn bilinear_axis(
    center: f32x8,
    mode: SpreadMode,
    limits: TileCtx,
) -> Result<([i32x8; 2], [f32x8; 2]), Never> {
    let Ok(half) = f32x8::splat(0.5);
    let Ok(one) = f32x8::splat(1.0);
    let Ok(fraction) = (center + half).fract();
    let first = center - half;
    let Ok(lower_sample) = tile(first, mode, limits);
    let Ok(upper_sample) = tile(first + one, mode, limits);
    let Ok(lower) = sample_coordinate(lower_sample, limits.scale.to_bits());
    let Ok(upper) = sample_coordinate(upper_sample, limits.scale.to_bits());
    Ok(([lower, upper], [one - fraction, fraction]))
}

#[inline(always)]
fn sample_bilinear(
    pixmap: PixmapRef,
    ctx: &super::SamplerCtx,
    [x, y]: [f32x8; 2],
) -> Result<[f32x8; 4], Never> {
    let Ok((columns, weights_x)) = bilinear_axis(x, ctx.spread_mode, ctx.horizontal);
    let Ok((rows, weights_y)) = bilinear_axis(y, ctx.spread_mode, ctx.vertical);
    let Ok(width) = pixmap.width();
    let Ok(width_i32) = wrapped::<_, i32>(width);
    let Ok(stride) = i32x8::splat(width_i32);
    let mut color = [f32x8::default(); 4];

    for (row, weight_y) in rows.into_iter().zip(weights_y) {
        let row_start = row * stride;

        for (column, weight_x) in columns.into_iter().zip(weights_x) {
            let Ok(indices) = (row_start + column).to_u32x8_bitcast();
            let Ok(texel) = load_sample(pixmap, indices);
            let Ok(()) = accumulate_sample(&mut color, texel, weight_x * weight_y);
        }
    }

    Ok(color)
}

#[cfg(test)]
#[test]
fn bilinear_reuses_coordinates_without_changing_the_sample()
    -> Result<(), crate::test_support::TestFailure>
{
    use crate::test_support::Required;

    let texels: [[u8; 4]; 12] = [
        [0, 9, 24, 90], [96, 37, 48, 150], [123, 80, 17, 160], [13, 54, 72, 200],
        [17, 77, 89, 120], [220, 6, 183, 244], [0, 14, 54, 87], [9, 13, 20, 232],
        [35, 18, 57, 64], [83, 12, 4, 90], [142, 24, 0, 255], [0, 3, 56, 99],
    ];
    let (width, height) = (4, 3);
    let Ok(image) = PixmapRef::from_bytes(bytemuck::cast_slice(&texels), width, height);
    let pixmap = image.required()?;
    let horizontal = TileCtx { scale: 4.0, inv_scale: 0.25 };
    let vertical = TileCtx { scale: 3.0, inv_scale: 1.0 / 3.0 };
    let x = f32x8::from([-8.75, -0.875, 0.0, 0.5, 1.499_999_9, 2.125, 3.75, 5.625]);
    let y = f32x8::from([1.25, 0.0, -0.875, 2.75, 0.5, 1.125, 3.25, 8.625]);
    let Ok(half) = f32x8::splat(0.5);
    let Ok(one) = f32x8::splat(1.0);
    let Ok(offset) = f32x8::splat(-0.5);
    let Ok(fx) = (x + half).fract();
    let Ok(fy) = (y + half).fract();

    for spread_mode in [SpreadMode::Pad, SpreadMode::Repeat, SpreadMode::Reflect] {
        let ctx = super::SamplerCtx { spread_mode, horizontal, vertical };
        let Ok(expected) = sample_window(pixmap, &ctx, [x, y], [[one - fx, fx], [one - fy, fy]], offset);
        let Ok(actual) = sample_bilinear(pixmap, &ctx, [x, y]);
        assert_eq!(actual, expected, "{spread_mode:?}");
    }

    Ok(())
}

fn bicubic(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let x = p.r;
    let fx = { let Ok(explicit_value) = (x + ({ let Ok(explicit_value) = f32x8::splat(0.5); explicit_value })).fract(); explicit_value };
    let y = p.g;
    let fy = { let Ok(explicit_value) = (y + ({ let Ok(explicit_value) = f32x8::splat(0.5); explicit_value })).fract(); explicit_value };
    let one = { let Ok(explicit_value) = f32x8::splat(1.0); explicit_value };
    let wx = [({ let Ok(explicit_value) = bicubic_far(one - fx); explicit_value }), ({ let Ok(explicit_value) = bicubic_near(one - fx); explicit_value }), ({ let Ok(explicit_value) = bicubic_near(fx); explicit_value }), ({ let Ok(explicit_value) = bicubic_far(fx); explicit_value })];
    let wy = [({ let Ok(explicit_value) = bicubic_far(one - fy); explicit_value }), ({ let Ok(explicit_value) = bicubic_near(one - fy); explicit_value }), ({ let Ok(explicit_value) = bicubic_near(fy); explicit_value }), ({ let Ok(explicit_value) = bicubic_far(fy); explicit_value })];

    let Ok(offset) = f32x8::splat(-1.5);
    let Ok([r, g, b, a]) = sample_window(p.pixmap_src, &p.ctx.sampler, [x, y], [wx, wy], offset);
    (p.r, p.g, p.b, p.a) = (r, g, b, a);


    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

#[inline(always)]
fn bicubic_near(t: f32x8) -> core::result::Result<f32x8, console_core_never::Never> {
    Ok({ let Ok(explicit_value) = mad(
        t,
        { let Ok(explicit_value) = mad(t,
            { let Ok(explicit_value) = mad(
                { let Ok(explicit_value) = f32x8::splat(-21.0/18.0); explicit_value },
                t,
                { let Ok(explicit_value) = f32x8::splat(27.0/18.0); explicit_value },
            ); explicit_value },
            { let Ok(explicit_value) = f32x8::splat(9.0/18.0); explicit_value },
        ); explicit_value },
        { let Ok(explicit_value) = f32x8::splat(1.0/18.0); explicit_value },
    ); explicit_value })
}

#[inline(always)]
fn bicubic_far(t: f32x8) -> core::result::Result<f32x8, console_core_never::Never> {
    Ok((t * t) * ({ let Ok(explicit_value) = mad({ let Ok(explicit_value) = f32x8::splat(7.0/18.0); explicit_value }, t, { let Ok(explicit_value) = f32x8::splat(-6.0/18.0); explicit_value }); explicit_value }))
}

#[inline(always)]
fn sample_window<const N: usize>(
    pixmap: PixmapRef,
    ctx: &super::SamplerCtx,
    [center_x, center_y]: [f32x8; 2],
    [weights_x, weights_y]: [[f32x8; N]; 2],
    offset: f32x8,
) -> Result<[f32x8; 4], Never> {
    let mut color = [f32x8::default(); 4];
    let Ok(one) = f32x8::splat(1.0);
    let mut y = center_y + offset;

    for weight_y in weights_y {
        let mut x = center_x + offset;

        for weight_x in weights_x {
            let Ok(texel) = sample(pixmap, ctx, [x, y]);
            let Ok(()) = accumulate_sample(&mut color, texel, weight_x * weight_y);

            x += one;
        }

        y += one;
    }

    Ok(color)
}

#[inline(always)]
fn accumulate_sample(
    color: &mut [f32x8; 4],
    texel: [f32x8; 4],
    weight: f32x8,
) -> Result<(), Never> {
    for (target, source) in color.iter_mut().zip(texel) {
        let Ok(accumulated) = mad(weight, source, *target);
        *target = accumulated;
    }

    Ok(())
}

#[inline(always)]
fn sample(pixmap: PixmapRef, ctx: &super::SamplerCtx, [x,y]: [f32x8; 2]) -> Result<[f32x8; 4], Never> {
    let Ok(tx) = tile(x, ctx.spread_mode, ctx.horizontal);
    let Ok(ty) = tile(y, ctx.spread_mode, ctx.vertical);
    let Ok(indices) = gather_ix(pixmap, tx, ty);
    load_sample(pixmap, indices)
}

#[inline(always)]
fn load_sample(pixmap: PixmapRef, indices: u32x8) -> Result<[f32x8; 4], Never> {
    let Ok(texels) = pixmap.gather(indices);
    let mut rgba = [f32x8::default(); 4];
    let [red, green, blue, alpha] = &mut rgba;
    let Ok(()) = load_8888(&texels, red, green, blue, alpha);

    Ok(rgba)
}

#[inline(always)]
fn tile(value: f32x8, mode: SpreadMode, limits: TileCtx) -> Result<f32x8, Never> {
    match mode {
        SpreadMode::Pad => Ok(value),
        SpreadMode::Repeat => exclusive_repeat(value, limits),
        SpreadMode::Reflect => exclusive_reflect(value, limits),
    }
}

fn pad_x1(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = { let Ok(explicit_value) = p.r.normalize(); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn reflect_x1(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = { let Ok(explicit_value) = ({ let Ok(explicit_value) = (
        (p.r - ({ let Ok(explicit_value) = f32x8::splat(1.0); explicit_value }))
            - ({ let Ok(explicit_value) = two({ let Ok(explicit_value) = ((p.r - ({ let Ok(explicit_value) = f32x8::splat(1.0); explicit_value })) * ({ let Ok(explicit_value) = f32x8::splat(0.5); explicit_value })).floor(); explicit_value }); explicit_value })
            - ({ let Ok(explicit_value) = f32x8::splat(1.0); explicit_value })
    ).abs(); explicit_value }).normalize(); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn repeat_x1(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = { let Ok(explicit_value) = (p.r - ({ let Ok(explicit_value) = p.r.floor(); explicit_value })).normalize(); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn gradient(p: &mut Pipeline) -> Result<(), console_core_never::Never> {
    let bands = &p.ctx.gradient;
    let t = p.r;
    let coordinates: Lanes<f32> = bytemuck::cast(t);
    let mut indices = [0u32; 8];

    for band in bands.iter().skip(1) {
        let threshold = band.start.get();

        for (slot, coordinate) in indices.iter_mut().zip(coordinates) {
            *slot = slot.wrapping_add(u32::from(coordinate >= threshold));
        }
    }

    let sampled = indices.map(|which| {
        let Ok(position) = console_core_number_conversion::index(which);

        match bands.get(position) {
            Some(band) => band.coeffs,
            None => super::GradientCoefficients::default(),
        }
    });
    let Ok(factors) = gradient_channels(sampled.map(|entry| entry.factor));
    let Ok(biases) = gradient_channels(sampled.map(|entry| entry.bias));
    let Ok([r, g, b, a]) = interpolate_channels(t, factors, biases);
    p.r = r;
    p.g = g;
    p.b = b;
    p.a = a;

    p.next_stage()
}

fn gradient_channels(entries: Lanes<super::GradientColor>) -> Result<[f32x8; 4], console_core_never::Never> {
    let packed: [[f32; 8]; 4] = [
        entries.map(|super::GradientColor([r, _, _, _])| r),
        entries.map(|super::GradientColor([_, g, _, _])| g),
        entries.map(|super::GradientColor([_, _, b, _])| b),
        entries.map(|super::GradientColor([_, _, _, a])| a),
    ];

    Ok(packed.map(bytemuck::cast))
}

fn interpolate_channels(
    t: f32x8,
    [red, green, blue, alpha]: [f32x8; 4],
    [red_bias, green_bias, blue_bias, alpha_bias]: [f32x8; 4],
) -> Result<[f32x8; 4], console_core_never::Never> {
    let Ok(r) = mad(t, red, red_bias);
    let Ok(g) = mad(t, green, green_bias);
    let Ok(b) = mad(t, blue, blue_bias);
    let Ok(a) = mad(t, alpha, alpha_bias);

    Ok([r, g, b, a])
}

fn evenly_spaced_2_stop_gradient(p: &mut Pipeline) -> Result<(), console_core_never::Never> {
    let ctx = p.ctx.evenly_spaced_2_stop_gradient;
    let t = p.r;
    let factors = ctx.factor.0.map(|value| {
        let Ok(splat) = f32x8::splat(value);

        splat
    });
    let biases = ctx.bias.0.map(|value| {
        let Ok(splat) = f32x8::splat(value);

        splat
    });
    let Ok([r, g, b, a]) = interpolate_channels(t, factors, biases);
    p.r = r;
    p.g = g;
    p.b = b;
    p.a = a;

    p.next_stage()
}

fn xy_to_unit_angle(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let x = p.r;
    let y = p.g;
    let x_abs = { let Ok(explicit_value) = x.abs(); explicit_value };
    let y_abs = { let Ok(explicit_value) = y.abs(); explicit_value };
    let slope = ({ let Ok(explicit_value) = x_abs.min(y_abs); explicit_value }) / ({ let Ok(explicit_value) = x_abs.max(y_abs); explicit_value });
    let s = slope * slope;

    let phi = slope
        * (({ let Ok(explicit_value) = f32x8::splat(0.159_121_17); explicit_value })
           + s * (({ let Ok(explicit_value) = f32x8::splat(-5.185_397e-2); explicit_value })
                  + s * (({ let Ok(explicit_value) = f32x8::splat(2.476_102e-2); explicit_value })
                         + s * ({ let Ok(explicit_value) = f32x8::splat(-7.054_738_2e-3); explicit_value }))));
    let phi_2 = { let Ok(explicit_value) = ({ let Ok(explicit_value) = x_abs.cmp_lt(y_abs); explicit_value }).blend(({ let Ok(explicit_value) = f32x8::splat(0.25); explicit_value }) - phi, phi); explicit_value };
    let phi_3 = { let Ok(explicit_value) = ({ let Ok(explicit_value) = x
        .cmp_lt({ let Ok(explicit_value) = f32x8::splat(0.0); explicit_value }); explicit_value })
        .blend(({ let Ok(explicit_value) = f32x8::splat(0.5); explicit_value }) - phi_2, phi_2); explicit_value };
    let phi_4 = { let Ok(explicit_value) = ({ let Ok(explicit_value) = y
        .cmp_lt({ let Ok(explicit_value) = f32x8::splat(0.0); explicit_value }); explicit_value })
        .blend(({ let Ok(explicit_value) = f32x8::splat(1.0); explicit_value }) - phi_3, phi_3); explicit_value };
    let phi_5 = { let Ok(explicit_value) = ({ let Ok(explicit_value) = phi_4.cmp_ne(phi_4); explicit_value }).blend({ let Ok(explicit_value) = f32x8::splat(0.0); explicit_value }, phi_4); explicit_value };
    p.r = phi_5;
    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn xy_to_radius(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let x2 = p.r * p.r;
    let y2 = p.g * p.g;
    p.r = { let Ok(explicit_value) = (x2 + y2).sqrt(); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn xy_to_2pt_conical_focal_on_circle(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let x = p.r;
    let y = p.g;
    p.r = x + y * y / x;

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn xy_to_2pt_conical_well_behaved(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ctx = &p.ctx.two_point_conical_gradient;

    let x = p.r;
    let y = p.g;
    p.r = ({ let Ok(explicit_value) = (x * x + y * y).sqrt(); explicit_value }) - x * ({ let Ok(explicit_value) = f32x8::splat(ctx.p0); explicit_value });

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn xy_to_2pt_conical_greater(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ctx = &p.ctx.two_point_conical_gradient;

    let x = p.r;
    let y = p.g;
    p.r = ({ let Ok(explicit_value) = (x * x - y * y).sqrt(); explicit_value }) - x * ({ let Ok(explicit_value) = f32x8::splat(ctx.p0); explicit_value });

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn xy_to_2pt_conical_smaller(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ctx = &p.ctx.two_point_conical_gradient;

    let x = p.r;
    let y = p.g;
    p.r = -({ let Ok(explicit_value) = (x * x - y * y).sqrt(); explicit_value }) - x * ({ let Ok(explicit_value) = f32x8::splat(ctx.p0); explicit_value });

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn xy_to_2pt_conical_strip(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ctx = &p.ctx.two_point_conical_gradient;

    let x = p.r;
    let y = p.g;
    p.r = x + ({ let Ok(explicit_value) = (({ let Ok(explicit_value) = f32x8::splat(ctx.p0); explicit_value }) - y * y).sqrt(); explicit_value });

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn mask_2pt_conical_nan(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ctx = &mut p.ctx.two_point_conical_gradient;

    let t = p.r;
    let is_degenerate = { let Ok(explicit_value) = t.cmp_ne(t); explicit_value };
    p.r = { let Ok(explicit_value) = is_degenerate.blend(f32x8::default(), t); explicit_value };
    ctx.mask = { let Ok(explicit_value) = cond_to_mask(!({ let Ok(explicit_value) = is_degenerate.to_u32x8_mask(); explicit_value })); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn mask_2pt_conical_degenerates(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ctx = &mut p.ctx.two_point_conical_gradient;

    let t = p.r;
    let is_degenerate = ({ let Ok(explicit_value) = t.cmp_le(f32x8::default()); explicit_value }) | ({ let Ok(explicit_value) = t.cmp_ne(t); explicit_value });
    p.r = { let Ok(explicit_value) = is_degenerate.blend(f32x8::default(), t); explicit_value };
    ctx.mask = { let Ok(explicit_value) = cond_to_mask(!({ let Ok(explicit_value) = is_degenerate.to_u32x8_mask(); explicit_value })); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn apply_vector_mask(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ctx = &p.ctx.two_point_conical_gradient;

    p.r = { let Ok(explicit_value) = (({ let Ok(explicit_value) = p.r.to_u32x8_bitcast(); explicit_value }) & ctx.mask).to_f32x8_bitcast(); explicit_value };
    p.g = { let Ok(explicit_value) = (({ let Ok(explicit_value) = p.g.to_u32x8_bitcast(); explicit_value }) & ctx.mask).to_f32x8_bitcast(); explicit_value };
    p.b = { let Ok(explicit_value) = (({ let Ok(explicit_value) = p.b.to_u32x8_bitcast(); explicit_value }) & ctx.mask).to_f32x8_bitcast(); explicit_value };
    p.a = { let Ok(explicit_value) = (({ let Ok(explicit_value) = p.a.to_u32x8_bitcast(); explicit_value }) & ctx.mask).to_f32x8_bitcast(); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn alter_2pt_conical_compensate_focal(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ctx = &p.ctx.two_point_conical_gradient;

    let Ok(compensation) = f32x8::splat(ctx.p1);
    p.r += compensation;

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn alter_2pt_conical_unswap(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = ({ let Ok(explicit_value) = f32x8::splat(1.0); explicit_value }) - p.r;

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn negate_x(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = -p.r;

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn apply_concentric_scale_bias(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ctx = &p.ctx.two_point_conical_gradient;

    let x = p.r;
    p.r = x * ({ let Ok(explicit_value) = f32x8::splat(ctx.p0); explicit_value }) + ({ let Ok(explicit_value) = f32x8::splat(ctx.p1); explicit_value });

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn gamma_expand_2(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = p.r * p.r;
    p.g = p.g * p.g;
    p.b = p.b * p.b;

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn gamma_expand_dst_2(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.dr = p.dr * p.dr;
    p.dg = p.dg * p.dg;
    p.db = p.db * p.db;

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn gamma_compress_2(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = { let Ok(explicit_value) = p.r.sqrt(); explicit_value };
    p.g = { let Ok(explicit_value) = p.g.sqrt(); explicit_value };
    p.b = { let Ok(explicit_value) = p.b.sqrt(); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn gamma_expand_22(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = { let Ok(explicit_value) = p.r.powf(GammaExponent(2.2)); explicit_value };
    p.g = { let Ok(explicit_value) = p.g.powf(GammaExponent(2.2)); explicit_value };
    p.b = { let Ok(explicit_value) = p.b.powf(GammaExponent(2.2)); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn gamma_expand_dst_22(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.dr = { let Ok(explicit_value) = p.dr.powf(GammaExponent(2.2)); explicit_value };
    p.dg = { let Ok(explicit_value) = p.dg.powf(GammaExponent(2.2)); explicit_value };
    p.db = { let Ok(explicit_value) = p.db.powf(GammaExponent(2.2)); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn gamma_compress_22(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = { let Ok(explicit_value) = p.r.powf(GammaExponent(0.45454545)); explicit_value };
    p.g = { let Ok(explicit_value) = p.g.powf(GammaExponent(0.45454545)); explicit_value };
    p.b = { let Ok(explicit_value) = p.b.powf(GammaExponent(0.45454545)); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn srgb_expand(x: f32x8) -> core::result::Result<f32x8, console_core_never::Never> {
    let small = { let Ok(explicit_value) = x.cmp_le({ let Ok(explicit_value) = f32x8::splat(0.04045); explicit_value }); explicit_value };
    let linear = x / ({ let Ok(explicit_value) = f32x8::splat(12.92); explicit_value });
    let exp = { let Ok(explicit_value) = ((x + ({ let Ok(explicit_value) = f32x8::splat(0.055); explicit_value })) / ({ let Ok(explicit_value) = f32x8::splat(1.055); explicit_value })).powf(GammaExponent(2.4)); explicit_value };

    Ok({ let Ok(explicit_value) = small.blend(linear, exp); explicit_value })
}

fn srgb_compress(x: f32x8) -> core::result::Result<f32x8, console_core_never::Never> {
    let small = { let Ok(explicit_value) = x.cmp_le({ let Ok(explicit_value) = f32x8::splat(0.0031308); explicit_value }); explicit_value };
    let linear = x * ({ let Ok(explicit_value) = f32x8::splat(12.92); explicit_value });
    let exp = ({ let Ok(explicit_value) = x.powf(GammaExponent(0.416_666_66)); explicit_value }) * ({ let Ok(explicit_value) = f32x8::splat(1.055); explicit_value }) - ({ let Ok(explicit_value) = f32x8::splat(0.055); explicit_value });

    Ok({ let Ok(explicit_value) = small.blend(linear, exp); explicit_value })
}

fn gamma_expand_srgb(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = { let Ok(explicit_value) = srgb_expand(p.r); explicit_value };
    p.g = { let Ok(explicit_value) = srgb_expand(p.g); explicit_value };
    p.b = { let Ok(explicit_value) = srgb_expand(p.b); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn gamma_expand_dst_srgb(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.dr = { let Ok(explicit_value) = srgb_expand(p.dr); explicit_value };
    p.dg = { let Ok(explicit_value) = srgb_expand(p.dg); explicit_value };
    p.db = { let Ok(explicit_value) = srgb_expand(p.db); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

fn gamma_compress_srgb(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = { let Ok(explicit_value) = srgb_compress(p.r); explicit_value };
    p.g = { let Ok(explicit_value) = srgb_compress(p.g); explicit_value };
    p.b = { let Ok(explicit_value) = srgb_compress(p.b); explicit_value };

    ({ let Ok(explicit_value) = p.next_stage(); explicit_value });

    Ok(())
}

#[inline(always)]
fn cond_to_mask(cond: u32x8) -> core::result::Result<u32x8, console_core_never::Never> {
    let cond_2: [u32; 8] = bytemuck::cast(cond);

    Ok(bytemuck::cast([
        match ::core::primitive::bool::then_some((match cond_2 { [element, ..] => element }) != 0, ()) { ::core::option::Option::Some(()) => { !0 }, ::core::option::Option::None => { 0 } },
        match ::core::primitive::bool::then_some((match cond_2 { [_, element, ..] => element }) != 0, ()) { ::core::option::Option::Some(()) => { !0 }, ::core::option::Option::None => { 0 } },
        match ::core::primitive::bool::then_some((match cond_2 { [_, _, element, ..] => element }) != 0, ()) { ::core::option::Option::Some(()) => { !0 }, ::core::option::Option::None => { 0 } },
        match ::core::primitive::bool::then_some((match cond_2 { [_, _, _, element, ..] => element }) != 0, ()) { ::core::option::Option::Some(()) => { !0 }, ::core::option::Option::None => { 0 } },
        match ::core::primitive::bool::then_some((match cond_2 { [_, _, _, _, _explicit_element, ..] => _explicit_element }) != 0, ()) { ::core::option::Option::Some(()) => { !0 }, ::core::option::Option::None => { 0 } },
        match ::core::primitive::bool::then_some((match cond_2 { [_, _, _, _, _, _explicit_element, ..] => _explicit_element }) != 0, ()) { ::core::option::Option::Some(()) => { !0 }, ::core::option::Option::None => { 0 } },
        match ::core::primitive::bool::then_some((match cond_2 { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }) != 0, ()) { ::core::option::Option::Some(()) => { !0 }, ::core::option::Option::None => { 0 } },
        match ::core::primitive::bool::then_some((match cond_2 { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }) != 0, ()) { ::core::option::Option::Some(()) => { !0 }, ::core::option::Option::None => { 0 } },
    ]))
}

#[inline(always)]
fn load_8888(
    data: &Lanes<PremultipliedColorU8>,
    r: &mut f32x8, g: &mut f32x8, b: &mut f32x8, a: &mut f32x8,
) -> core::result::Result<(), console_core_never::Never> {
    *r = f32x8::from(data.map(|pixel| {
        let Ok(value) = pixel.red();

        f32::from(value) * (1.0 / 255.0)
    }));

    *g = f32x8::from(data.map(|pixel| {
        let Ok(value) = pixel.green();

        f32::from(value) * (1.0 / 255.0)
    }));

    *b = f32x8::from(data.map(|pixel| {
        let Ok(value) = pixel.blue();

        f32::from(value) * (1.0 / 255.0)
    }));

    *a = f32x8::from(data.map(|pixel| {
        let Ok(value) = pixel.alpha();

        f32::from(value) * (1.0 / 255.0)
    }));

    Ok(())
}

#[inline(always)]
fn load_8888_tail(
    tail: u32, data: &[PremultipliedColorU8],
    r: &mut f32x8, g: &mut f32x8, b: &mut f32x8, a: &mut f32x8,
) -> core::result::Result<(), console_core_never::Never> {
    let mut ready = [PremultipliedColorU8::TRANSPARENT; 8];
    let Ok(lanes) = console_core_number_conversion::index(tail);

    for (pixel, source) in ready.iter_mut().zip(data).take(lanes) {
        *pixel = *source;
    }

    load_8888(&ready, r, g, b, a)
}

#[inline(always)]
fn store_8888(
    r: &f32x8, g: &f32x8, b: &f32x8, a: &f32x8,
    data: &mut Lanes<PremultipliedColorU8>,
) -> core::result::Result<(), console_core_never::Never> {
    store_8888_tail(r, g, b, a, STAGE_WIDTH, data)
}

#[inline(always)]
fn store_8888_tail(
    r: &f32x8, g: &f32x8, b: &f32x8, a: &f32x8,
    tail: u32, data: &mut [PremultipliedColorU8],
) -> core::result::Result<(), console_core_never::Never> {
    let Ok(r_packed) = unnorm(r);
    let red: Lanes<i32> = bytemuck::cast(r_packed);
    let Ok(g_packed) = unnorm(g);
    let green: Lanes<i32> = bytemuck::cast(g_packed);
    let Ok(b_packed) = unnorm(b);
    let blue: Lanes<i32> = bytemuck::cast(b_packed);
    let Ok(a_packed) = unnorm(a);
    let alpha: Lanes<i32> = bytemuck::cast(a_packed);
    let Ok(lanes) = console_core_number_conversion::index(tail);
    let channels = red.iter().zip(green.iter()).zip(blue.iter().zip(alpha.iter()));

    for (destination, ((&red_3, &green_3), (&blue_3, &alpha_3))) in data.iter_mut().zip(channels).take(lanes) {
        let Ok(red_2) = console_core_number_conversion::wrapped::<i32, u8>(red_3);
        let Ok(green_2) = console_core_number_conversion::wrapped::<i32, u8>(green_3);
        let Ok(blue_2) = console_core_number_conversion::wrapped::<i32, u8>(blue_3);
        let Ok(alpha_2) = console_core_number_conversion::wrapped::<i32, u8>(alpha_3);
        let Ok(pixel) = PremultipliedColorU8::from_components([red_2, green_2, blue_2, alpha_2]);

        *destination = pixel;
    }

    Ok(())
}

#[inline(always)]
fn unnorm(v: &f32x8) -> core::result::Result<i32x8, console_core_never::Never> {
    v.to_pixel_channels()
}

#[inline(always)]
fn inv(v: f32x8) -> core::result::Result<f32x8, console_core_never::Never> {
    Ok(({ let Ok(explicit_value) = f32x8::splat(1.0); explicit_value }) - v)
}

#[inline(always)]
fn two(v: f32x8) -> core::result::Result<f32x8, console_core_never::Never> {
    Ok(v + v)
}

#[inline(always)]
fn mad(f: f32x8, m: f32x8, a: f32x8) -> core::result::Result<f32x8, console_core_never::Never> {
    Ok(f * m + a)
}

#[inline(always)]
fn lerp(from: f32x8, to: f32x8, t: f32x8) -> core::result::Result<f32x8, console_core_never::Never> {
    Ok({ let Ok(explicit_value) = mad(to - from, t, from); explicit_value })
}
