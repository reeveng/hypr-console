//! Keep source line numbers when removing an ordinary comment.

fn main() {
    // Repeats the code.
    // A second line.
    assert_eq!(line!(), 6);
    // Standalone line.
    assert_eq!(line!(), 8); // One more.
}
