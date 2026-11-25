//! Accessibility callbacks enqueue actions and lifecycle transitions. The panel
//! loop owns the published node identifiers and checks each queued action against
//! its current view. A full lifecycle mailbox requests a full tree because the
//! adapter may have transitioned while the panel was drawing.

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::hash::{BuildHasher, Hash, RandomState};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};

use accesskit::{
  Action, ActionData, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Node,
  NodeId, Rect, Role, Toggled, TreeId, TreeInfo, TreeUpdate,
};
use console_core_controls::Switch;
use console_core_geometry::Point;
use console_core_never::Never;
use console_draw_surface::KeyboardFocus;

use super::{Beside, Card, HitRegion, Lands, Owner, Panelled, Secret, State, Viewport, nth};
use crate::frames::{self, Notice};
use crate::page::{ActionOutcome, Control, Face, Heading, Row};

const ROOT: NodeId = NodeId(0);
const REQUEST_LIMIT: u16 = 16;

#[derive(Clone, Copy)]
enum LifecycleEvent {
  Activate,
  Deactivate,
}

#[derive(Clone)]
struct Lifecycle(SyncSender<LifecycleEvent>);

impl Lifecycle {
  fn notify(&self, event: LifecycleEvent) -> Result<(), Never> {
    match self.0.try_send(event) {
      Ok(()) | Err(TrySendError::Full(_)) => frames::tell(Notice::Accessibility),
      Err(TrySendError::Disconnected(_bridge_has_ended)) => Ok(()),
    }
  }
}

impl ActivationHandler for Lifecycle {
  fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
    let Ok(()) = self.notify(LifecycleEvent::Activate);
    None
  }
}

impl DeactivationHandler for Lifecycle {
  fn deactivate_accessibility(&mut self) {
    let Ok(()) = self.notify(LifecycleEvent::Deactivate);
  }
}

struct Requests(SyncSender<ActionRequest>);

impl ActionHandler for Requests {
  fn do_action(&mut self, request: ActionRequest) {
    let supported = matches!(
      request.action,
      Action::Click
        | Action::Focus
        | Action::Increment
        | Action::Decrement
        | Action::SetValue
        | Action::ScrollDown
        | Action::ScrollUp
    );

    match (request.target_tree == TreeId::ROOT && supported).then_some(()) {
      Some(()) => {}
      None => return,
    }

    match self.0.try_send(request) {
      Ok(()) | Err(TrySendError::Full(_)) => {
        let Ok(()) = frames::tell(Notice::Accessibility);
      }
      Err(TrySendError::Disconnected(_panel_has_ended)) => return,
    }
  }
}

pub(super) enum ViewChange {
  Unchanged,
  Different,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TreeChange {
  InitialTree,
  ChangedNodes,
}

#[derive(Clone, Copy)]
enum Publication {
  Inactive,
  Initial,
  Published,
}

impl Publication {
  fn advance(self, events: &Receiver<LifecycleEvent>) -> Result<Self, Never> {
    let mut next = self;
    let mut delivered = 0u16;
    let Ok(capacity) = console_core_number_conversion::index(REQUEST_LIMIT);

    for event in events.try_iter().take(capacity) {
      next = match event {
        LifecycleEvent::Activate => Self::Initial,
        LifecycleEvent::Deactivate => Self::Inactive,
      };
      delivered = delivered.saturating_add(1);
    }

    Ok(match (delivered == REQUEST_LIMIT).then_some(()) {
      Some(()) => Self::Initial,
      None => next,
    })
  }
}

pub(super) struct Bridge {
  adapter: accesskit_unix::Adapter,
  lifecycle: Receiver<LifecycleEvent>,
  requests: Receiver<ActionRequest>,
  ids: RandomState,
  published: Snapshot,
  publication: Publication,
  focus: KeyboardFocus,
  name: String,
}

impl Bridge {
  #[must_use]
  pub(super) fn new(name: &str) -> Result<Self, Never> {
    let Ok(capacity) = console_core_number_conversion::index(REQUEST_LIMIT);
    let (notify, lifecycle) = mpsc::sync_channel(capacity);
    let registration = Lifecycle(notify);
    let (send, requests) = mpsc::sync_channel(capacity);
    let adapter =
      accesskit_unix::Adapter::new(registration.clone(), Requests(send), registration);

    Ok(Self {
      adapter,
      lifecycle,
      requests,
      ids: RandomState::new(),
      published: Snapshot::default(),
      publication: Publication::Inactive,
      focus: KeyboardFocus::Elsewhere,
      name: name.to_owned(),
    })
  }

