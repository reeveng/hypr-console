// edition:2021

fn nested(outer: bool, inner: bool) -> u8 {
    if outer {
        if inner {
            return 1;
        }
        return 2;
    }
    0
}

fn main() {
    assert_eq!(nested(true, true), 1);
    assert_eq!(nested(true, false), 2);
    assert_eq!(nested(false, true), 0);
}
