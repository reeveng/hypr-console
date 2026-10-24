use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
use console_core_picture_scaling::{Plane, Scaling, Source, scaled, scaled_plane};

fn whole(size: Size<u32>) -> Result<Rectangle<u32>, Never> {
    Ok(Rectangle { origin: Point { x: 0, y: 0 }, size })
}

#[test]
fn the_same_size_is_the_same_picture() {
    let rgba: Vec<u8> = (0u8..24).collect();
    let size = Size { width: 3, height: 2 };
    let Ok(region) = whole(size);

    assert_eq!(scaled(Source { rgba: &rgba, width: 3, region }, size), Ok(rgba));
}

#[test]
fn black_beside_white_made_one_pixel_is_grey() {
    let rgba = [0, 0, 0, 255, 255, 255, 255, 255];
    let Ok(region) = whole(Size { width: 2, height: 1 });
    let one = Size { width: 1, height: 1 };

    assert_eq!(scaled(Source { rgba: &rgba, width: 2, region }, one), Ok(vec![128, 128, 128, 255]));
}

#[test]
fn a_region_is_drawn_from_its_own_pixels_only() {
    let rgba = [10, 10, 10, 255, 20, 20, 20, 255, 30, 30, 30, 255, 40, 40, 40, 255];
    let region = Rectangle { origin: Point { x: 1, y: 1 }, size: Size { width: 1, height: 1 } };
    let one = Size { width: 1, height: 1 };

    assert_eq!(scaled(Source { rgba: &rgba, width: 2, region }, one), Ok(vec![40, 40, 40, 255]));
}

#[test]
fn rows_handed_over_one_at_a_time_make_the_picture_handed_over_whole() {
    let rgba: Vec<u8> = (0u8..=u8::MAX).cycle().take(432).collect();
    let region = Rectangle { origin: Point { x: 2, y: 1 }, size: Size { width: 9, height: 7 } };
    let to = Size { width: 4, height: 3 };

    let Ok(mut scaling) = Scaling::new(12, region, to);

    for row in rgba.chunks(48) {
        let Ok(()) = scaling.received(row);
    }

    assert_eq!(scaling.finished(), scaled(Source { rgba: &rgba, width: 12, region }, to));
}

#[test]
fn a_plane_is_drawn_with_the_filter_a_picture_is_and_its_padding_is_never_read() {
    let size = Size { width: 9, height: 7 };
    let samples: Vec<u8> = (0u8..=u8::MAX).cycle().step_by(7).take(63).collect();
    let padded: Vec<u8> = samples.chunks(9).flat_map(|line| line.iter().copied().chain([255, 255, 255])).collect();
    let rgba: Vec<u8> = samples.iter().flat_map(|sample| [*sample, *sample, *sample, 255]).collect();
    let Ok(region) = whole(size);

    for to in [Size { width: 4, height: 3 }, Size { width: 9, height: 7 }, Size { width: 20, height: 11 }] {
        let Ok(picture) = scaled(Source { rgba: &rgba, width: 9, region }, to);
        let reds: Vec<u8> = picture.as_chunks::<4>().0.iter().map(|[red, ..]| *red).collect();
        let Ok(plane) = scaled_plane(Plane { samples: &padded, stride: 12, size }, to);
        let furthest = plane.iter().zip(&reds).map(|(plane, red)| plane.abs_diff(*red)).max();

        assert_eq!(plane.len(), reds.len(), "{to:?}");
        assert!(furthest <= Some(1), "drawn down then across rather than across then down, so a sample may round the other way, and no further: {furthest:?} at {to:?}");
    }
}

#[test]
fn a_plane_of_nothing_is_nothing() {
    let nothing = Size { width: 0, height: 0 };

    assert_eq!(scaled_plane(Plane { samples: &[], stride: 0, size: nothing }, Size { width: 4, height: 4 }), Ok(Vec::new()));
}
