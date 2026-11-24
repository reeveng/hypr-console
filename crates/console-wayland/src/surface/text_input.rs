//! Wayland text input keeps the published editor, focus, and commit serial
//! together. Password prompts publish content hints and cursor geometry.

use crate::protocols::wp::text_input::zv3::client::zwp_text_input_v3::{
  ContentHint, ContentPurpose, ZwpTextInputV3,
};
use console_core_never::Never;
use console_core_number_conversion::{fitted, fixed_index, index};

use super::Part;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composition {
  pub text: String,
  pub cursor_begin: i32,
  pub cursor_end: i32,
}

#[derive(Debug, Default)]
pub struct TextInputChange {
  pub preedit: Option<Composition>,
  pub commit: Option<String>,
  pub delete_before: u32,
  pub delete_after: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextInputState {
  pub content: InputContent,
  pub cursor: Option<Part>,
  pub identity: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputContent {
  Plain(Option<Surrounding>),
  Password,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surrounding {
  pub text: String,
  pub cursor: u32,
}

#[derive(Clone, Copy)]
pub(crate) enum TextFocus {
  Entered,
  Left,
}

#[derive(Clone, Copy)]
enum CommitStatus {
  Current,
  AwaitingLatest,
}

struct Active {
  editor: TextInputState,
  enabled_at: u32,
  commit: CommitStatus,
}

enum Published {
  Disabled,
  Enabled(Active),
}

enum Session {
  Unfocused,
  Focused(Published),
}

enum Update {
  Stay,
  Restart(CommitStatus),
  Edit {
    enabled_at: u32,
    commit: CommitStatus,
  },
}

pub(crate) struct Input {
  protocol: ZwpTextInputV3,
  session: Session,
  pub(crate) pending: TextInputChange,
  pub(crate) received: Vec<TextInputChange>,
  desired: Option<TextInputState>,
  serial: u32,
}

impl Input {
  pub(crate) fn new(protocol: ZwpTextInputV3) -> Result<Self, Never> {
    Ok(Self {
      protocol,
      session: Session::Unfocused,
      pending: TextInputChange::default(),
      received: Vec::new(),
      desired: None,
      serial: 0,
    })
  }

  pub(crate) fn focus(&mut self, focused: TextFocus) -> Result<(), Never> {
    self.pending = TextInputChange::default();
    self.received.clear();
    self.session = match focused {
      TextFocus::Entered => Session::Focused(Published::Disabled),
      TextFocus::Left => {
        self.received.push(TextInputChange::default());
        Session::Unfocused
      }
    };

    self.sync()
  }

  pub(crate) fn set(&mut self, desired: Option<TextInputState>) -> Result<(), Never> {
    self.desired = desired;

    self.sync()
  }

  pub(crate) fn done(&mut self, serial: u32) -> Result<(), Never> {
    let pending = std::mem::take(&mut self.pending);
    let active = match (&mut self.session, &self.desired) {
      (Session::Focused(Published::Enabled(active)), Some(_)) => active,
      (Session::Unfocused | Session::Focused(Published::Disabled), _)
      | (Session::Focused(Published::Enabled(_)), None) => return Ok(()),
    };

    match serial.wrapping_sub(active.enabled_at) {
      0..=0x7fff_ffff => {
        self.received.push(pending);
        active.commit = match (serial == self.serial).then_some(()) {
          Some(()) => CommitStatus::Current,
          None => CommitStatus::AwaitingLatest,
        };
      }
      0x8000_0000.. => {}
    }

    Ok(())
  }

  fn sync(&mut self) -> Result<(), Never> {
    let published = match &mut self.session {
      Session::Unfocused => return Ok(()),
      Session::Focused(published) => published,
    };

    let desired = match (&self.desired, &*published) {
      (Some(edit), _) => edit,
      (None, Published::Disabled) => return Ok(()),
      (None, Published::Enabled(_)) => {
        self.protocol.disable();
        self.pending = TextInputChange::default();
        self.received.clear();
        self.protocol.commit();
        self.serial = self.serial.wrapping_add(1);
        *published = Published::Disabled;
        return Ok(());
      }
    };

    let Ok(update) = update_for(desired, published);
    let (enabled_at, commit) = match update {
      Update::Stay => return Ok(()),
      Update::Edit { enabled_at, commit } => (enabled_at, commit),
      Update::Restart(commit) => {
        self.protocol.enable();
        self.pending = TextInputChange::default();
        self.received.clear();
        let (hints, purpose) = match &desired.content {
          InputContent::Plain(_) => (ContentHint::None, ContentPurpose::Normal),
          InputContent::Password => (
            ContentHint::HiddenText | ContentHint::SensitiveData,
            ContentPurpose::Password,
          ),
        };
        self.protocol.set_content_type(hints, purpose);
        (self.serial.wrapping_add(1), commit)
      }
    };

    let Ok(()) = update_editor(&self.protocol, desired);
    self.protocol.commit();
    self.serial = self.serial.wrapping_add(1);
    *published = Published::Enabled(Active {
      editor: desired.clone(),
      enabled_at,
      commit,
    });

    Ok(())
  }
}

fn update_for(editor: &TextInputState, published: &Published) -> Result<Update, Never> {
  let active = match published {
    Published::Disabled => return Ok(Update::Restart(CommitStatus::Current)),
    Published::Enabled(active) => active,
  };
  let shape_changed = editor.identity != active.editor.identity
    || std::mem::discriminant(&editor.content) != std::mem::discriminant(&active.editor.content);

  Ok(
    match (shape_changed, editor == &active.editor, active.commit) {
      (_, true, _) => Update::Stay,
      (true, false, commit) => Update::Restart(commit),
      (false, false, CommitStatus::AwaitingLatest) => Update::Stay,
      (false, false, commit @ CommitStatus::Current) => Update::Edit {
        enabled_at: active.enabled_at,
        commit,
      },
    },
  )
}

fn update_editor(protocol: &ZwpTextInputV3, editor: &TextInputState) -> Result<(), Never> {
  match &editor.content {
    InputContent::Plain(Some(Surrounding { text, cursor })) => match surrounding(text, *cursor) {
      Ok(Some((window, position))) => {
        protocol.set_surrounding_text(window.to_owned(), position, position)
      }
      Ok(None) => {}
    },
    InputContent::Plain(None) | InputContent::Password => {}
  }

  match editor.cursor {
    Some(cursor) => {
      let max = i32::MAX.unsigned_abs();
      let Ok(wide) = fitted::<_, i32>(cursor.size.width.min(max));
      let Ok(tall) = fitted::<_, i32>(cursor.size.height.min(max));
      protocol.set_cursor_rectangle(cursor.at.x, cursor.at.y, wide, tall);
    }
    None => {}
  }

  Ok(())
}

fn surrounding(text: &str, cursor: u32) -> Result<Option<(&str, i32)>, Never> {
  let Ok(cursor_index) = index(cursor);

  match (text.is_char_boundary(cursor_index) && !text.contains('\0')).then_some(()) {
    Some(()) => {}
    None => return Ok(None),
  }

  let Ok(start) = fixed_index(text.ceil_char_boundary(cursor_index.saturating_sub(2_000)));
  let Ok(from) = index(start);
  let Ok(end) = fixed_index(text.floor_char_boundary(text.len().min(from.saturating_add(4_000))));
  let Ok(through) = index(end);
  let Ok(relative) = fitted::<_, i32>(u64::from(cursor).saturating_sub(start));

  Ok(text.get(from..through).map(|window| (window, relative)))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn waiting_for_a_commit_defers_edits_but_restarts_for_a_new_editor() {
    let editor = TextInputState {
      content: InputContent::Plain(None),
      cursor: None,
      identity: 8,
    };
    let published = Published::Enabled(Active {
      editor: editor.clone(),
      enabled_at: 18,
      commit: CommitStatus::AwaitingLatest,
    });
    let edited = TextInputState {
      content: InputContent::Plain(Some(Surrounding {
        text: "draft".to_string(),
        cursor: 5,
      })),
      ..editor
    };
    let Ok(unchanged) = update_for(&edited, &published);
    assert!(matches!(unchanged, Update::Stay));

    let password = TextInputState {
      content: InputContent::Password,
      ..edited
    };
    let Ok(restarted) = update_for(&password, &published);
    assert!(matches!(
      restarted,
      Update::Restart(CommitStatus::AwaitingLatest)
    ));
  }

  #[test]
  fn surrounding_rejects_invalid_cursor_boundaries_and_nuls() {
    assert_eq!(surrounding("café", 3), Ok(Some(("café", 3))));
    assert_eq!(surrounding("café", 4), Ok(None));
    assert_eq!(surrounding("café", u32::MAX), Ok(None));
    assert_eq!(surrounding("a\0b", 1), Ok(None));
  }

  #[test]
  fn surrounding_clips_to_whole_scalars_on_both_sides() {
    let text = "€".repeat(2_000);
    let Ok(window) = surrounding(&text, 3_000);
    let expected = "€".repeat(1_333);
    assert_eq!(window, Some((expected.as_str(), 1_998)));
  }
}
