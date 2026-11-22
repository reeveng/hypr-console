// edition:2024
// rustc-env:CARGO_PKG_RUST_VERSION=1.94

fn keep(input: &[u8]) -> u8 {
    let mut result = 0;
    for chunk in input.chunks(2) {
        result += chunk[1];
    }
    for chunk in input.chunks_exact(2) {
        result += chunk[2];
    }
    for chunk in input.chunks_exact(2) {
        result += chunk[0];
        result += chunk.iter().sum::<u8>();
    }
    for window in input.windows(2) {
        result += (|| window[0])();
    }
    result
}

fn conditional(input: &[u8]) -> u8 {
    let mut result = 0;
    for chunk in input.chunks_exact(2) {
        #[cfg(any())]
        use_chunk(chunk);
        result += chunk[0];
    }
    result
}

fn expanded(input: &[u8]) -> u8 {
    macro_rules! inspect { ($chunk:expr) => {} }
    let mut result = 0;
    for chunk in input.chunks_exact(2) {
        inspect!(chunk);
        result += chunk[0];
    }
    result
}

struct Custom<'a>(&'a [u8]);
impl<'a> Custom<'a> {
    fn chunks_exact(&self, _: usize) -> std::slice::Chunks<'a, u8> {
        self.0.chunks(1)
    }
}
fn custom(input: Custom<'_>) {
    for chunk in input.chunks_exact(2) {
        let sample = chunk[1];
        std::hint::black_box(sample);
    }
}
fn main() {}
