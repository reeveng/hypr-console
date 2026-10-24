struct Placed {
    x: f64,
}

#[allow(dead_code)]
fn drawn(at: f64, from: f64) -> f64 {
    at - from
}

struct Ruler;

impl Ruler {
    fn measured(&self, across: f64) -> f64 {
        across
    }
}

trait Scaled {
    fn scaled(&self, by: f32) -> u8;
}

impl Scaled for Ruler {
    fn scaled(&self, by: f32) -> u8 {
        u8::from(by > 1.0)
    }
}

impl From<f64> for Ruler {
    fn from(_: f64) -> Self {
        Ruler
    }
}

fn main() {
    let _ = Placed { x: 1.0 };
    let _ = drawn(1.0, 2.0);
    let _ = (Ruler.measured(1.0), Ruler.scaled(2.0), Ruler::from(1.0));
}
