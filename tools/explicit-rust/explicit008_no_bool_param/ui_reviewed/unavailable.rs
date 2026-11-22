// edition:2024

#[derive(Clone, Copy)]
enum Scaling {
    Apply,
    Skip,
}

fn scale_hidden(value: i32, apply_scale: bool) -> i32 {
    match apply_scale {
        true => value * 2,
        false => value,
    }
}

#[cfg(any())]
fn unavailable() -> i32 {
    scale_hidden(4, true)
}

fn scale_callback(value: i32, apply_scale: bool) -> i32 {
    match apply_scale {
        true => value * 2,
        false => value,
    }
}

extern "C" fn scale_export(value: i32, apply_scale: bool) -> i32 {
    match apply_scale {
        true => value * 2,
        false => value,
    }
}

fn scale_conditional_body(value: i32, apply_scale: bool) -> i32 {
    #[cfg(any())]
    let _ = !apply_scale;

    match apply_scale {
        true => value * 2,
        false => value,
    }
}

fn main() {
    let callback: fn(i32, bool) -> i32 = scale_callback;
    let _ = callback(4, true);
    let _ = scale_export(4, true);
    let _ = scale_hidden(4, false);
    let _ = scale_conditional_body(4, true);
}