  pub(super) fn update(
    &mut self,
    change: ViewChange,
    focus: KeyboardFocus,
    state: &State,
    view: &Panelled,
  ) -> Result<(), Never> {
    match (focus != self.focus).then_some(()) {
      Some(()) => {
        self
          .adapter
          .update_window_focus_state(focus == KeyboardFocus::Here);
        self.focus = focus;
      }
      None => {}
    }

    let Ok(publication) = self.publication.advance(&self.lifecycle);
    self.publication = publication;

    match (publication, change) {
      (Publication::Inactive, _) => self.published = Snapshot::default(),
      (Publication::Published, ViewChange::Unchanged) => {}
      (Publication::Initial, _) | (Publication::Published, ViewChange::Different) => {
        let Ok(next) = Snapshot::from_panel(&self.ids, &self.name, state, view);
        let Ok(()) = self.publish(next);
      }
    }

    Ok(())
  }

  fn publish(&mut self, next: Snapshot) -> Result<(), Never> {
    let applied = Cell::new(None);
    let former = self.publication;
    let lifecycle = &self.lifecycle;
    let previous = &self.published;

    self.adapter.update_if_active(|| {
      let Ok(publication) = former.advance(lifecycle);
      let change = match publication {
        Publication::Inactive | Publication::Initial => TreeChange::InitialTree,
        Publication::Published => TreeChange::ChangedNodes,
      };
      let Ok(update) = next.changes(previous, change);
      applied.set(Some(Publication::Published));
      update
    });

    match applied.get() {
      Some(publication) => {
        self.publication = publication;
        self.published = next;
      }
      None => {
        self.publication = Publication::Inactive;
        self.published = Snapshot::default();
      }
    }

    Ok(())
  }

  #[must_use]
  pub(super) fn action(
    &mut self,
    focus: KeyboardFocus,
    view: &Panelled,
  ) -> Result<Option<Requested>, Never> {
    let Ok(publication) = self.publication.advance(&self.lifecycle);
    self.publication = publication;
    let Ok(limit) = console_core_number_conversion::index(REQUEST_LIMIT);

    for request in self.requests.try_iter().take(limit) {
      match (focus, self.publication) {
        (KeyboardFocus::Here, Publication::Published) => {}
        (KeyboardFocus::Here, Publication::Inactive | Publication::Initial)
        | (KeyboardFocus::Elsewhere, _) => continue,
      }

      let Ok(hit) = self.published.action(request, view);

      match hit {
        Some(action) => {
          let Ok(()) = frames::tell(Notice::Accessibility);
          return Ok(Some(action));
        }
        None => {}
      }
    }

    Ok(None)
  }
}

pub(super) struct Snapshot {
  nodes: HashMap<NodeId, Node>,
  targets: HashMap<NodeId, Target>,
  focus: NodeId,
}

#[derive(Clone, Copy)]
enum Target {
  Region(HitRegion),
  Choice { picker: HitRegion, which: u32 },
}

pub(super) struct Requested {
  pub(super) action: Action,
  target: Option<Target>,
  data: Option<ActionData>,
}

impl Default for Snapshot {
  fn default() -> Self {
    Self {
      nodes: HashMap::new(),
      targets: HashMap::new(),
      focus: ROOT,
    }
  }
}

#[derive(Default)]
struct RowNodes {
  parent: Option<NodeId>,
  controls: Vec<NodeId>,
}

impl Snapshot {
  fn insert(
    &mut self,
    ids: &RandomState,
    key: impl Hash,
    node: Node,
  ) -> Result<Option<NodeId>, Never> {
    let first = ids.hash_one(key);
    let Ok(count) = console_core_number_conversion::fitted::<_, u64>(self.nodes.len());
    let available = (0..count.saturating_add(2))
      .map(|offset| NodeId(first.wrapping_add(offset)))
      .find(|id| *id != ROOT && !self.nodes.contains_key(id));

    match available {
      Some(id) => {
        let _prior_node = self.nodes.insert(id, node);
      }
      None => {}
    }

    Ok(available)
  }

