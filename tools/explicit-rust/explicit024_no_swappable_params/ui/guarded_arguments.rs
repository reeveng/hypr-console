// run-rustfix
// edition:2024

mod storage {
    macro_rules! write_sample { ($dst:expr, $value:expr) => {{ debug_assert!($dst.len() < 8); $dst.push($value) }} }
    pub(crate) use write_sample;
}

mod pixels {
    use super::storage::write_sample;

    #[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
    fn predict<const BIAS: usize>(dst: &mut Vec<usize>, base: usize, stride: isize, width: usize, height: usize) {
        write_sample!(dst, base + stride.unsigned_abs() + width * height + BIAS);
    }

    #[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
    pub fn verify() {
        let mut calls = Vec::new();
        let mut order = Vec::new();
        let mut once = || predict::<3>(&mut calls, { order.push(1); 2 }, { order.push(2); -5 }, { order.push(3); 7 }, { order.push(4); 11 });
        once();
        assert_eq!(calls, [87]);
        assert_eq!(order, [1, 2, 3, 4]);
    }
}

fn main() { pixels::verify(); }
