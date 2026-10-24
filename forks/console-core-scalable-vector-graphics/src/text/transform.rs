// Copyright 2025 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub(crate) fn skrifa_to_tsp_transform(t: skrifa::color::Transform) -> console_core_paths::Transform {
    console_core_paths::Transform::from_row(t.xx, t.yx, t.xy, t.yy, t.dx, t.dy)
}

pub(crate) fn tsp_to_skrifa_transform(t: console_core_paths::Transform) -> skrifa::color::Transform {
    skrifa::color::Transform {
        xx: t.sx,
        yx: t.ky,
        xy: t.kx,
        yy: t.sy,
        dx: t.tx,
        dy: t.ty,
    }
}
