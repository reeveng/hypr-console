//! What a hand on the page does to a book's notes, as one machine.
//!
//! The reader used to hold the tool, the gesture under the finger, the notes
//! and both halves of undo as fields of the program, written from wherever a
//! press happened to land. Here they are one state and one request at a time,
//! so what a press means can be asked on a laptop with no screen and answered
//! the same way twice.
//!
//! The cut is the fonts. Which letter is under a finger, which stroke an
//! eraser is touching and which lines the open page shows are answered by
//! measuring text, and that is the program's to do: it turns a touch into the
//! book's own terms -- a letter, a stroke, the lines on the page -- before it
//! asks anything here. What those mean is decided here: a tap on a highlight
//! takes it off, a stroke started near the last handwriting joins it, an eraser
//! on the last stroke of a note takes the note, and undo reaches only what the
//! open page shows.
//!
//! A typed note is written in a popup under its words, and the rule that keeps
//! it simple is that anything but typing ends the writing: a turn, a press on
//! the rail, a touch somewhere else. Writing one is one edit to undo rather than
//! one per letter, and a note left with nothing typed in it was never made.
//! Every letter is still saved as it is typed, because a book closed in the
//! middle of a sentence is the moment nobody is going to press anything to keep
//! it. B backs out one layer at a time: out of the writing, then out of the
//! popups open on the page, and only then out of the book.

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_state_machine::{EffectDependencies, Machine, Queue, Service};

use crate::notes::{self, Mark, Note, Popup, Scope, Spot, Stroke};
use crate::reading::Turn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
  Read,
  Highlight,
  Text,
  Pen,
  Eraser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rail {
  Tool(Tool),
  Scope,
  Undo,
  Redo,
}

