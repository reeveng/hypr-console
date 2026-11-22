use std::fmt::{Display, Write};
use std::ops::MulAssign;

use crate::core::{
    _c, Drawable, FillStyle, OpSet, OpSetType, OpType, Options, OptionsBuilder, PathInfo,
    RoughJsSeed, RoughMathRandom, RoughRandomness,
};
use crate::curve_points::{curve_to_bezier, points_on_bezier_curves};
use crate::geometry::{convert_bezier_quadratic_to_cubic, BezierQuadratic};
use crate::points_on_path::{points_on_path, points_on_segments};
use crate::renderer::{
    bezier_cubic, bezier_quadratic, curve, ellipse_with_params, generate_ellipse_params, line,
    linear_path, pattern_fill_arc, pattern_fill_polygons, rectangle, solid_fill_polygon, svg_path,
    svg_segments, ArcParams, ArcRenderParams,
};
use crate::work::{WorkControl, WorkError};
use crate::SvgPathSegment;
use euclid::default::Point2D;
use euclid::Trig;
use num_traits::{Float, FromPrimitive};

pub struct Generator {
    default_options: Options,
}

impl Default for Generator {
    fn default() -> Self {
        Self {
            default_options: OptionsBuilder::default()
                .randomness(RoughRandomness::new(
                    RoughJsSeed::new(345.0),
                    RoughMathRandom::new(345),
                ))
                .build()
                .expect("failed to build default options"),
        }
    }
}

impl Generator {
    pub fn line_with_control<F: Float + Trig + FromPrimitive>(
        &self,
        x1: F,
        y1: F,
        x2: F,
        y2: F,
        options: &Option<Options>,
        control: &mut impl WorkControl,
    ) -> Result<Drawable<F>, WorkError> {
        let mut resolved = options
            .clone()
            .unwrap_or_else(|| self.default_options.clone());
        let outline = crate::renderer::line_with_control(x1, y1, x2, y2, &mut resolved, control)?;
        Ok(self.d("line", &[outline], options))
    }

