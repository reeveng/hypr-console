// edition:2024

struct Search;

impl Search {
  fn binary_search(&self, offset: usize) -> Result<usize, usize> {
    if offset == 3 {
      Ok(offset)
    } else {
      Err(offset * 2)
    }
  }
}

fn main() {
  assert_eq!(spoofed(), None);
}

fn spoofed() -> Option<usize> {
  Search.binary_search(1).ok()
}
