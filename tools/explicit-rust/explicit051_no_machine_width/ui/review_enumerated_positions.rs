// edition:2024

struct Custom;

impl Custom {
    fn enumerate<T: Default>(self) -> [(T, u8); 1] {
        [(T::default(), 6)]
    }
}

macro_rules! spelled {
    ($value:expr) => {
        (stringify!($value), $value)
    };
}

fn main() {
    let mut answer = [0; 1];
    for (position, item) in Custom.enumerate() {
        answer[usize::wrapping_add(position, 0)] = item;
    }
    for (_, item) in [0].into_iter().enumerate() {
        answer[usize::wrapping_add(item, 0)] = 1;
    }
    for (position, _) in [0].into_iter().enumerate() {
        let _as_written = spelled!(usize::wrapping_add(position, 0));
        assert_eq!(_as_written.0, "usize::wrapping_add(position, 0)");
    }
}