    pub fn rectangle_with_control<F: Float + Trig + FromPrimitive>(
        &self,
        x: F,
        y: F,
        width: F,
        height: F,
        options: &Option<Options>,
        control: &mut impl WorkControl,
    ) -> Result<Drawable<F>, WorkError> {
        let mut options = options
            .clone()
            .unwrap_or_else(|| self.default_options.clone());
        let mut paths = Vec::new();
        let outline =
            crate::renderer::rectangle_with_control(x, y, width, height, &mut options, control)?;

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.fill.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                control.charge(4)?;
                let mut polygons = vec![vec![
                    Point2D::new(x, y),
                    Point2D::new(x + width, y),
                    Point2D::new(x + width, y + height),
                    Point2D::new(x, y + height),
                ]];
                paths.push(match {
                    let _explicit_decision = ::core::primitive::bool::then_some(options.fill_style == Some(FillStyle::Solid), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        crate::renderer::solid_fill_polygon_with_control(&polygons, &mut options, control)?
                    },
                    ::core::option::Option::None => {
                        crate::renderer::hachure_fill_polygons_with_control(
                            &mut polygons,
                            &mut options,
                            control,
                        )?
                    },
                });
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.stroke.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                paths.push(outline);
            },
            ::core::option::Option::None => {},
        }

        Ok(Drawable {
            shape: "rectangle".into(),
            options,
            sets: paths,
        })
    }

    pub fn ellipse_with_control<F: Float + Trig + FromPrimitive>(
        &self,
        x: F,
        y: F,
        width: F,
        height: F,
        options: &Option<Options>,
        control: &mut impl WorkControl,
    ) -> Result<Drawable<F>, WorkError> {
        let mut options = options
            .clone()
            .unwrap_or_else(|| self.default_options.clone());
        let mut paths = Vec::new();
        let params = crate::renderer::generate_ellipse_params_with_control(
            width,
            height,
            &mut options,
            control,
        )?;
        let outline = crate::renderer::ellipse_with_params_with_control(
            x,
            y,
            &mut options,
            &params,
            control,
        )?;

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.fill.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                paths.push(match {
                    let _explicit_decision = ::core::primitive::bool::then_some(options.fill_style == Some(FillStyle::Solid), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        let mut fill = crate::renderer::ellipse_with_params_with_control(
                            x,
                            y,
                            &mut options,
                            &params,
                            control,
                        )?
                        .opset;
                        fill.op_set_type = OpSetType::FillPath;
                        fill
                    },
                    ::core::option::Option::None => {
                        crate::renderer::hachure_fill_polygons_with_control(
                            &mut [outline.estimated_points],
                            &mut options,
                            control,
                        )?
                    },
                });
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.stroke.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                paths.push(outline.opset);
            },
            ::core::option::Option::None => {},
        }

        Ok(Drawable {
            shape: "ellipse".into(),
            options,
            sets: paths,
        })
    }

    pub fn circle_with_control<F: Float + Trig + FromPrimitive>(
        &self,
        x: F,
        y: F,
        diameter: F,
        options: &Option<Options>,
        control: &mut impl WorkControl,
    ) -> Result<Drawable<F>, WorkError> {
        let mut result = self.ellipse_with_control(x, y, diameter, diameter, options, control)?;
        result.shape = "circle".into();
        Ok(result)
    }

    pub fn path_with_control<F>(
        &self,
        path: &str,
        options: &Option<Options>,
        control: &mut impl WorkControl,
    ) -> Result<Drawable<F>, WorkError>
    where
        F: Float + Trig + FromPrimitive + MulAssign + Display,
    {
        let mut options = options
            .clone()
            .unwrap_or_else(|| self.default_options.clone());
        let mut paths = Vec::new();
        control.charge(1)?;
        crate::renderer::validate_options(&options)?;

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(!path.is_empty(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                let simplified = options.simplification.is_some_and(|value| value < 1.0);
                let distance = match ::core::primitive::bool::then_some(simplified, ()) {
                    ::core::option::Option::Some(()) => {
                        _c::<F>(4.0) - _c::<F>(4.0) * _c::<F>(options.simplification.unwrap())
                    },
                    ::core::option::Option::None => {
                        (_c::<F>(1.0) + _c::<F>(options.roughness.unwrap_or(1.0))) / _c::<F>(2.0)
                    },
                };
                let segments = crate::points_on_path::normalized_path_with_control(path, control)?;
                let sets = crate::points_on_path::points_on_normalized_segments_with_control(
                    &segments,
                    Some(_c(1.0)),
                    Some(distance),
                    control,
                )?;

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(options.fill.is_some(), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        paths.push(match {
                            let _explicit_decision = ::core::primitive::bool::then_some(options.fill_style == Some(FillStyle::Solid), ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                crate::renderer::solid_fill_polygon_with_control(&sets, &mut options, control)?
                            },
                            ::core::option::Option::None => {
                                let mut polygons = Vec::new();

                                for set in &sets {
                                    control.charge(1)?;
                                    let mut polygon = Vec::new();

                                    for point in set {
                                        control.charge(1)?;
                                        polygon.push(*point);
                                    }

                                    polygons.push(polygon);
                                }

                                crate::renderer::hachure_fill_polygons_with_control(
                                    &mut polygons,
                                    &mut options,
                                    control,
                                )?
                            },
                        });
                    },
                    ::core::option::Option::None => {},
                }

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(options.stroke.is_some(), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        match ::core::primitive::bool::then_some(simplified, ()) {
                            ::core::option::Option::Some(()) => {
                                for set in &sets {
                                    control.charge(1)?;
                                    paths.push(crate::renderer::linear_path_with_control(
                                        set,
                                        false,
                                        &mut options,
                                        control,
                                    )?);
                                }
                            },
                            ::core::option::Option::None => {
                                paths.push(crate::renderer::svg_normalized_segments_with_control(
                                    &segments,
                                    &mut options,
                                    control,
                                )?);
                            },
                        }
                    },
                    ::core::option::Option::None => {},
                }
            },
            ::core::option::Option::None => {},
        }

        Ok(Drawable {
            shape: "path".into(),
            options,
            sets: paths,
        })
    }

    #[must_use]
    fn d<T, F>(&self, name: T, op_sets: &[OpSet<F>], options: &Option<Options>) -> Drawable<F>
    where
        T: Into<String>,
        F: Float + Trig + FromPrimitive,
    {
        Drawable {
            shape: name.into(),
            options: options
                .clone()
                .unwrap_or_else(|| self.default_options.clone()),
            sets: Vec::from_iter(op_sets.iter().cloned()),
        }
    }

    #[must_use]
    pub fn line<F>(&self, x1: F, y1: F, x2: F, y2: F, options: &Option<Options>) -> Drawable<F>
    where
        F: Float + Trig + FromPrimitive,
    {
        self.d(
            "line",
            &[line(
                x1,
                y1,
                x2,
                y2,
                &mut options
                    .clone()
                    .unwrap_or_else(|| self.default_options.clone()),
            )],
            options,
        )
    }

    #[must_use]
    pub fn rectangle<F>(
        &self,
        x: F,
        y: F,
        width: F,
        height: F,
        options: &Option<Options>,
    ) -> Drawable<F>
    where
        F: Float + Trig + FromPrimitive,
    {
        let mut paths = vec![];
        let mut options = options
            .clone()
            .unwrap_or_else(|| self.default_options.clone());
        let outline = rectangle(x, y, width, height, &mut options);

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.fill.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                let points = vec![
                    Point2D::new(x, y),
                    Point2D::new(x + width, y),
                    Point2D::new(x + width, y + height),
                    Point2D::new(x, y + height),
                ];
                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(options.fill_style == Some(FillStyle::Solid), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        paths.push(solid_fill_polygon(&vec![points], &mut options));
                    },
                    ::core::option::Option::None => {
                        paths.push(pattern_fill_polygons(vec![points], &mut options));
                    },
                }
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.stroke.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                paths.push(outline);
            },
            ::core::option::Option::None => {},
        }

        self.d("rectangle", &paths, &Some(options))
    }

    #[must_use]
    pub fn ellipse<F>(
        &self,
        x: F,
        y: F,
        width: F,
        height: F,
        options: &Option<Options>,
    ) -> Drawable<F>
    where
        F: Float + Trig + FromPrimitive,
    {
        let mut paths = vec![];
        let mut options = options
            .clone()
            .unwrap_or_else(|| self.default_options.clone());
        let ellipse_params = generate_ellipse_params(width, height, &mut options);
        let ellipse_response = ellipse_with_params(x, y, &mut options, &ellipse_params);

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.fill.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(options.fill_style == Some(FillStyle::Solid), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        let mut shape = ellipse_with_params(x, y, &mut options, &ellipse_params).opset;
                        shape.op_set_type = OpSetType::FillPath;
                        paths.push(shape);
                    },
                    ::core::option::Option::None => {
                        paths.push(pattern_fill_polygons(
                            vec![ellipse_response.estimated_points],
                            &mut options,
                        ));
                    },
                }
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.stroke.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                paths.push(ellipse_response.opset);
            },
            ::core::option::Option::None => {},
        }

        self.d("ellipse", &paths, &Some(options))
    }

    #[must_use]
    pub fn circle<F>(&self, x: F, y: F, diameter: F, options: &Option<Options>) -> Drawable<F>
    where
        F: Float + Trig + FromPrimitive,
    {
        let mut shape = self.ellipse(x, y, diameter, diameter, options);
        shape.shape = "circle".into();
        shape
    }

    #[must_use]
    pub fn linear_path<F>(
        &self,
        points: &[Point2D<F>],
        close: bool,
        options: &Option<Options>,
    ) -> Drawable<F>
    where
        F: Float + Trig + FromPrimitive,
    {
        let mut options = options
            .clone()
            .unwrap_or_else(|| self.default_options.clone());
        self.d(
            "linear_path",
            &[linear_path(points, close, &mut options)],
            &Some(options),
        )
    }

    #[must_use]
    pub fn arc<F>(&self, arc: ArcParams<F>, options: &Option<Options>) -> Drawable<F>
    where
        F: Float + Trig + FromPrimitive,
    {
        let mut options = options
            .clone()
            .unwrap_or_else(|| self.default_options.clone());
        let mut paths = vec![];
        let outline = crate::renderer::arc(
            ArcRenderParams {
                arc,
                rough_closure: true,
            },
            &mut options,
        );

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(arc.closed && options.fill.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(options.fill_style == Some(FillStyle::Solid), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        options.disable_multi_stroke = Some(true);
                        let mut shape = crate::renderer::arc(
                            ArcRenderParams {
                                arc,
                                rough_closure: false,
                            },
                            &mut options,
                        );
                        shape.op_set_type = OpSetType::FillPath;
                        paths.push(shape);
                    },
                    ::core::option::Option::None => {
                        paths.push(pattern_fill_arc(
                            arc.x,
                            arc.y,
                            arc.width,
                            arc.height,
                            arc.start,
                            arc.stop,
                            &mut options,
                        ));
                    },
                }
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.stroke.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                paths.push(outline);
            },
            ::core::option::Option::None => {},
        }

        self.d("arc", &paths, &Some(options))
    }

    #[must_use]
    pub fn bezier_quadratic<F>(
        &self,
        start: Point2D<F>,
        cp: Point2D<F>,
        end: Point2D<F>,
        options: &Option<Options>,
    ) -> Drawable<F>
    where
        F: Float + Trig + FromPrimitive + MulAssign + Display,
    {
        let mut paths = vec![];
        let mut options = options
            .clone()
            .unwrap_or_else(|| self.default_options.clone());

        let outline = bezier_quadratic(start, cp, end, &mut options);

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.fill.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                // The fill algorithms expect at least 4 points of a cubic curve, else they panic
                let cubic = convert_bezier_quadratic_to_cubic(BezierQuadratic { start, cp, end });
                let crv = vec![cubic.start, cubic.cp1, cubic.cp2, cubic.end];

                let poly_points = points_on_bezier_curves(
                    &crv,
                    _c(10.0),
                    Some(_c::<F>(1.0) + _c::<F>(options.roughness.unwrap_or(0.0)) / _c(2.0)),
                );

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(options.fill_style == Some(FillStyle::Solid), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        paths.push(solid_fill_polygon(&vec![poly_points], &mut options));
                    },
                    ::core::option::Option::None => {
                        paths.push(pattern_fill_polygons(&mut vec![poly_points], &mut options));
                    },
                }
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.stroke.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                paths.push(outline);
            },
            ::core::option::Option::None => {},
        }

        self.d("curve", &paths, &Some(options))
    }

    #[must_use]
    pub fn bezier_cubic<F>(
        &self,
        start: Point2D<F>,
        cp1: Point2D<F>,
        cp2: Point2D<F>,
        end: Point2D<F>,
        options: &Option<Options>,
    ) -> Drawable<F>
    where
        F: Float + Trig + FromPrimitive + MulAssign + Display,
    {
        let mut paths = vec![];
        let mut options = options
            .clone()
            .unwrap_or_else(|| self.default_options.clone());

        let outline = bezier_cubic(start, cp1, cp2, end, &mut options);

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.fill.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                let crv = vec![start, cp1, cp2, end];

                let poly_points = points_on_bezier_curves(
                    &crv,
                    _c(10.0),
                    Some(_c::<F>(1.0) + _c::<F>(options.roughness.unwrap_or(0.0)) / _c(2.0)),
                );

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(options.fill_style == Some(FillStyle::Solid), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        paths.push(solid_fill_polygon(&vec![poly_points], &mut options));
                    },
                    ::core::option::Option::None => {
                        paths.push(pattern_fill_polygons(&mut vec![poly_points], &mut options));
                    },
                }
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.stroke.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                paths.push(outline);
            },
            ::core::option::Option::None => {},
        }

        self.d("curve", &paths, &Some(options))
    }

    #[must_use]
    pub fn curve<F>(&self, points: &[Point2D<F>], options: &Option<Options>) -> Drawable<F>
    where
        F: Float + Trig + FromPrimitive + MulAssign + Display,
    {
        let mut paths = vec![];
        let mut options = options
            .clone()
            .unwrap_or_else(|| self.default_options.clone());
        let outline = curve(points, &mut options);

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.fill.is_some() && points.len() >= 3, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                let curve = curve_to_bezier(points, _c(0.0));

                match curve {
                    Some(crv) => {
                        let poly_points = points_on_bezier_curves(
                            &crv,
                            _c(10.0),
                            Some(_c::<F>(1.0) + _c::<F>(options.roughness.unwrap_or(0.0)) / _c(2.0)),
                        );

                        match {
                            let _explicit_decision = ::core::primitive::bool::then_some(options.fill_style == Some(FillStyle::Solid), ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                paths.push(solid_fill_polygon(&vec![poly_points], &mut options));
                            },
                            ::core::option::Option::None => {
                                paths.push(pattern_fill_polygons(&mut vec![poly_points], &mut options));
                            },
                        }
                    },
                    ::core::option::Option::None => {},
                }
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.stroke.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                paths.push(outline);
            },
            ::core::option::Option::None => {},
        }

        self.d("curve", &paths, &Some(options))
    }

    #[must_use]
    pub fn polygon<F>(&self, points: &[Point2D<F>], options: &Option<Options>) -> Drawable<F>
    where
        F: Float + Trig + FromPrimitive + MulAssign + Display,
    {
        let mut options = options
            .clone()
            .unwrap_or_else(|| self.default_options.clone());
        let mut paths = vec![];
        let outline = linear_path(points, true, &mut options);

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.fill.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(options.fill_style == Some(FillStyle::Solid), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        paths.push(solid_fill_polygon(&vec![points.to_vec()], &mut options));
                    },
                    ::core::option::Option::None => {
                        paths.push(pattern_fill_polygons(
                            &mut vec![points.to_vec()],
                            &mut options,
                        ));
                    },
                }
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(options.stroke.is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                paths.push(outline);
            },
            ::core::option::Option::None => {},
        }

        self.d("polygon", &paths, &Some(options))
    }

    #[must_use]
    pub fn path<F>(&self, d: String, options: &Option<Options>) -> Drawable<F>
    where
        F: Float + Trig + FromPrimitive + MulAssign + Display,
    {
        let mut options = options.clone().unwrap_or(self.default_options.clone());
        let mut paths = vec![];

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(d.is_empty(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                self.d("path", &paths, &Some(options))
            },
            ::core::option::Option::None => {
                let simplified = options.simplification.map(|a| a < 1.0).unwrap_or(false);
                let distance = match ::core::primitive::bool::then_some(simplified, ()) {
                    ::core::option::Option::Some(()) => {
                        _c::<F>(4.0) - _c::<F>(4.0) * _c::<F>(options.simplification.unwrap())
                    },
                    ::core::option::Option::None => {
                        (_c::<F>(1.0) + _c::<F>(options.roughness.unwrap_or(1.0))) / _c::<F>(2.0)
                    },
                };

                let sets = points_on_path(d.clone(), Some(_c(1.0)), Some(distance));

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(options.fill.is_some(), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        match {
                            let _explicit_decision = ::core::primitive::bool::then_some(options.fill_style == Some(FillStyle::Solid), ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                paths.push(solid_fill_polygon(&sets, &mut options));
                            },
                            ::core::option::Option::None => {
                                paths.push(pattern_fill_polygons(sets.clone(), &mut options));
                            },
                        }
                    },
                    ::core::option::Option::None => {},
                }

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(options.stroke.is_some(), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        match ::core::primitive::bool::then_some(simplified, ()) {
                            ::core::option::Option::Some(()) => {
                                sets.iter()
                                    .for_each(|s| paths.push(linear_path(s, false, &mut options)));
                            },
                            ::core::option::Option::None => {
                                paths.push(svg_path(d, &mut options));
                            },
                        }
                    },
                    ::core::option::Option::None => {},
                }

                self.d("path", &paths, &Some(options))
            },
        }
    }

    #[must_use]
    pub fn path_from_segments<F>(
        &self,
        segments: Vec<impl SvgPathSegment>,
        options: &Option<Options>,
    ) -> Drawable<F>
    where
        F: Float + Trig + FromPrimitive + MulAssign + Display,
    {
        let mut options = options.clone().unwrap_or(self.default_options.clone());
        let mut paths = vec![];

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(segments.is_empty(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                self.d("path", &paths, &Some(options))
            },
            ::core::option::Option::None => {
                let simplified = options.simplification.map(|a| a < 1.0).unwrap_or(false);
                let distance = match ::core::primitive::bool::then_some(simplified, ()) {
                    ::core::option::Option::Some(()) => {
                        _c::<F>(4.0) - _c::<F>(4.0) * _c::<F>(options.simplification.unwrap())
                    },
                    ::core::option::Option::None => {
                        (_c::<F>(1.0) + _c::<F>(options.roughness.unwrap_or(1.0))) / _c::<F>(2.0)
                    },
                };

                let sets = points_on_segments(segments.clone(), Some(_c(1.0)), Some(distance));

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(options.fill.is_some(), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        match {
                            let _explicit_decision = ::core::primitive::bool::then_some(options.fill_style == Some(FillStyle::Solid), ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                paths.push(solid_fill_polygon(&sets, &mut options));
                            },
                            ::core::option::Option::None => {
                                paths.push(pattern_fill_polygons(sets.clone(), &mut options));
                            },
                        }
                    },
                    ::core::option::Option::None => {},
                }

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(options.stroke.is_some(), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        match ::core::primitive::bool::then_some(simplified, ()) {
                            ::core::option::Option::Some(()) => {
                                sets.iter()
                                    .for_each(|s| paths.push(linear_path(s, false, &mut options)));
                            },
                            ::core::option::Option::None => {
                                paths.push(svg_segments(segments, &mut options));
                            },
                        }
                    },
                    ::core::option::Option::None => {},
                }

                self.d("path", &paths, &Some(options))
            },
        }
    }

    #[must_use]
    pub fn ops_to_path<F>(mut drawing: OpSet<F>, fixed_decimals: Option<u32>) -> String
    where
        F: Float + FromPrimitive + Trig + Display,
    {
        let mut path = String::new();

        for item in drawing.ops.iter_mut() {
            match fixed_decimals {
                Some(fd) => {
                    let pow: u32 = 10u32.pow(fd);
                    item.data.iter_mut().for_each(|p| {
                        *p = (*p * F::from(pow).unwrap()).round() / F::from(pow).unwrap();
                    });
                },
                ::core::option::Option::None => {},
            }

            match item.op {
                OpType::Move => {
                    write!(&mut path, "L{} {} ", item.data[0], item.data[1])
                        .expect("Failed to write path string");
                }
                OpType::BCurveTo => {
                    write!(
                        &mut path,
                        "C{} {}, {} {}, {} {} ",
                        item.data[0],
                        item.data[1],
                        item.data[2],
                        item.data[3],
                        item.data[4],
                        item.data[5]
                    )
                    .expect("Failed to write path string");
                }
                OpType::LineTo => {
                    write!(&mut path, "L{} {}, ", item.data[0], item.data[1])
                        .expect("Failed to write path string");
                }
            }
        }

        path
    }

    #[must_use]
    pub fn to_paths<F>(drawable: Drawable<F>) -> Vec<PathInfo>
    where
        F: Float + FromPrimitive + Trig + Display,
    {
        let sets = drawable.sets;
        let o = drawable.options;
        let mut path_infos = vec![];

        for drawing in sets.iter() {
            let path_info = match drawing.op_set_type {
                OpSetType::Path => PathInfo {
                    d: Self::ops_to_path(drawing.clone(), None),
                    stroke: o.stroke,
                    stroke_width: o.stroke_width,
                    fill: None,
                },
                OpSetType::FillPath => PathInfo {
                    d: Self::ops_to_path(drawing.clone(), None),
                    stroke: None,
                    stroke_width: Some(0.0f32),
                    fill: o.fill,
                },
                OpSetType::FillSketch => {
                    let fill_weight = match {
                        let _explicit_decision = ::core::primitive::bool::then_some(o.fill_weight.unwrap_or(0.0) < 0.0, ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            o.stroke_width.unwrap_or(0.0) / 2.0
                        },
                        ::core::option::Option::None => {
                            o.fill_weight.unwrap_or(0.0)
                        },
                    };
                    PathInfo {
                        d: Self::ops_to_path(drawing.clone(), None),
                        stroke: o.fill,
                        stroke_width: Some(fill_weight),
                        fill: None,
                    }
                }
            };
            path_infos.push(path_info);
        }

        path_infos
    }
}

