// edition:2024

struct Source;
struct Destination;

impl Into<Destination> for Source {
    fn into(self) -> Destination {
        Destination
    }
}

fn user_implementation(value: Source) -> Destination {
    value.into()
    //~^ EXPLICIT045_NO_UNNAMED_CONVERSION
}

struct Inherent;

impl Inherent {
    fn into(self) -> Destination {
        Destination
    }
}

fn inherent_implementation(value: Inherent) -> Destination {
    value.into()
    //~^ EXPLICIT045_NO_UNNAMED_CONVERSION
}

fn main() {}
