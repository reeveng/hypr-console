#![allow(unknown_lints)]
#![allow(explicit024_no_swappable_params, reason = "this fixture models published positional methods and their typed alternatives")]
#[derive(Clone, Copy)]
pub struct Point { pub x: f32, pub y: f32 }
pub use path_builder::PathBuilder;
pub mod path_builder {
    use super::Point;
    #[derive(Default)]
    pub struct PathBuilder(pub Vec<(f32, f32)>);
    type Outcome = Result<(), core::convert::Infallible>;
    impl PathBuilder {
        pub fn move_to(&mut self, x: f32, y: f32) -> Outcome { self.0.push((x, y)); Ok(()) }
        pub fn move_to_pt(&mut self, end: Point) -> Outcome { self.0.push((end.x, end.y)); Ok(()) }
        pub fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) -> Outcome {
            self.0.extend([(x1, y1), (x, y)]); Ok(())
        }
        pub fn quad_to_pt(&mut self, control: Point, end: Point) -> Outcome {
            self.0.extend([(control.x, control.y), (end.x, end.y)]); Ok(())
        }
    }
}

pub use transform::{ScaleFactors, Transform};
pub mod transform {
    use super::Point;

    pub struct ScaleFactors { pub x: f32, pub y: f32 }
    pub struct Transform(pub Point);
    type Outcome = Result<Transform, core::convert::Infallible>;

    impl Transform {
        pub fn from_translate(x: f32, y: f32) -> Outcome { Self::from_translation(Point { x, y }) }
        pub fn from_translation(offset: Point) -> Outcome { Ok(Self(offset)) }
        pub fn post_scale(&self, x: f32, y: f32) -> Outcome { self.post_scale_by(ScaleFactors { x, y }) }
        pub fn post_scale_by(&self, factors: ScaleFactors) -> Outcome {
            Ok(Self(Point { x: self.0.x * factors.x, y: self.0.y * factors.y }))
        }
    }
}
