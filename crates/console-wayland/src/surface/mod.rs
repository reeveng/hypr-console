//! Wayland surfaces for drawing, receiving input, and locking the screen.

pub mod fingers;
pub mod layer_focus;
mod region;
pub mod scale;
pub mod standing;
mod text_input;

pub use console_core_highlight::Activation;
pub use scale::Scale;
pub use standing::{
  Anchor, Closed, Damage, DmaBufView, Drawing, KeyPress, Keyboard, KeyboardEvent, KeyboardFocus,
  Keysym, LayerSupport, Lock, Margin, Part, PointerEvent, Room, Surface, SurfaceError, Under,
  Unlocked, Visible, Wanted, Window, on_the_device,
};
pub use text_input::{Composition, InputContent, Surrounding, TextInputChange, TextInputState};
