// run-rustfix
// edition:2024

mod frame {
    fn predict_block(px: usize, py: usize, bias: usize, edge: usize) -> usize {
        px + py + bias + edge
    }

    pub fn sample(bytes: &[u8]) -> usize {
        let px = bytes.len().saturating_sub(5);
        let py = bytes.len().saturating_sub(7);
        let edge = bytes.len().saturating_sub(3);
        predict_block(px, py, predict_block(px, py, 2, edge), edge)
    }
}

fn main() { assert_eq!(frame::sample(&[0; 10]), 32); }
