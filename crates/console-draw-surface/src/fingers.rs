//! A hand on the glass, read as one finger or as a pinch.
//!
//! `wl_touch` numbers every finger and says each one's moves apart, and
//! everything above this crate was written for one pointer. So the first
//! finger down is the pointer and the rest are not: a second one down turns
//! the moves of either into how much further apart the two are than they were,
//! and nothing is pressed while it lasts. The pointer is lifted when the last
//! finger is, so a pinch never ends in a tap on whatever was under it.

use console_core_never::Never;

use crate::standing::PointerEvent;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Touch {
    Down { id: i32, at: (f64, f64) },
    Moved { id: i32, at: (f64, f64) },
    Up { id: i32 },
    Cancelled,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fingers {
    pressed: Vec<(i32, (f64, f64))>,
}

fn apart(down: &[(i32, (f64, f64))]) -> Result<Option<f64>, Never> {
    Ok(match (down.first(), down.get(1)) {
        (Some((_, one)), Some((_, other))) => Some((one.0 - other.0).hypot(one.1 - other.1)),
        (Some(_) | None, _) => None,
    })
}

impl Fingers {
    pub fn heard(&mut self, touch: Touch) -> Result<Option<PointerEvent>, Never> {
        Ok(match touch {
            Touch::Down { id, at } => {
                self.pressed.push((id, at));

                match self.pressed.len() {
                    1 => Some(PointerEvent::Down { at }),
                    _ => None,
                }
            }
            Touch::Moved { id, at } => {
                let Ok(was) = apart(&self.pressed);
                let first = self.pressed.first().map(|(first, _)| *first);

                for finger in self.pressed.iter_mut().filter(|(which, _)| *which == id) {
                    finger.1 = at;
                }

                let Ok(now) = apart(&self.pressed);

                match (was, now) {
                    (Some(was), Some(now)) => match was > 0.0 {
                        true => Some(PointerEvent::Pinched { by: now / was }),
                        false => None,
                    },
                    (None, _) | (Some(_), None) => match first == Some(id) {
                        true => Some(PointerEvent::Moved { at }),
                        false => None,
                    },
                }
            }
            Touch::Up { id } => {
                self.pressed.retain(|(which, _)| *which != id);

                match self.pressed.is_empty() {
                    true => Some(PointerEvent::Up),
                    false => None,
                }
            }
            Touch::Cancelled => {
                self.pressed.clear();

                Some(PointerEvent::Left)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard(fingers: &mut Fingers, touches: &[Touch]) -> Vec<PointerEvent> {
        touches
            .iter()
            .filter_map(|touch| {
                let Ok(event) = fingers.heard(*touch);

                event
            })
            .collect()
    }

    #[test]
    fn one_finger_is_the_pointer() {
        let mut fingers = Fingers::default();

        assert_eq!(
            heard(&mut fingers, &[
                Touch::Down { id: 4, at: (10.0, 10.0) },
                Touch::Moved { id: 4, at: (20.0, 10.0) },
                Touch::Up { id: 4 },
            ]),
            vec![
                PointerEvent::Down { at: (10.0, 10.0) },
                PointerEvent::Moved { at: (20.0, 10.0) },
                PointerEvent::Up,
            ],
        );
    }

    #[test]
    fn two_fingers_moving_apart_is_a_pinch_by_how_much_further_apart_they_are() {
        let mut fingers = Fingers::default();

        assert_eq!(
            heard(&mut fingers, &[
                Touch::Down { id: 1, at: (100.0, 100.0) },
                Touch::Down { id: 2, at: (200.0, 100.0) },
                Touch::Moved { id: 2, at: (300.0, 100.0) },
                Touch::Moved { id: 1, at: (200.0, 100.0) },
            ]),
            vec![
                PointerEvent::Down { at: (100.0, 100.0) },
                PointerEvent::Pinched { by: 2.0 },
                PointerEvent::Pinched { by: 0.5 },
            ],
        );
    }

    #[test]
    fn a_pinch_is_lifted_only_with_its_last_finger() {
        let mut fingers = Fingers::default();

        assert_eq!(
            heard(&mut fingers, &[
                Touch::Down { id: 1, at: (0.0, 0.0) },
                Touch::Down { id: 2, at: (50.0, 0.0) },
                Touch::Up { id: 1 },
                Touch::Up { id: 2 },
            ]),
            vec![PointerEvent::Down { at: (0.0, 0.0) }, PointerEvent::Up],
        );
    }
}
