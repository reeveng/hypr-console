

/*!
A low precision raster pipeline implementation.

A lowp pipeline uses u16 instead of f32 for math.
Because of that, it doesn't implement stages that require high precision.
The pipeline compiler will automatically decide which one to use.

The sixteen-lane operations use Rust's portable SIMD API when enabled. The
compiler lowers them to the instructions available on the configured target.
*/

use console_core_paths::{NormalizedF32, StrictNum};
use crate::PremultipliedColorU8;

use crate::pixmap::SubPixmapMut;
use crate::wide::{f32x8, u16x16, f32x16};
use crate::geom::{PixelPosition, ScreenIntRect};

const STAGE_WIDTH: u32 = 16;
pub type Lanes<T> = [T; 16];

pub type StageFn = fn(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never>;

pub struct Pipeline<'a, 'b: 'a> {
    #[cfg(feature = "simd")]
    simd: console_core_simd::Simd,
    functions: &'a [StageFn],
    pixmap: &'a mut SubPixmapMut<'b>,
    mask_ctx: super::MaskCtx<'a>,
    aa_mask_ctx: super::AAMaskCtx,
    ctx: &'a mut super::Context,
    r: u16x16,
    g: u16x16,
    b: u16x16,
    a: u16x16,
    dr: u16x16,
    dg: u16x16,
    db: u16x16,
    da: u16x16,
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

pub(super) fn stage_function(stage: super::Stage) -> Result<Option<StageFn>, console_core_never::Never> {
    use super::Stage;

    Ok(Some(match stage {
        Stage::MoveSourceToDestination => move_source_to_destination,
        Stage::MoveDestinationToSource => move_destination_to_source,
        Stage::Clamp0 => return Ok(None),
        Stage::ClampA => return Ok(None),
        Stage::Premultiply => premultiply,
        Stage::UniformColor => uniform_color,
        Stage::SeedShader => seed_shader,
        Stage::LoadDestination => load_dst,
        Stage::Store => store,
        Stage::LoadDestinationU8 => load_dst_u8,
        Stage::StoreU8 => store_u8,
        Stage::Gather => return Ok(None),
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
        Stage::ColorBurn => return Ok(None),
        Stage::ColorDodge => return Ok(None),
        Stage::Darken => darken,
        Stage::Difference => difference,
        Stage::Exclusion => exclusion,
        Stage::HardLight => hard_light,
        Stage::Lighten => lighten,
        Stage::Overlay => overlay,
        Stage::SoftLight => return Ok(None),
        Stage::Hue => return Ok(None),
        Stage::Saturation => return Ok(None),
        Stage::Color => return Ok(None),
        Stage::Luminosity => return Ok(None),
        Stage::SourceOverRgba => source_over_rgba,
        Stage::Transform => transform,
        Stage::Reflect => return Ok(None),
        Stage::Repeat => return Ok(None),
        Stage::Bilinear => return Ok(None),
        Stage::Bicubic => return Ok(None),
        Stage::PadX1 => pad_x1,
        Stage::ReflectX1 => reflect_x1,
        Stage::RepeatX1 => repeat_x1,
        Stage::Gradient => gradient,
        Stage::EvenlySpaced2StopGradient => evenly_spaced_2_stop_gradient,
        Stage::XYToUnitAngle => return Ok(None),
        Stage::XYToRadius => xy_to_radius,
        Stage::XYTo2PtConicalFocalOnCircle => return Ok(None),
        Stage::XYTo2PtConicalWellBehaved => return Ok(None),
        Stage::XYTo2PtConicalSmaller => return Ok(None),
        Stage::XYTo2PtConicalGreater => return Ok(None),
        Stage::XYTo2PtConicalStrip => return Ok(None),
        Stage::Mask2PtConicalNan => return Ok(None),
        Stage::Mask2PtConicalDegenerates => return Ok(None),
        Stage::ApplyVectorMask => return Ok(None),
        Stage::Alter2PtConicalCompensateFocal => return Ok(None),
        Stage::Alter2PtConicalUnswap => return Ok(None),
        Stage::NegateX => return Ok(None),
        Stage::ApplyConcentricScaleBias => return Ok(None),
        Stage::GammaExpand2 => return Ok(None),
        Stage::GammaExpandDestination2 => return Ok(None),
        Stage::GammaCompress2 => return Ok(None),
        Stage::GammaExpand22 => return Ok(None),
        Stage::GammaExpandDestination22 => return Ok(None),
        Stage::GammaCompress22 => return Ok(None),
        Stage::GammaExpandSrgb => return Ok(None),
        Stage::GammaExpandDestinationSrgb => return Ok(None),
        Stage::GammaCompressSrgb => return Ok(None),
    }))
}

#[inline(never)]
pub fn start(
    #[cfg(feature = "simd")] simd: console_core_simd::Simd,
    functions: &[StageFn],
    functions_tail: &[StageFn],
    rect: &ScreenIntRect,
    aa_mask_ctx: super::AAMaskCtx,
    mask_ctx: super::MaskCtx,
    ctx: &mut super::Context,
    pixmap: &mut SubPixmapMut,
) -> core::result::Result<(), console_core_never::Never> {
    let mut p = Pipeline {
        #[cfg(feature = "simd")]
        simd,
        functions: &[],
        pixmap,
        mask_ctx,
        aa_mask_ctx,
        ctx,
        r: u16x16::default(),
        g: u16x16::default(),
        b: u16x16::default(),
        a: u16x16::default(),
        dr: u16x16::default(),
        dg: u16x16::default(),
        db: u16x16::default(),
        da: u16x16::default(),
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

        for x in (left..last_full).step_by(16) {
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

    let Ok(()) = p.next_stage();

    Ok(())
}

fn move_destination_to_source(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = p.dr;
    p.g = p.dg;
    p.b = p.db;
    p.a = p.da;

    let Ok(()) = p.next_stage();

    Ok(())
}

fn premultiply(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    p.r = { let Ok(explicit_value) = div255(p.r * p.a); explicit_value };
    p.g = { let Ok(explicit_value) = div255(p.g * p.a); explicit_value };
    p.b = { let Ok(explicit_value) = div255(p.b * p.a); explicit_value };

    let Ok(()) = p.next_stage();

    Ok(())
}

fn uniform_color(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ctx = p.ctx.uniform_color;
    let Ok(splat) = u16x16::splat(match ctx.rgba { [element, ..] => element });
    p.r = splat;
    p.g = { let Ok(explicit_value) = u16x16::splat(match ctx.rgba { [_, element, ..] => element }); explicit_value };
    p.b = { let Ok(explicit_value) = u16x16::splat(match ctx.rgba { [_, _, element, ..] => element }); explicit_value };
    p.a = { let Ok(explicit_value) = u16x16::splat(match ctx.rgba { [_, _, _, element, ..] => element }); explicit_value };

    let Ok(()) = p.next_stage();

    Ok(())
}

fn seed_shader(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let iota = f32x16(
        f32x8::from([0.5,  1.5,  2.5,  3.5,  4.5,  5.5,  6.5,  7.5]),
        f32x8::from([8.5,  9.5, 10.5, 11.5, 12.5, 13.5, 14.5, 15.5]),
    );

    let Ok(splat) = f32x16::splat(match ::console_core_number_conversion::nearest_f32_from_u64(u64::from(p.position.x)) { ::core::result::Result::Ok(_converted_number) => _converted_number });
    let x = splat + iota;
    let Ok(y) = f32x16::splat((match ::console_core_number_conversion::nearest_f32_from_u64(u64::from(p.position.y)) { ::core::result::Result::Ok(_converted_number) => _converted_number }) + 0.5);
    let Ok(()) = split(&x, &mut p.r, &mut p.g);
    let Ok(()) = split(&y, &mut p.b, &mut p.a);

    let Ok(()) = p.next_stage();

    Ok(())
}

pub fn load_dst(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(found_pixels) = p.pixmap.pixels16(p.position);
    let slice16_at_xy = match found_pixels {
        Some(pixels) => pixels,
        None => return Ok(()),
    };

    let Ok(()) = load_8888(slice16_at_xy, &mut p.dr, &mut p.dg, &mut p.db, &mut p.da);
    let Ok(()) = p.next_stage();

    Ok(())
}

pub fn load_dst_tail(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(()) = load_8888_tail(p.tail, { let Ok(explicit_value) = p.pixmap.pixels_from(p.position); explicit_value }, &mut p.dr, &mut p.dg, &mut p.db, &mut p.da);
    let Ok(()) = p.next_stage();

    Ok(())
}

pub fn store(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(found_pixels) = p.pixmap.pixels16(p.position);
    let span = match found_pixels {
        Some(pixels) => pixels,
        None => return Ok(()),
    };

    let Ok(()) = store_8888(&p.r, &p.g, &p.b, &p.a, span);
    let Ok(()) = p.next_stage();

    Ok(())
}

pub fn store_tail(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(()) = store_8888_tail(&p.r, &p.g, &p.b, &p.a, p.tail, { let Ok(explicit_value) = p.pixmap.pixels_from(p.position); explicit_value });
    let Ok(()) = p.next_stage();

    Ok(())
}

pub fn load_dst_u8(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(found_pixels) = p.pixmap.mask16(p.position);
    let slice16_mask_at_xy = match found_pixels {
        Some(pixels) => pixels,
        None => return Ok(()),
    };

    let Ok(()) = load_8(slice16_mask_at_xy, &mut p.da);
    let Ok(()) = p.next_stage();

    Ok(())
}

pub fn load_dst_u8_tail(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(data) = p.pixmap.mask_from(p.position);
    let Ok(coverage) = mask_lanes(data, p.tail);
    p.da = coverage;

    p.next_stage()
}

pub fn store_u8(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(found) = p.pixmap.mask16(p.position);
    let data = match found {
        Some(data) => data,
        None => return Ok(()),
    };
    let Ok(()) = store_mask_lanes(data, &p.a, STAGE_WIDTH);

    p.next_stage()
}

pub fn store_u8_tail(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(data) = p.pixmap.mask_from(p.position);
    let Ok(()) = store_mask_lanes(data, &p.a, p.tail);

    p.next_stage()
}

fn load_mask_u8(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(data) = p.mask_ctx.from_position(p.position);
    let Ok(alpha) = mask_lanes(data, p.tail);

    p.r = u16x16::default();
    p.g = u16x16::default();
    p.b = u16x16::default();
    p.a = alpha;

    p.next_stage()
}

fn mask_u8(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(data) = p.mask_ctx.from_position(p.position);
    let Ok(c) = mask_lanes(data, p.tail);

    match ::core::primitive::bool::then_some(c == u16x16::default(), ()) { ::core::option::Option::Some(()) => {
        return Ok(());
    }, ::core::option::Option::None => {} }

    p.r = { let Ok(explicit_value) = div255(p.r * c); explicit_value };
    p.g = { let Ok(explicit_value) = div255(p.g * c); explicit_value };
    p.b = { let Ok(explicit_value) = div255(p.b * c); explicit_value };
    p.a = { let Ok(explicit_value) = div255(p.a * c); explicit_value };

    let Ok(()) = p.next_stage();

    Ok(())
}

fn scale_u8(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {

    let Ok(data) = p.aa_mask_ctx.copy_at(p.position, p.tail);
    let c = u16x16([
        u16::from(match data { [element, ..] => element }),
        u16::from(match data { [_, element, ..] => element }),
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ]);

    p.r = { let Ok(explicit_value) = div255(p.r * c); explicit_value };
    p.g = { let Ok(explicit_value) = div255(p.g * c); explicit_value };
    p.b = { let Ok(explicit_value) = div255(p.b * c); explicit_value };
    p.a = { let Ok(explicit_value) = div255(p.a * c); explicit_value };

    let Ok(()) = p.next_stage();

    Ok(())
}

fn lerp_u8(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {

    let Ok(data) = p.aa_mask_ctx.copy_at(p.position, p.tail);
    let c = u16x16([
        u16::from(match data { [element, ..] => element }),
        u16::from(match data { [_, element, ..] => element }),
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ]);

    p.r = { let Ok(explicit_value) = lerp(p.dr, p.r, c); explicit_value };
    p.g = { let Ok(explicit_value) = lerp(p.dg, p.g, c); explicit_value };
    p.b = { let Ok(explicit_value) = lerp(p.db, p.b, c); explicit_value };
    p.a = { let Ok(explicit_value) = lerp(p.da, p.a, c); explicit_value };

    let Ok(()) = p.next_stage();

    Ok(())
}

fn scale_1_float(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(c) = from_float(p.ctx.current_coverage);
    p.r = { let Ok(explicit_value) = div255(p.r * c); explicit_value };
    p.g = { let Ok(explicit_value) = div255(p.g * c); explicit_value };
    p.b = { let Ok(explicit_value) = div255(p.b * c); explicit_value };
    p.a = { let Ok(explicit_value) = div255(p.a * c); explicit_value };

    let Ok(()) = p.next_stage();

    Ok(())
}

fn lerp_1_float(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(c) = from_float(p.ctx.current_coverage);
    p.r = { let Ok(explicit_value) = lerp(p.dr, p.r, c); explicit_value };
    p.g = { let Ok(explicit_value) = lerp(p.dg, p.g, c); explicit_value };
    p.b = { let Ok(explicit_value) = lerp(p.db, p.b, c); explicit_value };
    p.a = { let Ok(explicit_value) = lerp(p.da, p.a, c); explicit_value };

    let Ok(()) = p.next_stage();

    Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the integer compositing modes share the same four lane writes and dispatch, with each formula listed at its call site"
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

blend_fn!(clear,            |_, _,  _,  _| { let Ok(explicit_value) = u16x16::splat(0); explicit_value });
blend_fn!(source_atop,      |s, d, sa, da| { let Ok(explicit_value) = div255(s * da + d * ({ let Ok(explicit_value) = inv(sa); explicit_value })); explicit_value });
blend_fn!(destination_atop, |s, d, sa, da| { let Ok(explicit_value) = div255(d * sa + s * ({ let Ok(explicit_value) = inv(da); explicit_value })); explicit_value });
blend_fn!(source_in,        |s, _,  _, da| { let Ok(explicit_value) = div255(s * da); explicit_value });
blend_fn!(destination_in,   |_, d, sa,  _| { let Ok(explicit_value) = div255(d * sa); explicit_value });
blend_fn!(source_out,       |s, _,  _, da| { let Ok(explicit_value) = div255(s * ({ let Ok(explicit_value) = inv(da); explicit_value })); explicit_value });
blend_fn!(destination_out,  |_, d, sa,  _| { let Ok(explicit_value) = div255(d * ({ let Ok(explicit_value) = inv(sa); explicit_value })); explicit_value });
blend_fn!(source_over,      |s, d, sa,  _| s + ({ let Ok(explicit_value) = div255(d * ({ let Ok(explicit_value) = inv(sa); explicit_value })); explicit_value }));
blend_fn!(destination_over, |s, d,  _, da| d + ({ let Ok(explicit_value) = div255(s * ({ let Ok(explicit_value) = inv(da); explicit_value })); explicit_value }));
blend_fn!(modulate,         |s, d,  _,  _| { let Ok(explicit_value) = div255(s * d); explicit_value });
blend_fn!(multiply,         |s, d, sa, da| { let Ok(explicit_value) = div255(s * ({ let Ok(explicit_value) = inv(da); explicit_value }) + d * ({ let Ok(explicit_value) = inv(sa); explicit_value }) + s * d); explicit_value });
blend_fn!(screen,           |s, d,  _,  _| s + d - ({ let Ok(explicit_value) = div255(s * d); explicit_value }));
blend_fn!(xor,              |s, d, sa, da| { let Ok(explicit_value) = div255(s * ({ let Ok(explicit_value) = inv(da); explicit_value }) + d * ({ let Ok(explicit_value) = inv(sa); explicit_value })); explicit_value });

blend_fn!(plus, |s: u16x16, d, _, _| { let Ok(explicit_value) = (s + d).min(&({ let Ok(explicit_value) = u16x16::splat(255); explicit_value })); explicit_value });

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the integer separable modes share the alpha equation and lane writes, with their distinct formula visible at each call"
  )
)]
macro_rules! blend_fn2 {
    ($name:ident, $f:expr) => {
        fn $name(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {

            p.r = $f(p.r, p.dr, p.a, p.da);
            p.g = $f(p.g, p.dg, p.a, p.da);
            p.b = $f(p.b, p.db, p.a, p.da);
            p.a = p.a + ({ let Ok(value) = div255(p.da * ({ let Ok(value) = inv(p.a); value })); value });

            let Ok(()) = p.next_stage();

            Ok(())
        }
    };
}

blend_fn2!(darken,      |s: u16x16, d, sa, da| s + d - ({ let Ok(explicit_value) = div255({ let Ok(explicit_value) = (s * da).max(&(d * sa)); explicit_value }); explicit_value }));
blend_fn2!(lighten,     |s: u16x16, d, sa, da| s + d - ({ let Ok(explicit_value) = div255({ let Ok(explicit_value) = (s * da).min(&(d * sa)); explicit_value }); explicit_value }));
blend_fn2!(exclusion,   |s: u16x16, d,  _,  _| s + d - ({ let Ok(explicit_value) = u16x16::splat(2); explicit_value }) * ({ let Ok(explicit_value) = div255(s * d); explicit_value }));

blend_fn2!(difference,  |s: u16x16, d, sa, da|
    s + d - ({ let Ok(explicit_value) = u16x16::splat(2); explicit_value }) * ({ let Ok(explicit_value) = div255({ let Ok(explicit_value) = (s * da).min(&(d * sa)); explicit_value }); explicit_value }));

blend_fn2!(hard_light, |s: u16x16, d: u16x16, sa, da| {
    { let Ok(explicit_value) = div255(s * ({ let Ok(explicit_value) = inv(da); explicit_value }) + d * ({ let Ok(explicit_value) = inv(sa); explicit_value })
        + ({ let Ok(explicit_value) = ({ let Ok(explicit_value) = (s+s).cmp_le(&sa); explicit_value }).blend(
            ({ let Ok(explicit_value) = u16x16::splat(2); explicit_value }) * s * d,
            sa * da - ({ let Ok(explicit_value) = u16x16::splat(2); explicit_value }) * (sa-s)*(da-d)
        ); explicit_value })
    ); explicit_value }
});

blend_fn2!(overlay, |s: u16x16, d: u16x16, sa, da| {
    { let Ok(explicit_value) = div255(s * ({ let Ok(explicit_value) = inv(da); explicit_value }) + d * ({ let Ok(explicit_value) = inv(sa); explicit_value })
        + ({ let Ok(explicit_value) = ({ let Ok(explicit_value) = (d+d).cmp_le(&da); explicit_value }).blend(
            ({ let Ok(explicit_value) = u16x16::splat(2); explicit_value }) * s * d,
            sa * da - ({ let Ok(explicit_value) = u16x16::splat(2); explicit_value }) * (sa-s)*(da-d)
        ); explicit_value })
    ); explicit_value }
});

pub fn source_over_rgba(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(found_pixels) = p.pixmap.pixels16(p.position);
    let pixels = match found_pixels {
        Some(pixels) => pixels,
        None => return Ok(()),
    };

    #[cfg(feature = "simd")]
    match p.functions.is_empty().then_some(()) {
        Some(()) => {
            let Ok(compositing) = p.simd.source_over_rgba16(
            [&p.r.0, &p.g.0, &p.b.0, &p.a.0],
            bytemuck::cast_mut(pixels),
            );

            match compositing {
                console_core_simd::RgbaCompositing::Applied => return Ok(()),
                console_core_simd::RgbaCompositing::Unavailable => {}
            }
        }
        None => {}
    }

    let Ok(()) = load_8888(pixels, &mut p.dr, &mut p.dg, &mut p.db, &mut p.da);
    p.r = p.r + ({ let Ok(explicit_value) = div255(p.dr * ({ let Ok(explicit_value) = inv(p.a); explicit_value })); explicit_value });
    p.g = p.g + ({ let Ok(explicit_value) = div255(p.dg * ({ let Ok(explicit_value) = inv(p.a); explicit_value })); explicit_value });
    p.b = p.b + ({ let Ok(explicit_value) = div255(p.db * ({ let Ok(explicit_value) = inv(p.a); explicit_value })); explicit_value });
    p.a = p.a + ({ let Ok(explicit_value) = div255(p.da * ({ let Ok(explicit_value) = inv(p.a); explicit_value })); explicit_value });
    let Ok(()) = store_8888(&p.r, &p.g, &p.b, &p.a, pixels);

    let Ok(()) = p.next_stage();

    Ok(())
}

pub fn source_over_rgba_tail(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(pixels) = p.pixmap.pixels_from(p.position);
    let Ok(()) = load_8888_tail(p.tail, pixels, &mut p.dr, &mut p.dg, &mut p.db, &mut p.da);
    p.r = p.r + ({ let Ok(explicit_value) = div255(p.dr * ({ let Ok(explicit_value) = inv(p.a); explicit_value })); explicit_value });
    p.g = p.g + ({ let Ok(explicit_value) = div255(p.dg * ({ let Ok(explicit_value) = inv(p.a); explicit_value })); explicit_value });
    p.b = p.b + ({ let Ok(explicit_value) = div255(p.db * ({ let Ok(explicit_value) = inv(p.a); explicit_value })); explicit_value });
    p.a = p.a + ({ let Ok(explicit_value) = div255(p.da * ({ let Ok(explicit_value) = inv(p.a); explicit_value })); explicit_value });
    let Ok(()) = store_8888_tail(&p.r, &p.g, &p.b, &p.a, p.tail, pixels);

    let Ok(()) = p.next_stage();

    Ok(())
}

fn transform(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let ts = &p.ctx.transform;

    let Ok(x) = join(&p.r, &p.g);
    let Ok(y) = join(&p.b, &p.a);

    let Ok(splat) = f32x16::splat(ts.sx);
    let Ok(nx) = mad(x, splat, { let Ok(explicit_value) = mad(y, { let Ok(explicit_value) = f32x16::splat(ts.kx); explicit_value }, { let Ok(explicit_value) = f32x16::splat(ts.tx); explicit_value }); explicit_value });
    let Ok(ny) = mad(x, { let Ok(explicit_value) = f32x16::splat(ts.ky); explicit_value }, { let Ok(explicit_value) = mad(y, { let Ok(explicit_value) = f32x16::splat(ts.sy); explicit_value }, { let Ok(explicit_value) = f32x16::splat(ts.ty); explicit_value }); explicit_value });

    let Ok(()) = split(&nx, &mut p.r, &mut p.g);
    let Ok(()) = split(&ny, &mut p.b, &mut p.a);

    let Ok(()) = p.next_stage();

    Ok(())
}

fn pad_x1(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(x) = join(&p.r, &p.g);
    let Ok(x_2) = x.normalize();
    let Ok(()) = split(&x_2, &mut p.r, &mut p.g);

    let Ok(()) = p.next_stage();

    Ok(())
}

fn reflect_x1(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(x) = join(&p.r, &p.g);
    let two = |x_2| x_2 + x_2;
    let Ok(splat) = f32x16::splat(1.0);
    let Ok(x_3) = ({ let Ok(explicit_value) = (
        (x - splat)
        - two({ let Ok(explicit_value) = ((x - ({ let Ok(explicit_value) = f32x16::splat(1.0); explicit_value })) * ({ let Ok(explicit_value) = f32x16::splat(0.5); explicit_value })).floor(); explicit_value })
        - ({ let Ok(explicit_value) = f32x16::splat(1.0); explicit_value })
    ).abs(); explicit_value }).normalize();
    let Ok(()) = split(&x_3, &mut p.r, &mut p.g);

    let Ok(()) = p.next_stage();

    Ok(())
}

fn repeat_x1(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(x) = join(&p.r, &p.g);
    let Ok(x_2) = (x - ({ let Ok(explicit_value) = x.floor(); explicit_value })).normalize();
    let Ok(()) = split(&x_2, &mut p.r, &mut p.g);

    let Ok(()) = p.next_stage();

    Ok(())
}

fn gradient(p: &mut Pipeline) -> Result<(), console_core_never::Never> {
    let bands = &p.ctx.gradient;
    let Ok(t) = join(&p.r, &p.g);
    let coordinates: Lanes<f32> = bytemuck::cast(t);
    let mut indices = [0u16; 16];

    for band in bands.iter().skip(1) {
        let threshold = band.start.get();

        for (slot, coordinate) in indices.iter_mut().zip(coordinates) {
            *slot = slot.wrapping_add(u16::from(coordinate >= threshold));
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
    let Ok(()) = round_f32_to_u16(r, g, b, a, &mut p.r, &mut p.g, &mut p.b, &mut p.a);

    p.next_stage()
}

fn gradient_channels(entries: Lanes<super::GradientColor>) -> Result<[f32x16; 4], console_core_never::Never> {
    let packed: [[f32; 16]; 4] = [
        entries.map(|super::GradientColor([r, _, _, _])| r),
        entries.map(|super::GradientColor([_, g, _, _])| g),
        entries.map(|super::GradientColor([_, _, b, _])| b),
        entries.map(|super::GradientColor([_, _, _, a])| a),
    ];

    Ok(packed.map(bytemuck::cast))
}

fn interpolate_channels(
    t: f32x16,
    [red, green, blue, alpha]: [f32x16; 4],
    [red_bias, green_bias, blue_bias, alpha_bias]: [f32x16; 4],
) -> Result<[f32x16; 4], console_core_never::Never> {
    let Ok(r) = mad(t, red, red_bias);
    let Ok(g) = mad(t, green, green_bias);
    let Ok(b) = mad(t, blue, blue_bias);
    let Ok(a) = mad(t, alpha, alpha_bias);

    Ok([r, g, b, a])
}

fn evenly_spaced_2_stop_gradient(p: &mut Pipeline) -> Result<(), console_core_never::Never> {
    let ctx = p.ctx.evenly_spaced_2_stop_gradient;
    let Ok(t) = join(&p.r, &p.g);
    let factors = ctx.factor.0.map(|value| {
        let Ok(splat) = f32x16::splat(value);

        splat
    });
    let biases = ctx.bias.0.map(|value| {
        let Ok(splat) = f32x16::splat(value);

        splat
    });
    let Ok([r, g, b, a]) = interpolate_channels(t, factors, biases);
    let Ok(()) = round_f32_to_u16(r, g, b, a, &mut p.r, &mut p.g, &mut p.b, &mut p.a);

    p.next_stage()
}

fn xy_to_radius(p: &mut Pipeline) -> core::result::Result<(), console_core_never::Never> {
    let Ok(x) = join(&p.r, &p.g);
    let Ok(y) = join(&p.b, &p.a);
    let Ok(x_2) = (x*x + y*y).sqrt();
    let Ok(()) = split(&x_2, &mut p.r, &mut p.g);
    let Ok(()) = split(&y, &mut p.b, &mut p.a);

    let Ok(()) = p.next_stage();

    Ok(())
}

#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "four float lanes in and the four integer lanes they round into, which the Pipeline keeps as separate fields")]
fn round_f32_to_u16(
    rf: f32x16, gf: f32x16, bf: f32x16, af: f32x16,
    r: &mut u16x16, g: &mut u16x16, b: &mut u16x16, a: &mut u16x16,
) -> core::result::Result<(), console_core_never::Never> {
    let Ok(normalize) = rf.normalize();
    let Ok(splat) = f32x16::splat(255.0);
    let rf_2 = normalize * splat + ({ let Ok(explicit_value) = f32x16::splat(0.5); explicit_value });
    let Ok(gf_normalize) = gf.normalize();
    let gf_2 = gf_normalize * ({ let Ok(explicit_value) = f32x16::splat(255.0); explicit_value }) + ({ let Ok(explicit_value) = f32x16::splat(0.5); explicit_value });
    let Ok(bf_normalize) = bf.normalize();
    let bf_2 = bf_normalize * ({ let Ok(explicit_value) = f32x16::splat(255.0); explicit_value }) + ({ let Ok(explicit_value) = f32x16::splat(0.5); explicit_value });
    let af_2 = af * ({ let Ok(explicit_value) = f32x16::splat(255.0); explicit_value }) + ({ let Ok(explicit_value) = f32x16::splat(0.5); explicit_value });

    let Ok(()) = rf_2.save_to_u16x16(r);
    let Ok(()) = gf_2.save_to_u16x16(g);
    let Ok(()) = bf_2.save_to_u16x16(b);
    let Ok(()) = af_2.save_to_u16x16(a);

    Ok(())
}

#[inline(always)]
fn load_8888(
    data: &Lanes<PremultipliedColorU8>,
    r: &mut u16x16, g: &mut u16x16, b: &mut u16x16, a: &mut u16x16,
) -> core::result::Result<(), console_core_never::Never> {
    *r = u16x16(data.map(|pixel| {
        let Ok(value) = pixel.red();

        u16::from(value)
    }));

    *g = u16x16(data.map(|pixel| {
        let Ok(value) = pixel.green();

        u16::from(value)
    }));

    *b = u16x16(data.map(|pixel| {
        let Ok(value) = pixel.blue();

        u16::from(value)
    }));

    *a = u16x16(data.map(|pixel| {
        let Ok(value) = pixel.alpha();

        u16::from(value)
    }));

    Ok(())
}

#[inline(always)]
fn load_8888_tail(
    tail: u32, data: &[PremultipliedColorU8],
    r: &mut u16x16, g: &mut u16x16, b: &mut u16x16, a: &mut u16x16,
) -> core::result::Result<(), console_core_never::Never> {
    let mut ready = [PremultipliedColorU8::TRANSPARENT; 16];
    let Ok(lanes) = console_core_number_conversion::index(tail);

    for (pixel, source) in ready.iter_mut().zip(data).take(lanes) {
        *pixel = *source;
    }

    load_8888(&ready, r, g, b, a)
}

#[inline(always)]
fn store_8888(
    r: &u16x16, g: &u16x16, b: &u16x16, a: &u16x16,
    data: &mut Lanes<PremultipliedColorU8>,
) -> core::result::Result<(), console_core_never::Never> {
    store_8888_tail(r, g, b, a, STAGE_WIDTH, data)
}

#[inline(always)]
fn store_8888_tail(
    r: &u16x16, g: &u16x16, b: &u16x16, a: &u16x16,
    tail: u32, data: &mut [PremultipliedColorU8],
) -> core::result::Result<(), console_core_never::Never> {
    let Ok(red) = r.as_slice();
    let Ok(green) = g.as_slice();
    let Ok(blue) = b.as_slice();
    let Ok(alpha) = a.as_slice();
    let Ok(lanes) = console_core_number_conversion::index(tail);
    let channels = red.iter().zip(green.iter()).zip(blue.iter().zip(alpha.iter()));

    for (destination, ((&red_3, &green_3), (&blue_3, &alpha_3))) in data.iter_mut().zip(channels).take(lanes) {
        let Ok(red_2) = console_core_number_conversion::wrapped::<u16, u8>(red_3);
        let Ok(green_2) = console_core_number_conversion::wrapped::<u16, u8>(green_3);
        let Ok(blue_2) = console_core_number_conversion::wrapped::<u16, u8>(blue_3);
        let Ok(alpha_2) = console_core_number_conversion::wrapped::<u16, u8>(alpha_3);
        let Ok(pixel) = PremultipliedColorU8::from_components([red_2, green_2, blue_2, alpha_2]);

        *destination = pixel;
    }

    Ok(())
}

#[inline(always)]
fn load_8(data: &Lanes<u8>, a: &mut u16x16) -> core::result::Result<(), console_core_never::Never> {
    *a = u16x16(data.map(u16::from));

    Ok(())
}

#[inline(always)]
fn mask_lanes(data: &[u8], total: u32) -> Result<u16x16, console_core_never::Never> {
    let mut lanes = [0; 16];
    let Ok(count) = console_core_number_conversion::index(total);

    for (lane, &alpha) in lanes.iter_mut().zip(data).take(count) {
        *lane = u16::from(alpha);
    }

    Ok(u16x16(lanes))
}

#[inline(always)]
fn store_mask_lanes(data: &mut [u8], source: &u16x16, total: u32) -> Result<(), console_core_never::Never> {
    let Ok(lanes) = source.as_slice();
    let Ok(count) = console_core_number_conversion::index(total);

    for (slot, &alpha) in data.iter_mut().zip(lanes).take(count) {
        let Ok(value) = console_core_number_conversion::wrapped::<u16, u8>(alpha);

        *slot = value;
    }

    Ok(())
}

#[inline(always)]
fn div255(v: u16x16) -> core::result::Result<u16x16, console_core_never::Never> {
    let Ok(splat) = u16x16::splat(255);

    Ok((v + splat) >> ({ let Ok(explicit_value) = u16x16::splat(8); explicit_value }))
}

#[inline(always)]
fn inv(v: u16x16) -> core::result::Result<u16x16, console_core_never::Never> {
    let Ok(splat) = u16x16::splat(255);

    Ok(splat - v)
}

#[inline(always)]
fn from_float(coverage: NormalizedF32) -> core::result::Result<u16x16, console_core_never::Never> {
    let Ok(splat) = u16x16::splat({ let Ok(numeric_value) = console_core_number_conversion::toward_zero_u16(f64::from(coverage.get() * 255.0 + 0.5)); numeric_value });

    Ok(splat)
}

#[inline(always)]
fn lerp(from: u16x16, to: u16x16, t: u16x16) -> core::result::Result<u16x16, console_core_never::Never> {
    Ok({ let Ok(explicit_value) = div255(from * ({ let Ok(explicit_value) = inv(t); explicit_value }) + to * t); explicit_value })
}

#[inline(always)]
fn split(v: &f32x16, lo: &mut u16x16, hi: &mut u16x16) -> Result<(), console_core_never::Never> {
    let [low, high]: [[u16; 16]; 2] = bytemuck::cast(*v);
    lo.0 = low;
    hi.0 = high;

    Ok(())
}

#[inline(always)]
fn join(lo: &u16x16, hi: &u16x16) -> Result<f32x16, console_core_never::Never> {
    Ok(bytemuck::cast([lo.0, hi.0]))
}

#[inline(always)]
fn mad(f: f32x16, m: f32x16, a: f32x16) -> core::result::Result<f32x16, console_core_never::Never> {
    Ok(f * m + a)
}