pub const RAIL: [Rail; 8] = [
  Rail::Tool(Tool::Read),
  Rail::Tool(Tool::Highlight),
  Rail::Tool(Tool::Text),
  Rail::Tool(Tool::Pen),
  Rail::Tool(Tool::Eraser),
  Rail::Scope,
  Rail::Undo,
  Rail::Redo,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gesture {
  None,
  Selecting { from: Spot, to: Spot },
  Drawing(Stroke),
  Erasing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Writing {
  pub at: Spot,
  was: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Edit {
  Added(Note),
  Removed(Note),
  Stroked { at: Spot, stroke: Stroke },
  Erased { at: Spot, stroke: Stroke },
  Rewritten { at: Spot, was: String, now: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotations {
  pub tool: Tool,
  pub scope: Scope,
  pub notes: Vec<Note>,
  pub gesture: Gesture,
  pub cursor: Option<u32>,
  pub writing: Option<Writing>,
  done: Vec<Edit>,
  undone: Vec<Edit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleLine {
  pub block: u32,
  pub start: u32,
  pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visible {
  Text {
    section: u32,
    lines: Vec<VisibleLine>,
  },
  Picture {
    section: u32,
    paragraphs: u32,
  },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Touch {
  Selecting(Spot),
  Drawing(Point<i32>),
  Erasing(Option<Target>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
  Stroke { note: u32, stroke: u32 },
  Letter(Spot),
  Note(Spot),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Made {
  Highlight { tapped: Option<Spot>, note: Note },
  Handwriting { joining: Option<Stroke>, note: Note },
  Text(Note),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
  Letter(char),
  Backspace,
  Newline,
  Finished,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationEvent {
  Loaded(Vec<Note>),
  Pressed { rail: Rail, page: Visible },
  Walked,
  Picked { way: Turn, page: Visible },
  Touched(Touch),
  Moved(Touch),
  Made(Made),
  Dropped,
  Left,
  Opened(Spot),
  Closed(Spot),
  Tapped(Spot),
  Typed(Typed),
  Backed(Visible),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationEffect {
  Save(Vec<Note>),
  ShowKeyboard,
  Back,
}

impl EffectDependencies for AnnotationEffect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Save(_) => &[Service::FileSystem],
      Self::ShowKeyboard => &[Service::Input],
      Self::Back => &[],
    })
  }
}

pub struct Annotating;

type Effects = Queue<AnnotationEffect>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Here {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Way {
  Forward,
  Back,
}

impl Annotations {
  pub fn empty() -> Result<Annotations, Never> {
    Ok(Annotations {
      tool: Tool::Read,
      scope: Scope::Paragraph,
      notes: Vec::new(),
      gesture: Gesture::None,
      cursor: None,
      writing: None,
      done: Vec::new(),
      undone: Vec::new(),
    })
  }
}

impl Machine for Annotating {
  type Input = ();
  type State = Annotations;
  type Request = AnnotationEvent;
  type Effect = AnnotationEffect;

  fn initialize(
    _input: &(),
    _previous: Option<Annotations>,
    _effects: &mut Effects,
  ) -> Result<Annotations, Never> {
    Annotations::empty()
  }

  fn handle(
    state: Annotations,
    event: AnnotationEvent,
    effects: &mut Effects,
  ) -> Result<Annotations, Never> {
    let Ok(state_2) = match &event {
      AnnotationEvent::Typed(_)
      | AnnotationEvent::Tapped(_)
      | AnnotationEvent::Backed(_)
      | AnnotationEvent::Loaded(_) => Ok(state),
      AnnotationEvent::Pressed { .. }
      | AnnotationEvent::Walked
      | AnnotationEvent::Picked { .. }
      | AnnotationEvent::Touched(_)
      | AnnotationEvent::Moved(_)
      | AnnotationEvent::Made(_)
      | AnnotationEvent::Dropped
      | AnnotationEvent::Left
      | AnnotationEvent::Opened(_)
      | AnnotationEvent::Closed(_) => finished(state, effects),
    };

    match event {
      AnnotationEvent::Loaded(notes) => {
        let Ok(empty) = Annotations::empty();

        Ok(Annotations {
          notes,
          tool: state_2.tool,
          scope: state_2.scope,
          ..empty
        })
      }
      AnnotationEvent::Pressed { rail, page } => pressed(state_2, rail, &page, effects),
      AnnotationEvent::Walked => walked(state_2),
      AnnotationEvent::Picked { way, page } => picked(state_2, way, &page),
      AnnotationEvent::Touched(touch) => touched(state_2, touch, effects),
      AnnotationEvent::Moved(touch) => moved(state_2, touch, effects),
      AnnotationEvent::Made(made) => made_one(state_2, made, effects),
      AnnotationEvent::Dropped => Ok(Annotations {
        gesture: Gesture::None,
        ..state_2
      }),
      AnnotationEvent::Left => Ok(Annotations {
        gesture: Gesture::None,
        cursor: None,
        ..state_2
      }),
      AnnotationEvent::Opened(at) => popped(state_2, at, Popup::Open, effects),
      AnnotationEvent::Closed(at) => popped(state_2, at, Popup::Closed, effects),
      AnnotationEvent::Tapped(at) => tapped(state_2, at, effects),
      AnnotationEvent::Typed(typed) => typed_into(state_2, typed, effects),
      AnnotationEvent::Backed(page) => backed(state_2, &page, effects),
    }
  }
}

fn saved(state: Annotations, effects: &mut Effects) -> Result<Annotations, Never> {
  let Ok(()) = effects.offer(AnnotationEffect::Save(state.notes.clone()));

  Ok(state)
}

fn recorded(mut state: Annotations, edit: Edit) -> Result<Annotations, Never> {
  state.done.push(edit);
  state.undone.clear();

  Ok(state)
}

fn text_position(notes: &[Note], at: Spot) -> Result<Option<u32>, Never> {
  let found = notes
    .iter()
    .position(|note| match note.mark {
      Mark::Text { .. } => note.spot == at,
      Mark::Highlight { .. } | Mark::Handwriting { .. } => false,
    })
    .map(fitted::<_, u32>);

  Ok(found.map(|Ok(at_2)| at_2))
}

fn note_at(notes: &[Note], at: u32) -> Result<Option<&Note>, Never> {
  let Ok(at_2) = console_core_number_conversion::index(at);

  Ok(notes.get(at_2))
}

fn taken<T>(from: &mut Vec<T>, at: u32) -> Result<Option<T>, Never> {
  let Ok(at_2) = console_core_number_conversion::index(at);

  Ok(match from.get(at_2) {
    Some(_present) => from.extract_if(at_2.., |_| true).next(),
    None => None,
  })
}

fn last_taken<T>(from: &mut Vec<T>, wanted: impl FnMut(&T) -> bool) -> Result<Option<T>, Never> {
  Ok(match from.iter().rposition(wanted) {
    Some(position) => from.extract_if(position.., |_| true).next(),
    None => None,
  })
}

fn text_of(note: &Note) -> Result<Option<&str>, Never> {
  Ok(match &note.mark {
    Mark::Text { text, .. } => Some(text.as_str()),
    Mark::Highlight { .. } | Mark::Handwriting { .. } => None,
  })
}

fn with_text(mut note: Note, now: String) -> Result<Note, Never> {
  match &mut note.mark {
    Mark::Text { text, .. } => *text = now,
    Mark::Highlight { .. } | Mark::Handwriting { .. } => {}
  }

  Ok(note)
}

fn finished(mut state: Annotations, effects: &mut Effects) -> Result<Annotations, Never> {
  let writing = match state.writing.take() {
    Some(writing) => writing,
    None => return Ok(state),
  };

  let Ok(found) = text_position(&state.notes, writing.at);
  let Ok(note) = match found {
    Some(at) => note_at(&state.notes, at).map(|note| note.cloned()),
    None => Ok(None),
  };
  let Ok(now) = match &note {
    Some(note_2) => text_of(note_2).map(|text| text.map(str::to_string)),
    None => Ok(None),
  };
  let empty = now.as_deref().map(|text| text.trim().is_empty());

  match (found, note, writing.was, now, empty) {
    (Some(at), Some(_), None, _, Some(true)) => {
      let Ok(_left_empty) = taken(&mut state.notes, at);

      saved(state, effects)
    }
    (Some(_), Some(note_3), None, _, Some(false)) => recorded(state, Edit::Added(note_3)),
    (Some(at), Some(note_4), Some(was), _, Some(true)) => {
      let Ok(_emptied) = taken(&mut state.notes, at);
      let Ok(note_5) = with_text(note_4, was);
      let Ok(state_2) = recorded(state, Edit::Removed(note_5));

      saved(state_2, effects)
    }
    (Some(_), Some(_), Some(was), Some(now_2), Some(false)) => {
      match ::core::primitive::bool::then_some(was == now_2, ()) {
        ::core::option::Option::Some(()) => Ok(state),
        ::core::option::Option::None => recorded(
          state,
          Edit::Rewritten {
            at: writing.at,
            was,
            now: now_2,
          },
        ),
      }
    }
    (None, _, _, _, _)
    | (_, None, _, _, _)
    | (_, _, _, _, None)
    | (Some(_), Some(_), Some(_), None, Some(false)) => Ok(state),
  }
}

fn pressed(
  state: Annotations,
  rail: Rail,
  page: &Visible,
  effects: &mut Effects,
) -> Result<Annotations, Never> {
  let Ok(state_2) = match rail {
    Rail::Tool(tool) => Ok(Annotations { tool, ..state }),
    Rail::Scope => Ok(Annotations {
      scope: match state.scope {
        Scope::Page => Scope::Paragraph,
        Scope::Paragraph => Scope::Page,
      },
      ..state
    }),
    Rail::Undo => undone(state, page, effects),
    Rail::Redo => redone(state, page, effects),
  };

  Ok(Annotations {
    gesture: Gesture::None,
    cursor: None,
    ..state_2
  })
}

fn walked(state: Annotations) -> Result<Annotations, Never> {
  let tool = match state.tool {
    Tool::Read => Tool::Highlight,
    Tool::Highlight => Tool::Text,
    Tool::Text => Tool::Pen,
    Tool::Pen => Tool::Eraser,
    Tool::Eraser => Tool::Read,
  };

  Ok(Annotations {
    tool,
    gesture: Gesture::None,
    cursor: None,
    ..state
  })
}

fn blocks(page: &Visible) -> Result<Vec<u32>, Never> {
  Ok(match page {
    Visible::Text { lines, .. } => {
      let mut blocks: Vec<u32> = lines.iter().map(|line| line.block).collect();

      blocks.dedup();

      blocks
    }
    Visible::Picture { paragraphs, .. } => (0..*paragraphs).collect(),
  })
}

fn picked(state: Annotations, way: Turn, page: &Visible) -> Result<Annotations, Never> {
  let Ok(blocks) = blocks(page);

  let next = match (
    state
      .cursor
      .and_then(|cursor| blocks.iter().position(|block| *block == cursor)),
    way,
  ) {
    (None, Turn::Forward) => blocks.first(),
    (None, Turn::Back) => blocks.last(),
    (Some(at), Turn::Forward) => blocks.get(at.saturating_add(1)).or(blocks.get(at)),
    (Some(at), Turn::Back) => blocks.get(at.saturating_sub(1)),
  };

  Ok(Annotations {
    cursor: next.copied(),
    ..state
  })
}

fn touched(
  state: Annotations,
  touch: Touch,
  effects: &mut Effects,
) -> Result<Annotations, Never> {
  match touch {
    Touch::Selecting(spot) => Ok(Annotations {
      gesture: Gesture::Selecting {
        from: spot,
        to: spot,
      },
      ..state
    }),
    Touch::Drawing(at) => Ok(Annotations {
      gesture: Gesture::Drawing(Stroke(vec![at])),
      ..state
    }),
    Touch::Erasing(target) => {
      let Ok(state_2) = erased(state, target, effects);

      Ok(Annotations {
        gesture: Gesture::Erasing,
        ..state_2
      })
    }
  }
}

fn moved(
  mut state: Annotations,
  touch: Touch,
  effects: &mut Effects,
) -> Result<Annotations, Never> {
  let gesture = std::mem::replace(&mut state.gesture, Gesture::None);

  match (gesture, touch) {
    (Gesture::Selecting { from, .. }, Touch::Selecting(to)) => Ok(Annotations {
      gesture: Gesture::Selecting { from, to },
      ..state
    }),
    (Gesture::Drawing(mut stroke), Touch::Drawing(at)) => {
      stroke.0.push(at);

      Ok(Annotations {
        gesture: Gesture::Drawing(stroke),
        ..state
      })
    }
    (Gesture::Erasing, Touch::Erasing(target)) => {
      let Ok(state_2) = erased(state, target, effects);

      Ok(Annotations {
        gesture: Gesture::Erasing,
        ..state_2
      })
    }
    (
      gesture_2 @ (Gesture::None
      | Gesture::Selecting { .. }
      | Gesture::Drawing(_)
      | Gesture::Erasing),
      Touch::Selecting(_) | Touch::Drawing(_) | Touch::Erasing(_),
    ) => Ok(Annotations {
      gesture: gesture_2,
      ..state
    }),
  }
}

fn erase_stroke(
  notes: &mut Vec<Note>,
  EraseStrokeArguments { note, stroke }: EraseStrokeArguments,
) -> Result<Option<Edit>, Never> {
  let Ok(note_index) = console_core_number_conversion::index(note);
  let Ok(stroke_index) = console_core_number_conversion::index(stroke);
  let (spot, strokes) = match notes.get_mut(note_index) {
    Some(Note {
      spot,
      mark: Mark::Handwriting { strokes, .. },
      ..
    }) => (*spot, strokes),
    Some(Note {
      mark: Mark::Highlight { .. } | Mark::Text { .. },
      ..
    })
    | None => return Ok(None),
  };

  match (strokes.get(stroke_index), strokes.as_slice()) {
    (Some(_stroke), [_]) => {
      let Ok(removed) = taken(notes, note);

      Ok(removed.map(Edit::Removed))
    }
    (Some(_stroke), [_, _, ..]) => {
      let Ok(removed) = taken(strokes, stroke);

      Ok(removed.map(|rubbed| Edit::Erased {
        at: spot,
        stroke: rubbed,
      }))
    }
    (None, _) | (Some(_), []) => Ok(None),
  }
}

struct EraseStrokeArguments {
  note: u32,
  stroke: u32,
}

fn erased(
  mut state: Annotations,
  target: Option<Target>,
  effects: &mut Effects,
) -> Result<Annotations, Never> {
  let edit = match target {
    None => None,
    Some(Target::Stroke { note, stroke }) => {
      let Ok(rubbed) = erase_stroke(&mut state.notes, EraseStrokeArguments { note, stroke });

      rubbed
    }
    Some(Target::Letter(spot)) => {
      let Ok(found) = notes::highlight_at(&state.notes, spot);
      let Ok(gone) = match found {
        Some(found_2) => taken(&mut state.notes, found_2),
        None => Ok(None),
      };

      gone.map(Edit::Removed)
    }
    Some(Target::Note(spot)) => {
      let Ok(found) = text_position(&state.notes, spot);
      let Ok(gone) = match found {
        Some(found_3) => taken(&mut state.notes, found_3),
        None => Ok(None),
      };

      gone.map(Edit::Removed)
    }
  };

  match edit {
    Some(edit_2) => {
      let Ok(state_2) = recorded(state, edit_2);

      saved(state_2, effects)
    }
    None => Ok(state),
  }
}

fn made_one(
  mut state: Annotations,
  made: Made,
  effects: &mut Effects,
) -> Result<Annotations, Never> {
  state.gesture = Gesture::None;

  match made {
    Made::Highlight { tapped, note } => {
      let Ok(found) = match tapped {
        Some(at) => notes::highlight_at(&state.notes, at),
        None => Ok(None),
      };
      let Ok(gone) = match found {
        Some(found_2) => taken(&mut state.notes, found_2),
        None => Ok(None),
      };

      let Ok(state_2) = match gone {
        Some(gone_2) => recorded(state, Edit::Removed(gone_2)),
        None => {
          state.notes.push(note.clone());

          recorded(state, Edit::Added(note))
        }
      };

      saved(state_2, effects)
    }
    Made::Handwriting { joining, note } => {
      let joined = match (joining, state.notes.last_mut()) {
        (
          Some(stroke),
          Some(Note {
            spot,
            mark: Mark::Handwriting { strokes, .. },
            ..
          }),
        ) => {
          strokes.push(stroke.clone());

          Some(Edit::Stroked { at: *spot, stroke })
        }
        (
          Some(_),
          Some(Note {
            mark: Mark::Highlight { .. } | Mark::Text { .. },
            ..
          })
          | None,
        )
        | (None, _) => None,
      };

      let Ok(state_3) = match joined {
        Some(edit) => recorded(state, edit),
        None => {
          state.notes.push(note.clone());

          recorded(state, Edit::Added(note))
        }
      };

      saved(state_3, effects)
    }
    Made::Text(note) => {
      let end = match note.mark {
        Mark::Text { end, .. } => end,
        Mark::Highlight { end } => end,
        Mark::Handwriting { .. } => note.spot.start,
      };
      let Ok(found) = notes::text_at(&state.notes, note.spot, end);
      let Ok(existing) = match found {
        Some(found_3) => note_at(&state.notes, found_3)
          .map(|existing| existing.map(|existing_2| existing_2.spot)),
        None => Ok(None),
      };

      let Ok(state_4) = match existing {
        Some(at) => written(state, at),
        None => {
          let at = note.spot;

          state.notes.push(note);
          state.writing = Some(Writing { at, was: None });

          Ok(state)
        }
      };

      let Ok(()) = effects.offer(AnnotationEffect::ShowKeyboard);

      saved(state_4, effects)
    }
  }
}

fn written(mut state: Annotations, at: Spot) -> Result<Annotations, Never> {
  let was = state
    .notes
    .iter_mut()
    .find_map(|note| match (note.spot == at, &mut note.mark) {
      (true, Mark::Text { text, popup, .. }) => {
        *popup = Popup::Open;

        Some(text.clone())
      }
      (false, _) | (true, Mark::Highlight { .. } | Mark::Handwriting { .. }) => None,
    });

  state.writing = was.map(|was_2| Writing {
    at,
    was: Some(was_2),
  });

  Ok(state)
}

fn popped(
  mut state: Annotations,
  at: Spot,
  now: Popup,
  effects: &mut Effects,
) -> Result<Annotations, Never> {
  let changed = state
    .notes
    .iter_mut()
    .find_map(|note| match (note.spot == at, &mut note.mark) {
      (true, Mark::Text { popup, .. }) => {
        *popup = now;

        Some(())
      }
      (false, _) | (true, Mark::Highlight { .. } | Mark::Handwriting { .. }) => None,
    });

  match changed {
    Some(()) => saved(state, effects),
    None => Ok(state),
  }
}

fn tapped(state: Annotations, at: Spot, effects: &mut Effects) -> Result<Annotations, Never> {
  match state.writing.as_ref().map(|writing| writing.at == at) {
    Some(true) => return Ok(state),
    Some(false) | None => {}
  }

  let Ok(state_2) = finished(state, effects);
  let Ok(state_3) = written(state_2, at);

  match state_3.writing {
    Some(_) => {
      let Ok(()) = effects.offer(AnnotationEffect::ShowKeyboard);

      saved(state_3, effects)
    }
    None => Ok(state_3),
  }
}

fn typed_into(
  mut state: Annotations,
  typed: Typed,
  effects: &mut Effects,
) -> Result<Annotations, Never> {
  let writing = state.writing.as_ref().map(|writing| writing.at);

  let at = match (writing, typed) {
    (Some(_), Typed::Finished) => return finished(state, effects),
    (Some(at), Typed::Letter(_) | Typed::Newline | Typed::Backspace) => at,
    (None, _) => return Ok(state),
  };

  let text = state
    .notes
    .iter_mut()
    .find_map(|note| match (note.spot == at, &mut note.mark) {
      (true, Mark::Text { text, .. }) => Some(text),
      (false, _) | (true, Mark::Highlight { .. } | Mark::Handwriting { .. }) => None,
    });

  match (typed, text) {
    (Typed::Finished, _) => return Ok(state),
    (Typed::Letter(letter), Some(text_2)) => text_2.push(letter),
    (Typed::Newline, Some(text_3)) => text_3.push('\n'),
    (Typed::Backspace, Some(text_4)) => {
      let _taken = text_4.pop();
    }
    (Typed::Letter(_) | Typed::Newline | Typed::Backspace, None) => return Ok(state),
  }

  saved(state, effects)
}

fn backed(
  mut state: Annotations,
  page: &Visible,
  effects: &mut Effects,
) -> Result<Annotations, Never> {
  match state.writing {
    Some(_) => return finished(state, effects),
    None => {}
  }

  let mut closed = Vec::new();

  for note in state.notes.iter_mut() {
    let Ok(seen) = here(note, page);

    match (seen, &mut note.mark) {
      (
        Here::Yes,
        Mark::Text {
          popup: popup @ Popup::Open,
          ..
        },
      ) => {
        *popup = Popup::Closed;
        closed.push(note.spot);
      }
      (
        Here::Yes,
        Mark::Text {
          popup: Popup::Closed,
          ..
        }
        | Mark::Highlight { .. }
        | Mark::Handwriting { .. },
      )
      | (Here::No, _) => {}
    }
  }

  match ::core::primitive::bool::then_some(closed.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(()) = effects.offer(AnnotationEffect::Back);

      Ok(state)
    }
    ::core::option::Option::None => saved(state, effects),
  }
}

fn here(note: &Note, page: &Visible) -> Result<Here, Never> {
  let (section, lines) = match page {
    Visible::Text { section, lines } => (*section, Some(lines)),
    Visible::Picture { section, .. } => (*section, None),
  };

  let seen = match (note.spot.section == section, lines, &note.mark) {
    (false, _, _) => false,
    (true, None, _) => true,
    (true, Some(lines_2), Mark::Highlight { end } | Mark::Text { end, .. }) => {
      lines_2.iter().any(|line| {
        line.block == note.spot.block && note.spot.start < line.end && line.start < *end
      })
    }
    (true, Some(lines_3), Mark::Handwriting { .. }) => lines_3.iter().any(|line| {
      line.block == note.spot.block
        && line.start <= note.spot.start
        && note.spot.start < line.end.max(line.start.saturating_add(1))
    }),
  };

  Ok(match ::core::primitive::bool::then_some(seen, ()) {
    ::core::option::Option::Some(()) => Here::Yes,
    ::core::option::Option::None => Here::No,
  })
}

fn edit_here(notes: &[Note], edit: &Edit, page: &Visible) -> Result<Here, Never> {
  match edit {
    Edit::Added(note) | Edit::Removed(note) => here(note, page),
    Edit::Stroked { at, .. } | Edit::Erased { at, .. } | Edit::Rewritten { at, .. } => {
      match notes.iter().find(|note| note.spot == *at) {
        Some(note) => here(note, page),
        None => Ok(Here::No),
      }
    }
  }
}

fn put(mut state: Annotations, edit: &Edit, way: Way) -> Result<Annotations, Never> {
  let adds = match (edit, way) {
    (Edit::Added(_) | Edit::Stroked { .. } | Edit::Rewritten { .. }, Way::Forward)
    | (Edit::Removed(_) | Edit::Erased { .. }, Way::Back) => true,
    (Edit::Added(_) | Edit::Stroked { .. } | Edit::Rewritten { .. }, Way::Back)
    | (Edit::Removed(_) | Edit::Erased { .. }, Way::Forward) => false,
  };

  match (edit, adds) {
    (Edit::Added(note) | Edit::Removed(note), true) => state.notes.push(note.clone()),
    (Edit::Added(note) | Edit::Removed(note), false) => {
      let found = state
        .notes
        .iter()
        .position(|kept| {
          kept.spot == note.spot
            && std::mem::discriminant(&kept.mark) == std::mem::discriminant(&note.mark)
        })
        .map(fitted::<_, u32>);

      match found {
        Some(Ok(at)) => {
          let Ok(_taken) = taken(&mut state.notes, at);
        }
        None => {}
      }
    }
    (Edit::Stroked { at, stroke } | Edit::Erased { at, stroke }, adds_2) => {
      match (
        state
          .notes
          .iter_mut()
          .find(|note| note.spot == *at)
          .map(|note| &mut note.mark),
        adds_2,
      ) {
        (Some(Mark::Handwriting { strokes, .. }), true) => strokes.push(stroke.clone()),
        (Some(Mark::Handwriting { strokes, .. }), false) => {
          let Ok(_taken) = last_taken(strokes, |kept| kept == stroke);
        }
        (Some(Mark::Highlight { .. } | Mark::Text { .. }) | None, _) => {}
      }
    }
    (Edit::Rewritten { at, was, now }, adds_3) => {
      let chosen = match ::core::primitive::bool::then_some(adds_3, ()) {
        ::core::option::Option::Some(()) => now,
        ::core::option::Option::None => was,
      };

      match state
        .notes
        .iter_mut()
        .find(|note| note.spot == *at)
        .map(|note| &mut note.mark)
      {
        Some(Mark::Text { text, .. }) => *text = chosen.clone(),
        Some(Mark::Highlight { .. } | Mark::Handwriting { .. }) | None => {}
      }
    }
  }

  Ok(state)
}

fn undone(
  mut state: Annotations,
  page: &Visible,
  effects: &mut Effects,
) -> Result<Annotations, Never> {
  let Ok(done) = last_taken(&mut state.done, |edit| {
    edit_here(&state.notes, edit, page) == Ok(Here::Yes)
  });

  match done {
    Some(edit) => {
      let Ok(put_back) = put(state, &edit, Way::Back);

      state = put_back;
      state.undone.push(edit);
    }
    None => {
      let Ok(removed) = last_taken(&mut state.notes, |note| here(note, page) == Ok(Here::Yes));

      match removed {
        Some(note) => state.undone.push(Edit::Added(note)),
        None => return Ok(state),
      }
    }
  }

  saved(state, effects)
}

fn redone(
  mut state: Annotations,
  page: &Visible,
  effects: &mut Effects,
) -> Result<Annotations, Never> {
  let Ok(removed) = last_taken(&mut state.undone, |edit| {
    edit_here(&state.notes, edit, page) == Ok(Here::Yes)
  });
  let edit = match removed {
    Some(edit) => edit,
    None => return Ok(state),
  };

  let Ok(mut state_2) = put(state, &edit, Way::Forward);

  state_2.done.push(edit);

  saved(state_2, effects)
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_state_machine::{run, run_from};

  fn page() -> Result<Visible, Never> {
    Ok(Visible::Text {
      section: 0,
      lines: vec![
        VisibleLine {
          block: 1,
          start: 0,
          end: 40,
        },
        VisibleLine {
          block: 2,
          start: 0,
          end: 30,
        },
      ],
    })
  }

  fn the_next_chapter() -> Result<Visible, Never> {
    Ok(Visible::Text {
      section: 1,
      lines: vec![VisibleLine {
        block: 0,
        start: 0,
        end: 40,
      }],
    })
  }

  fn noted(spot: Spot, mark: Mark) -> Result<Note, Never> {
    Ok(Note {
      spot,
      mark,
      heading: "Loomings".to_string(),
      context: "Call me Ishmael.".to_string(),
    })
  }

  const ISHMAEL: Spot = Spot {
    section: 0,
    block: 1,
    start: 8,
  };

  fn highlighted() -> Result<AnnotationEvent, Never> {
    let Ok(note) = noted(ISHMAEL, Mark::Highlight { end: 16 });

    Ok(AnnotationEvent::Made(Made::Highlight {
      tapped: Some(ISHMAEL),
      note,
    }))
  }

  fn a_note_on(spot: Spot) -> Result<AnnotationEvent, Never> {
    let Ok(note) = noted(
      spot,
      Mark::Text {
        end: spot.start.saturating_add(5),
        text: String::new(),
        popup: Popup::Open,
      },
    );

    Ok(AnnotationEvent::Made(Made::Text(note)))
  }

  fn typing(said: &str) -> Result<Vec<AnnotationEvent>, Never> {
    Ok(
      said
        .chars()
        .map(|letter| AnnotationEvent::Typed(Typed::Letter(letter)))
        .collect(),
    )
  }

  fn pressing(rail: Rail, on: Result<Visible, Never>) -> Result<AnnotationEvent, Never> {
    let Ok(page) = on;

    Ok(AnnotationEvent::Pressed { rail, page })
  }

  fn written_on(state: &Annotations, spot: Spot) -> Result<Option<(String, Popup)>, Never> {
    Ok(
      state
        .notes
        .iter()
        .find_map(|note| match (&note.mark, note.spot == spot) {
          (Mark::Text { text, popup, .. }, true) => Some((text.clone(), *popup)),
          (_, false) | (Mark::Highlight { .. } | Mark::Handwriting { .. }, true) => None,
        }),
    )
  }

  fn events(asked: Vec<Result<AnnotationEvent, Never>>) -> Result<Vec<AnnotationEvent>, Never> {
    Ok(asked.into_iter().map(|Ok(event)| event).collect())
  }

  fn written(
    said: &str,
    then: Vec<Result<AnnotationEvent, Never>>,
  ) -> Result<Vec<AnnotationEvent>, Never> {
    let Ok(made) = a_note_on(ISHMAEL);
    let Ok(typed) = typing(said);
    let Ok(then_2) = events(then);
    let mut all = vec![made];

    all.extend(typed);
    all.extend(then_2);

    Ok(all)
  }

  #[test]
  fn a_tap_on_a_highlight_takes_it_off_and_undo_puts_it_back() {
    let Ok(taps) = events(vec![highlighted(), highlighted()]);
    let Ok(trace) = run::<Annotating>(&(), &taps);

    assert_eq!(trace.state.notes, Vec::new(), "the second tap took it off");

    let Ok(undo) = events(vec![pressing(Rail::Undo, page())]);
    let Ok(trace_2) = run_from::<Annotating>(trace.state, &undo);

    assert_eq!(
      trace_2.state.notes.len(),
      1,
      "undo put back what the tap took off"
    );
  }

  #[test]
  fn undo_reaches_only_the_page_that_is_open() {
    let Ok(elsewhere) = events(vec![
      highlighted(),
      pressing(Rail::Undo, the_next_chapter()),
    ]);
    let Ok(trace) = run::<Annotating>(&(), &elsewhere);

    assert_eq!(
      trace.state.notes.len(),
      1,
      "undo on the next chapter left the note on the first alone"
    );

    let Ok(back_and_forth) = events(vec![
      pressing(Rail::Undo, page()),
      pressing(Rail::Redo, the_next_chapter()),
    ]);
    let Ok(trace_2) = run_from::<Annotating>(trace.state, &back_and_forth);

    assert_eq!(
      trace_2.state.notes,
      Vec::new(),
      "redo on another page brought nothing back"
    );
  }

  #[test]
  fn a_note_is_typed_into_its_popup_and_kept_letter_by_letter() {
    let Ok(typed) = written("Who?", vec![]);
    let Ok(trace) = run::<Annotating>(&(), &typed);

    assert_eq!(
      written_on(&trace.state, ISHMAEL),
      Ok(Some(("Who?".to_string(), Popup::Open)))
    );
    assert_eq!(
      trace.state.writing.as_ref().map(|writing| writing.at),
      Some(ISHMAEL),
      "still writing"
    );
    assert_eq!(
      trace
        .on(0)
        .map(|effects| effects.map(|at_step| at_step.first().cloned())),
      Ok(Some(Some(AnnotationEffect::ShowKeyboard)))
    );
    assert!(
      match trace.on(4) {
        Ok(effects) => effects.is_some_and(|at_step| at_step
          .iter()
          .any(|effect| matches!(effect, AnnotationEffect::Save(_)))),
        Err(_) => false,
      },
      "the last letter was saved as it was typed"
    );
  }

  #[test]
  fn writing_a_note_is_one_edit_to_undo_and_redo() {
    let Ok(typed) = written(
      "Who?",
      vec![
        Ok(AnnotationEvent::Typed(Typed::Finished)),
        pressing(Rail::Undo, page()),
      ],
    );
    let Ok(trace) = run::<Annotating>(&(), &typed);

    assert_eq!(
      trace.state.notes,
      Vec::new(),
      "one undo took the whole note"
    );

    let Ok(redo) = events(vec![pressing(Rail::Redo, page())]);
    let Ok(trace_2) = run_from::<Annotating>(trace.state, &redo);

    assert_eq!(
      written_on(&trace_2.state, ISHMAEL),
      Ok(Some(("Who?".to_string(), Popup::Open))),
      "redo brought it back with what was typed"
    );
  }

  #[test]
  fn a_note_left_empty_was_never_made() {
    let Ok(typed) = written(
      "",
      vec![
        Ok(AnnotationEvent::Typed(Typed::Finished)),
        pressing(Rail::Undo, page()),
      ],
    );
    let Ok(trace) = run::<Annotating>(&(), &typed);

    assert_eq!(trace.state.notes, Vec::new());
    assert_eq!(trace.state.writing, None);
  }

  #[test]
  fn writing_again_into_a_note_is_undone_back_to_what_it_said() {
    let Ok(again) = typing("m?");
    let Ok(first) = written(
      "Who",
      vec![
        Ok(AnnotationEvent::Closed(ISHMAEL)),
        Ok(AnnotationEvent::Opened(ISHMAEL)),
        Ok(AnnotationEvent::Tapped(ISHMAEL)),
      ],
    );
    let Ok(read) = events(vec![pressing(Rail::Tool(Tool::Read), page())]);
    let Ok(trace) = run::<Annotating>(&(), &[first, again, read].concat());

    assert_eq!(
      written_on(&trace.state, ISHMAEL),
      Ok(Some(("Whom?".to_string(), Popup::Open)))
    );
    assert_eq!(
      trace.state.writing, None,
      "a press on the rail ended the writing"
    );

    let Ok(undo) = events(vec![pressing(Rail::Undo, page())]);
    let Ok(trace_2) = run_from::<Annotating>(trace.state, &undo);

    assert_eq!(
      written_on(&trace_2.state, ISHMAEL),
      Ok(Some(("Who".to_string(), Popup::Open))),
      "undo took back only the second writing"
    );
  }

  #[test]
  fn a_second_note_on_the_same_words_opens_the_first() {
    let Ok(typed) = written(
      "Who?",
      vec![
        Ok(AnnotationEvent::Closed(ISHMAEL)),
        a_note_on(Spot {
          start: 10,
          ..ISHMAEL
        }),
      ],
    );
    let Ok(trace) = run::<Annotating>(&(), &typed);

    assert_eq!(trace.state.notes.len(), 1);
    assert_eq!(
      written_on(&trace.state, ISHMAEL),
      Ok(Some(("Who?".to_string(), Popup::Open)))
    );
    assert_eq!(
      trace.state.writing.as_ref().map(|writing| writing.at),
      Some(ISHMAEL)
    );
  }

  #[test]
  fn b_backs_out_of_the_writing_then_the_popups_then_the_book() {
    let Ok(typed) = written("Who?", vec![Ok(AnnotationEvent::Typed(Typed::Finished))]);
    let Ok(trace) = run::<Annotating>(&(), &typed);

    assert_eq!(
      written_on(&trace.state, ISHMAEL),
      Ok(Some(("Who?".to_string(), Popup::Open))),
      "done writing leaves it open to read"
    );

    let Ok(page) = page();
    let Ok(closed) =
      run_from::<Annotating>(trace.state, &[AnnotationEvent::Backed(page.clone())]);

    assert_eq!(
      written_on(&closed.state, ISHMAEL),
      Ok(Some(("Who?".to_string(), Popup::Closed)))
    );
    assert!(
      closed
        .effects()
        .is_ok_and(|effects| !effects.contains(&AnnotationEffect::Back)),
      "the book stayed open"
    );

    let Ok(trace_2) = run_from::<Annotating>(closed.state, &[AnnotationEvent::Backed(page)]);

    assert_eq!(trace_2.effects(), Ok(vec![AnnotationEffect::Back]));
  }

  #[test]
  fn the_eraser_takes_the_note_whose_last_stroke_it_rubs_out() {
    let at = Spot {
      section: 0,
      block: 2,
      start: 0,
    };
    let strokes = vec![Stroke(vec![Point { x: 1, y: 1 }])];
    let Ok(handwriting) = noted(
      at,
      Mark::Handwriting {
        scope: Scope::Paragraph,
        size: 22,
        strokes,
      },
    );
    let Ok(unused) = noted(at, Mark::Highlight { end: 1 });
    let Ok(trace) = run::<Annotating>(
      &(),
      &[
        AnnotationEvent::Made(Made::Handwriting {
          joining: None,
          note: handwriting,
        }),
        AnnotationEvent::Made(Made::Handwriting {
          joining: Some(Stroke(vec![Point { x: 5, y: 5 }])),
          note: unused,
        }),
        AnnotationEvent::Touched(Touch::Erasing(Some(Target::Stroke { note: 0, stroke: 1 }))),
      ],
    );

    assert_eq!(
      trace.state.notes.len(),
      1,
      "a stroke near the last handwriting joined it, and rubbing it out left the first"
    );

    let Ok(after_stale) = run_from::<Annotating>(
      trace.state,
      &[AnnotationEvent::Moved(Touch::Erasing(Some(
        Target::Stroke { note: 0, stroke: 1 },
      )))],
    );
    assert_eq!(after_stale.state.notes.len(), 1);

    let Ok(trace_2) = run_from::<Annotating>(
      after_stale.state,
      &[AnnotationEvent::Moved(Touch::Erasing(Some(
        Target::Stroke { note: 0, stroke: 0 },
      )))],
    );

    assert_eq!(
      trace_2.state.notes,
      Vec::new(),
      "the last stroke took the note with it"
    );
  }

  #[test]
  fn y_walks_the_tools_in_the_order_they_stand_on_the_rail() {
    let Ok(trace) = run::<Annotating>(&(), &vec![AnnotationEvent::Walked; 5]);
    let mut tools = Vec::new();
    let Ok(mut state) = Annotations::empty();

    for _ in 0..5 {
      let Ok(next) = walked(state);

      tools.push(next.tool);
      state = next;
    }

    assert_eq!(trace.state.tool, Tool::Read, "five steps come back round");
    assert_eq!(
      tools,
      vec![
        Tool::Highlight,
        Tool::Text,
        Tool::Pen,
        Tool::Eraser,
        Tool::Read
      ]
    );
    assert_eq!(
      RAIL
        .iter()
        .filter_map(|rail| match rail {
          Rail::Tool(tool) => Some(*tool),
          Rail::Scope | Rail::Undo | Rail::Redo => None,
        })
        .collect::<Vec<Tool>>(),
      vec![
        Tool::Read,
        Tool::Highlight,
        Tool::Text,
        Tool::Pen,
        Tool::Eraser
      ]
    );
  }
}