  #[must_use]
  pub(super) fn from_panel(
    ids: &RandomState,
    name: &str,
    state: &State,
    view: &Panelled,
  ) -> Result<Self, Never> {
    let mut snapshot = Self::default();
    let mut children = Vec::new();
    let mut rows = BTreeMap::new();
    let Ok(mut window) = named(Role::Window, name);

    match (state.asking.is_none() && state.sure.is_none() && !state.rows.is_empty()).then_some(())
    {
      Some(()) => {
        window.add_action(Action::ScrollDown);
        window.add_action(Action::ScrollUp);
      }
      None => {}
    }

    let Ok(prompt) = snapshot.prompt_node(ids, state);
    children.extend(prompt);

    for hit in &view.touching {
      let Ok(()) = snapshot.record_hit(ids, state, *hit, &mut rows, &mut children);
    }

    for (_, row) in rows {
      match row.parent {
        Some(id) => {
          match snapshot.nodes.get_mut(&id) {
            Some(node) => node.set_children(row.controls),
            None => {}
          }

          children.push(id);
        }
        None => children.extend(row.controls),
      }
    }

    window.set_children(children);
    let _prior_root = snapshot.nodes.insert(ROOT, window);
    Ok(snapshot)
  }

  fn prompt_node(&mut self, ids: &RandomState, state: &State) -> Result<Option<NodeId>, Never> {
    match (state.asking.as_ref(), state.sure.as_ref()) {
      (Some(prompt), _) => {
        let role = match prompt.secret {
          Secret::No => Role::TextInput,
          Secret::Hidden | Secret::Shown => Role::PasswordInput,
        };
        let Ok(mut node) = named(role, &prompt.question);

        match prompt.secret {
          Secret::No => {
            let Ok(value) = prompt.typed.text();
            let Ok(text) = safe_text(value);
            node.set_value(text);
          }
          Secret::Hidden | Secret::Shown => {}
        }

        let Ok(id) = self.insert(ids, ("prompt", prompt.identity), node);
        let Ok(()) = self.focus_node(id);
        Ok(id)
      }
      (None, Some(sure)) => {
        let Ok(mut node) = named(Role::Heading, &sure.question);
        let Ok(about) = safe_text(&sure.about);
        node.set_description(about);
        self.insert(ids, ("confirm", state.here, &sure.question), node)
      }
      (None, None) => self.search_node(ids, state),
    }
  }

  fn search_node(&mut self, ids: &RandomState, state: &State) -> Result<Option<NodeId>, Never> {
    let Ok(page) = nth(&state.pages, state.here);
    let search = match page.and_then(|current| current.sought.as_ref()) {
      Some(field) => field,
      None => return Ok(None),
    };

    let Ok(mut node) = named(Role::SearchInput, &search.about);
    let Ok(value) = state.typed.text();
    let Ok(text) = safe_text(value);
    node.set_value(text);
    let Ok(id) = self.insert(ids, ("search", state.here), node);
    let Ok(()) = self.focus_node(id);
    Ok(id)
  }

  fn focus_node(&mut self, id: Option<NodeId>) -> Result<(), Never> {
    match id {
      Some(node) => self.focus = node,
      None => {}
    }

    Ok(())
  }

  fn record_hit(
    &mut self,
    ids: &RandomState,
    state: &State,
    hit: HitRegion,
    rows: &mut BTreeMap<u32, RowNodes>,
    children: &mut Vec<NodeId>,
  ) -> Result<(), Never> {
    let Ok((_, owner)) = hit.lands.label_and_owner();
    let row = match owner {
      Owner::Row(index) => {
        let Ok(found) = nth(&state.rows, index);
        found.map(|selected| (index, selected))
      }
      Owner::Card => None,
    };
    let Ok(hit_node) = hit_node(state, hit.lands, row.map(|(_, selected)| selected));
    let mut node = match hit_node {
      Some(accessible) => accessible,
      None => return Ok(()),
    };
    let at = hit.panel.at;
    let size = hit.panel.size;
    node.set_bounds(Rect {
      x0: f64::from(at.x),
      y0: f64::from(at.y),
      x1: f64::from(at.x) + f64::from(size.width),
      y1: f64::from(at.y) + f64::from(size.height),
    });
    let Ok(region) = slot(hit.lands);

    match (hit.lands, row) {
      (Lands::Picker { .. }, Some((index, selected))) => {
        let Ok(key) = row_key(selected, index);
        let Ok(children_2) = self.picker_choices(ids, (state.here, key), hit, selected);
        node.set_children(children_2);
      }
      _ => {}
    }

    let Ok(inserted) = match (row, hit.lands) {
      (Some((index, selected)), _) => {
        let Ok(key) = row_key(selected, index);
        self.insert(ids, ("row", state.here, key, region), node)
      }
      (None, Lands::Reveal) => self.insert(ids, ("reveal", state.prompt_identity), node),
      (None, Lands::Answer(_)) => {
        let label = node.label().map_or("", |text| text).to_owned();
        self.insert(ids, ("answer", region, label), node)
      }
      (None, _) => self.insert(ids, ("panel", region), node),
    };
    let id = match inserted {
      Some(accessible) => accessible,
      None => return Ok(()),
    };
    let _prior_target = self.targets.insert(id, Target::Region(hit));

    match row {
      Some((index, _)) => {
        let entry = rows.entry(index).or_default();

        match (hit.lands == Lands::Row(index)).then_some(()) {
          Some(()) => entry.parent = Some(id),
          None => entry.controls.push(id),
        }
      }
      None => children.push(id),
    }

    let Ok(focus) = focused(state, hit.lands);

    match focus {
      KeyboardFocus::Here => self.focus = id,
      KeyboardFocus::Elsewhere => {}
    }

    Ok(())
  }