#[cfg(test)]
mod tests {
    use super::Generator;
    use crate::core::{
        FillStyle, OpSetType, Options, OptionsBuilder, RoughJsSeed, RoughMathRandom,
        RoughRandomness,
    };

    fn zero_seed_solid_options() -> Options {
        zero_seed_solid_options_with_math_random(RoughMathRandom::new(7))
    }

    fn zero_seed_solid_options_with_math_random(math_random: RoughMathRandom) -> Options {
        OptionsBuilder::default()
            .randomness(RoughRandomness::new(RoughJsSeed::new(0.0), math_random))
            .fill(roughr_color())
            .fill_style(FillStyle::Solid)
            .stroke(roughr_color())
            .build()
            .expect("rectangle options")
    }

    fn roughr_color() -> palette::Srgba {
        palette::Srgba::new(0.0, 0.0, 0.0, 1.0)
    }

    #[test]
    fn rectangle_advances_one_fallback_chain_then_emits_fill_before_stroke() {
        let generator = Generator::default();
        let drawable =
            generator.rectangle::<f64>(0.0, 0.0, 20.0, 10.0, &Some(zero_seed_solid_options()));

        assert_eq!(drawable.sets.len(), 2);
        assert_eq!(drawable.sets[0].op_set_type, OpSetType::FillPath);
        assert_eq!(drawable.sets[1].op_set_type, OpSetType::Path);

        let points = vec![vec![
            euclid::default::Point2D::new(0.0, 0.0),
            euclid::default::Point2D::new(20.0, 0.0),
            euclid::default::Point2D::new(20.0, 10.0),
            euclid::default::Point2D::new(0.0, 10.0),
        ]];
        let mut continuous = zero_seed_solid_options();
        let expected_stroke = crate::renderer::rectangle(0.0, 0.0, 20.0, 10.0, &mut continuous);
        let expected_fill = crate::renderer::solid_fill_polygon(&points, &mut continuous);
        assert_eq!(drawable.sets[0], expected_fill);
        assert_eq!(drawable.sets[1], expected_stroke);

        let mut reset = zero_seed_solid_options();
        let reset_fill = crate::renderer::solid_fill_polygon(&points, &mut reset);
        assert_ne!(drawable.sets[0], reset_fill);
    }

    #[test]
    fn separate_shapes_continue_one_shared_fallback_stream() {
        let generator = Generator::default();
        let shared = RoughMathRandom::new(7);
        let _first = generator.rectangle::<f64>(
            0.0,
            0.0,
            20.0,
            10.0,
            &Some(zero_seed_solid_options_with_math_random(shared.clone())),
        );
        let second = generator.rectangle::<f64>(
            30.0,
            0.0,
            20.0,
            10.0,
            &Some(zero_seed_solid_options_with_math_random(shared)),
        );
        let reset_second =
            generator.rectangle::<f64>(30.0, 0.0, 20.0, 10.0, &Some(zero_seed_solid_options()));

        assert_ne!(second.sets, reset_second.sets);
    }
}
