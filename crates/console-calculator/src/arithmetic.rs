//! Arithmetic shared by the desktop calculator and the native calculator executable.

pub(crate) fn calculate(left: f64, operator: u8, right: f64) -> Option<f64> {
  match operator {
    b'+' => Some(left + right),
    b'-' => Some(left - right),
    b'*' => Some(left * right),
    b'/' => Some(left / right),
    _other => None,
  }
}
