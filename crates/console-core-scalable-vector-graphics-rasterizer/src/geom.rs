//! Copyright 2023 the Resvg Authors.
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use console_core_never::Never;
use console_core_path_rasterizer::IntRect;

pub(super) fn fit_to_rect(rect: IntRect, bounds: IntRect) -> Result<Option<IntRect>, Never> {
  rect.intersect(&bounds)
}