  fn picker_choices(
    &mut self,
    ids: &RandomState,
    (tab, key): (u32, RowKey<'_>),
    picker: HitRegion,
    row: &Row,
  ) -> Result<Vec<NodeId>, Never> {
    let choices = {
      let optional_choices = row.choices.as_ref();

      match optional_choices {
        Some(matched_choices) => matched_choices,
        ::core::option::Option::None => return Ok(Vec::new()),
      }
    };

    let mut children = Vec::new();

    for (index, value) in choices.said.iter().enumerate() {
      let Ok(which) = console_core_number_conversion::fitted::<_, u32>(index);
      let Ok(mut choice) = named(Role::RadioButton, value);
      choice.set_toggled(
        match ::core::primitive::bool::then_some(which == choices.chosen, ()) {
          ::core::option::Option::Some(()) => Toggled::True,
          ::core::option::Option::None => Toggled::False,
        },
      );

      match { ::core::primitive::bool::then_some(which != choices.chosen, ()) } {
        ::core::option::Option::Some(()) => {
          choice.add_action(Action::Click);
        }
        ::core::option::Option::None => {}
      }

      let Ok(inserted) = self.insert(ids, ("choice", tab, &key, which, value), choice);

      match inserted {
        Some(id) => {
          let _prior_target = self.targets.insert(id, Target::Choice { picker, which });
          children.push(id);
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(children)
  }

  #[must_use]
  pub(super) fn changes(&self, previous: &Self, change: TreeChange) -> Result<TreeUpdate, Never> {
    let initial = change == TreeChange::InitialTree || previous.nodes.is_empty();
    let nodes = self
      .nodes
      .iter()
      .filter(|(id, node)| initial || previous.nodes.get(id) != Some(*node))
      .map(|(id, node)| (*id, node.clone()))
      .collect();

    Ok(TreeUpdate {
      nodes,
      tree: initial.then(|| TreeInfo::new(ROOT)),
      tree_id: TreeId::ROOT,
      focus: self.focus,
    })
  }

  #[must_use]
  pub(super) fn action(
    &self,
    request: ActionRequest,
    view: &Panelled,
  ) -> Result<Option<Requested>, Never> {
    let node = match self.nodes.get(&request.target_node) {
      Some(target) => target,
      None => return Ok(None),
    };

    match (request.target_tree != TreeId::ROOT || !node.supports_action(request.action))
      .then_some(())
    {
      Some(()) => return Ok(None),
      None => {}
    }

    match (request.target_node == ROOT).then_some(()) {
      Some(()) => {
        return Ok(Some(Requested {
          action: request.action,
          target: None,
          data: request.data,
        }));
      }
      None => {}
    }

    let hit = self.targets.get(&request.target_node).filter(|target| {
      let hit = match target {
        Target::Region(hit) | Target::Choice { picker: hit, .. } => hit,
      };
      view.touching.contains(hit)
    });

    Ok(hit.map(|target| Requested {
      action: request.action,
      target: Some(*target),
      data: request.data,
    }))
  }
}

#[derive(Hash)]
enum RowKey<'a> {
  Key(&'a str),
  At(u32, &'a str, &'a str),
}

#[must_use]
fn row_key(row: &Row, index: u32) -> Result<RowKey<'_>, Never> {
  Ok(
    row
      .key
      .as_deref()
      .map_or(RowKey::At(index, &row.says, &row.aside), RowKey::Key),
  )
}

#[must_use]
fn slot(
  lands: Lands,
) -> ::core::result::Result<(&'static str, i64), ::console_core_never::Never> {
  ::core::result::Result::Ok(match lands {
    Lands::Row(_) => ("row", 0),
    Lands::Answer(index) => ("answer", i64::from(index)),
    Lands::Nudge { step, .. } => ("step", i64::from(step)),
    Lands::Else(_) => ("else", 0),
    Lands::ButtonPress { which, .. } => ("button", i64::from(which)),
    Lands::Seek { .. } => ("seek", 0),
    Lands::Slider { .. } => ("slider", 0),
    Lands::Picker { .. } => ("picker", 0),
    Lands::Tab(index) => ("tab", i64::from(index)),
    Lands::More(step) => ("more", i64::from(step)),
    Lands::Back => ("back", 0),
    Lands::Reveal => ("reveal", 0),
  })
}

#[must_use]
fn safe_text(text: &str) -> Result<String, Never> {
  Ok(text.replace('\0', "\u{fffd}"))
}

#[must_use]
fn named(role: Role, name: &str) -> Result<Node, Never> {
  let mut node = Node::new(role);
  let Ok(label) = safe_text(name);
  node.set_label(label);
  Ok(node)
}

#[must_use]
fn button(name: &str) -> Result<Node, Never> {
  let Ok(mut node) = named(Role::Button, name);
  node.add_action(Action::Click);
  Ok(node)
}

#[must_use]
fn hit_node(state: &State, lands: Lands, row: Option<&Row>) -> Result<Option<Node>, Never> {
  let Ok(node) = match lands {
    Lands::Tab(index) => {
      let Ok(found) = nth(&state.pages, index);
      let page = match found {
        Some(current) => current,
        None => return Ok(None),
      };
      let Ok(mut tab) = named(Role::Tab, &page.title);
      tab.set_selected(state.here == index);
      tab.add_action(Action::Click);
      Ok(tab)
    }
    Lands::Answer(index) => {
      let prompt = match state.sure.as_ref() {
        Some(confirming) => confirming,
        None => return Ok(None),
      };
      let Ok(found) = nth(&prompt.answers, index);
      let answer = match found {
        Some(value) => value,
        None => return Ok(None),
      };
      button(answer)
    }
    Lands::Back => button("Back"),
    Lands::Reveal => {
      let prompt = match state.asking.as_ref() {
        Some(value) => value,
        None => return Ok(None),
      };

      match prompt.secret {
        Secret::Hidden => button("Show password"),
        Secret::Shown => button("Hide password"),
        Secret::No => return Ok(None),
      }
    }
    Lands::More(step) => button(match (step < 0).then_some(()) {
      Some(()) => "Previous tabs",
      None => "Next tabs",
    }),
    Lands::Seek { .. } => return Ok(None),
    Lands::Row(_)
    | Lands::Else(_)
    | Lands::ButtonPress { .. }
    | Lands::Nudge { .. }
    | Lands::Slider { .. }
    | Lands::Picker { .. } => {
      let selected = match row {
        Some(value) => value,
        None => return Ok(None),
      };
      return hit_row_node(state, lands, selected);
    }
  };

  Ok(Some(node))
}

fn hit_row_node(state: &State, lands: Lands, row: &Row) -> Result<Option<Node>, Never> {
  let Ok(node) = match lands {
    Lands::Row(_) => row_node(row, &state.selected),
    Lands::Else(_) => button(&format!("More options for {}", row.says)),
    Lands::ButtonPress { which, .. } => {
      let buttons = match row.buttons.as_ref() {
        Some(group) => group,
        None => return Ok(None),
      };
      let Ok(found) = nth(&buttons.presses, which);
      let press = match found {
        Some(selected) => selected,
        None => return Ok(None),
      };
      let label = match press.face {
        Face::Written(text) => text.to_owned(),
        Face::Swatch(_) => format!("Color {}", which.saturating_add(1)),
        Face::Glyph(icon) => {
          let Ok(name) = icon.name();
          name.trim_end_matches("-symbolic").replace('-', " ")
        }
      };

      button(&label)
    }
    Lands::Nudge { step, .. } => {
      let Ok((less, more)) = crate::page::ends_of(row);
      button(match (step < 0).then_some(()) {
        Some(()) => less,
        None => more,
      })
    }
    Lands::Slider { .. } => {
      let slider = match row.control {
        Some(Control::Slider(value)) => value,
        Some(Control::Switch(_)) | None => return Ok(None),
      };
      let Ok(mut slider_node) = named(Role::Slider, &row.says);
      let Ok(at) = slider.at();
      let Ok(most) = slider.most();
      slider_node.set_numeric_value(f64::from(at));
      slider_node.set_min_numeric_value(0.0);
      slider_node.set_max_numeric_value(f64::from(most.get()));
      slider_node.set_numeric_value_step(1.0);
      slider_node.add_action(Action::SetValue);
      slider_node.add_action(Action::Increment);
      slider_node.add_action(Action::Decrement);
      Ok(slider_node)
    }
    Lands::Picker { .. } => named(Role::RadioGroup, &row.says),
    Lands::Tab(_)
    | Lands::Answer(_)
    | Lands::Back
    | Lands::Reveal
    | Lands::More(_)
    | Lands::Seek { .. } => return Ok(None),
  };

  Ok(Some(node))
}

#[must_use]
fn row_node(row: &Row, selected: &[String]) -> Result<Node, Never> {
  let Ok(heading) = row.heading();
  let title = row
    .headline
    .as_ref()
    .map_or(row.says.as_str(), |headline| &headline.title);
  let Ok(mut node) = match row.control {
    Some(Control::Switch(switch)) => {
      let Ok(mut toggle) = named(Role::Switch, title);
      toggle.set_toggled(match switch {
        Switch::On => Toggled::True,
        Switch::Off => Toggled::False,
      });
      Ok(toggle)
    }
    Some(Control::Slider(_)) | None => named(
      match heading {
        Heading::Yes => Role::Heading,
        Heading::No => Role::ListItem,
      },
      title,
    ),
  };

  match (!row.aside.is_empty()).then_some(()) {
    Some(()) => {
      let Ok(aside) = safe_text(&row.aside);
      node.set_description(aside);
    }
    None => {}
  }

  match row.key.as_ref() {
    Some(key) => node.set_selected(selected.contains(key)),
    None => {}
  }

  match heading {
    Heading::No => node.add_action(Action::Focus),
    Heading::Yes => {}
  }

  match (row.does.is_some() || row.buttons.is_some()).then_some(()) {
    Some(()) => node.add_action(Action::Click),
    None => {}
  }

  Ok(node)
}

#[must_use]
fn focused(state: &State, lands: Lands) -> Result<KeyboardFocus, Never> {
  let matches = match lands {
    Lands::Row(row) => {
      state.at == Some(row) && state.beside == Beside::No && state.pressing.is_none()
    }
    Lands::Else(row) => state.at == Some(row) && state.beside == Beside::Yes,
    Lands::ButtonPress { row, which } => state.pressing == Some((row, which)),
    Lands::Answer(index) => state.sure.as_ref().is_some_and(|sure| sure.at == index),
    Lands::Nudge { .. }
    | Lands::Seek { .. }
    | Lands::Slider { .. }
    | Lands::Picker { .. }
    | Lands::Tab(_)
    | Lands::More(_)
    | Lands::Back
    | Lands::Reveal => false,
  };

  Ok(match matches.then_some(()) {
    Some(()) => KeyboardFocus::Here,
    None => KeyboardFocus::Elsewhere,
  })
}

pub(super) fn perform(
  state: &mut State,
  card: &Card,
  room: i32,
  Requested {
    action,
    target,
    data,
  }: Requested,
) -> Result<ActionOutcome, Never> {
  Ok(match (action, target) {
    (Action::Click, Some(Target::Region(target_2))) => {
      let at = target_2.panel.at;
      let Ok(half_width) = console_core_number_conversion::fitted::<_, i32>(
        target_2.panel.size.width.saturating_div(2),
      );
      let Ok(half_height) = console_core_number_conversion::fitted::<_, i32>(
        target_2.panel.size.height.saturating_div(2),
      );
      let point = Point {
        x: at.x.saturating_add(half_width),
        y: at.y.saturating_add(half_height),
      };
      let Ok(outcome) = super::tapped(state, Some(target_2.lands), point, card);
      outcome
    }
    (
      Action::Click,
      Some(Target::Choice {
        picker: HitRegion {
          lands: Lands::Picker { row, .. },
          ..
        },
        which,
      }),
    ) => {
      state.at = Some(row);
      state.beside = Beside::No;
      let Ok(outcome) = super::carried_out(
        &mut state.model,
        &state.rows,
        super::Outcome::Picked { row, which },
      );
      outcome
    }
    (
      Action::Focus,
      Some(Target::Region(HitRegion {
        lands: Lands::Row(row),
        ..
      })),
    ) => {
      let Ok(()) = state.reveal_focus();
      state.at = Some(row);
      state.beside = Beside::No;
      state.pressing = None;
      ActionOutcome::Stay
    }
    (
      Action::Increment | Action::Decrement,
      Some(Target::Region(HitRegion {
        lands: Lands::Slider { row, .. },
        ..
      })),
    ) => {
      let step = match { ::core::primitive::bool::then_some(action == Action::Increment, ()) } {
        ::core::option::Option::Some(()) => 1,
        ::core::option::Option::None => -1,
      };
      state.at = Some(row);
      state.beside = Beside::No;
      let Ok(outcome) = super::nudge_level(&state.rows, row, step);
      let Ok(outcome_2) = super::carried_out(&mut state.model, &state.rows, outcome);
      outcome_2
    }
    (
      Action::SetValue,
      Some(Target::Region(HitRegion {
        lands: Lands::Slider { row, .. },
        ..
      })),
    ) => {
      let Ok(()) = set_level(&state.rows, row, data);

      ActionOutcome::Stay
    }
    (Action::ScrollUp | Action::ScrollDown, None) => {
      let page = room.max(crate::fitting::ROW);
      let offset = match { ::core::primitive::bool::then_some(action == Action::ScrollUp, ()) } {
        ::core::option::Option::Some(()) => page.saturating_neg(),
        ::core::option::Option::None => page,
      };
      let y = state.scroll.saturating_add(offset);
      let Ok(()) = super::scrolled_to(state, Viewport { room, y });
      ActionOutcome::Stay
    }
    _ => ActionOutcome::Stay,
  })
}

fn set_level(rows: &[Row], row: u32, data: Option<ActionData>) -> Result<(), Never> {
  let value = match data {
    Some(ActionData::NumericValue(value)) => value,
    Some(
      ActionData::CustomAction(_)
      | ActionData::Value(_)
      | ActionData::ScrollUnit(_)
      | ActionData::ScrollHint(_)
      | ActionData::ScrollToPoint(_)
      | ActionData::SetScrollOffset(_)
      | ActionData::SetTextSelection(_),
    )
    | None => return Ok(()),
  };
  let Ok(selected) = nth(rows, row);

  let (slider, level) = match selected.map(|entry| (entry.control, entry.level.as_ref())) {
    Some((Some(Control::Slider(slider)), Some(level))) => (slider, level),
    Some((Some(Control::Slider(_)), None) | (Some(Control::Switch(_)) | None, _)) | None => {
      return Ok(());
    }
  };

  match { ::core::primitive::bool::then_some(value.is_finite(), ()) } {
    ::core::option::Option::Some(()) => {
      let Ok(requested) = console_core_number_conversion::whole_u32(value);
      let Ok(most) = slider.most();
      let Ok(at) = slider.at();
      let Ok(from) = console_core_number_conversion::fitted::<u32, i32>(at);
      let Ok(to) = console_core_number_conversion::fitted::<u32, i32>(requested.min(most.get()));

      match { ::core::primitive::bool::then_some(from != to, ()) } {
        ::core::option::Option::Some(()) => {
          level(to.saturating_sub(from));
        }
        ::core::option::Option::None => {}
      }
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_full_lifecycle_batch_refreshes_the_whole_tree() {
    let Ok(capacity) = console_core_number_conversion::index(REQUEST_LIMIT);
    let (notify, lifecycle) = mpsc::sync_channel(capacity);

    for _ in 0..REQUEST_LIMIT {
      assert!(matches!(
        notify.try_send(LifecycleEvent::Deactivate),
        Ok(())
      ));
    }

    assert!(matches!(
      notify.try_send(LifecycleEvent::Activate),
      Err(TrySendError::Full(_))
    ));
    assert!(matches!(
      Publication::Published.advance(&lifecycle),
      Ok(Publication::Initial)
    ));
  }
}
