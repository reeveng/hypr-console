// edition:2024
fn windows(cells: &[u8], order: usize) {
    let width = order.saturating_add(1);
    let _ = cells.windows(width);
    let stride = core::cmp::max(order, 1);
    let _ = cells.chunks_exact(stride);
    let _ = cells.iter().step_by(usize::saturating_add(2, order));
    let _ = cells.windows(order.saturating_add(0));
    let _ = cells.windows(order.wrapping_add(1));
    let mut mutable = 1;
    mutable += order;
    let _ = cells.chunks(mutable);
}

fn prepend(values: &mut Vec<u8>, at: usize) {
    values.insert(0, 42);
    values.insert(at, 7);
}

fn division(shift: u32) {
    const BASE: u32 = 16;
    let _ = 8_u32.wrapping_div(u32::wrapping_shl(1, shift));
    let _ = 8_i32.wrapping_rem(3_i32.wrapping_shl(shift));
    let _ = 8_u32.wrapping_div(BASE.saturating_mul(2));
    let _ = 8_u32.wrapping_div(2_u32.wrapping_shl(shift));
    let _ = 8_u32.wrapping_div(BASE.saturating_mul(shift));
}

fn main() {}
