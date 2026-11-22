// run-rustfix
// edition:2024
// aux-build:console_core_never.rs
extern crate console_core_never;

mod fixtures {
    fn write_header(bytes: &mut Vec<u8>) {
        bytes.push(3);
    }

    #[must_use]
    fn example_track_table() -> Vec<u8> {
        let mut bytes = vec![1];
        write_header(&mut bytes);
        bytes
    }

    pub fn inspect() -> Result<(), ()> {
        let first = example_track_table();
        let mut second = example_track_table();
        let _ = example_track_table();
        second.push(9);
        assert_eq!(first, vec![1, 3]);
        assert_eq!(second, vec![1, 3, 9]);
        Ok(())
    }
}

mod ranges {
    fn covers<T: Ord>(start: T, end: T, inner_start: T, inner_end: T) -> bool {
        start <= inner_start && inner_end <= end
    }

    fn intersection<T: Copy + Ord>(left: &[T; 2], right: &[T; 2]) -> Option<T> {
        match left[0].max(right[0]) <= left[1].min(right[1]) {
            true => Some(left[0].max(right[0])),
            false => None,
        }
    }

    pub fn inspect() -> Result<(), ()> {
        match covers(2, 8, 4, 6) {
            true => {}
            false => return Err(()),
        }

        match intersection(&[1, 3], &[2, 4]) {
            Some(start) if start == 2 => Ok(()),
            Some(_) | None => Err(()),
        }
    }
}

fn main() {
    assert!(fixtures::inspect().is_ok());
    assert!(ranges::inspect().is_ok());
}
