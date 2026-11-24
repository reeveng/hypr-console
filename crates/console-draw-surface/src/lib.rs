//! Console's drawing-surface API.

pub use console_wayland::memory;
pub use console_wayland::surface::{
  Activation, Anchor, Closed, Composition, Damage, DmaBufView, Drawing, InputContent, KeyPress,
  Keyboard, KeyboardEvent, KeyboardFocus, Keysym, LayerSupport, Lock, Margin, Part, PointerEvent,
  Room, Scale, Surface, SurfaceError, Surrounding, TextInputChange, TextInputState, Under,
  Unlocked, Visible, Wanted, Window, fingers, layer_focus, on_the_device, scale, standing,
};
