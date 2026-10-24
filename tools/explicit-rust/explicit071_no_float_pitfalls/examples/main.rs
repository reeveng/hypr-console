fn same(one: f64, another: f64) -> bool {
    //~v WARN EXPLICIT071_NO_FLOAT_PITFALLS
    one == another
}

fn asked(one: f64, another: f64) -> std::cmp::Ordering {
    one.total_cmp(&another)
}

fn held(one: f64, another: f64) -> bool {
    one.to_bits() == another.to_bits()
}

fn main() {
    let _ = (same(1.0, 2.0), asked(1.0, 2.0), held(1.0, 2.0));
}
