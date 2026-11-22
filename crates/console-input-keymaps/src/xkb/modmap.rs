use crate::ModifierIndex;
use crate::ModifierMask;
use crate::xkb::interner::Interned;
use crate::xkb::span::Spanned;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use std::cell::Cell;
use std::ops::ControlFlow;
use std::ops::Deref;

type VirtualModifiers = Vec<Vmod>;

#[derive(Default, Debug)]
pub(crate) struct Vmodmap {
  mods: VirtualModifiers,
}

#[derive(Debug)]
pub(crate) struct Vmod {
  pub(crate) name: Spanned<Interned>,
  pub(crate) idx: ModifierIndex,
  pub(crate) def: Cell<Option<Spanned<ModifierMask>>>,
}

impl Vmodmap {
  pub(crate) fn insert(&mut self, name: Spanned<Interned>) -> Result<Option<&Vmod>, Never> {
    match self.mods.iter().position(|modifier| modifier.name.val == name.val) {
      Some(at) => return Ok(self.mods.get(at)),
      None => {}
    }

    let Ok(count) = fitted::<_, u32>(self.mods.len());
    let Ok(found) = ModifierIndex::new(count.saturating_add(8));

    Ok(match found {
      Some(modifier) => {
        self.mods.push(Vmod {
          name,
          idx: modifier,
          def: Default::default(),
        });
        self.mods.last()
      }
      None => None,
    })
  }

  pub(crate) fn get(&self, name: Interned) -> Result<Option<&Vmod>, Never> {
    Ok(self.mods.iter().find(|m| m.name.val == name))
  }

  fn effective(&self, modifier: ModifierIndex) -> Result<ModifierMask, Never> {
    let Ok(raw) = modifier.raw();
    let Ok(at) = index(raw.saturating_sub(8));
    let definition = self.mods.get(at).and_then(|vmod| vmod.def.get());

    Ok(match definition {
      Some(def) => def.val,
      None => ModifierMask::NONE,
    })
  }
}

impl Deref for Vmodmap {
  type Target = [Vmod];

  fn deref(&self) -> &Self::Target {
    &self.mods
  }
}

impl IntoIterator for Vmodmap {
  type Item = Vmod;
  type IntoIter = <VirtualModifiers as IntoIterator>::IntoIter;

  fn into_iter(self) -> Self::IntoIter {
    self.mods.into_iter()
  }
}

impl<'a> IntoIterator for &'a Vmodmap {
  type Item = &'a Vmod;
  type IntoIter = <&'a VirtualModifiers as IntoIterator>::IntoIter;

  fn into_iter(self) -> Self::IntoIter {
    self.mods.iter()
  }
}

#[derive(Clone, Debug)]
pub(crate) struct ModifierTree {
  root: ModifierNode,
  node_count: u64,
}

#[derive(Clone, Debug)]
enum ModifierNode {
  Num(ModifierMask),
  VMod(ModifierIndex),
  Not(Box<Spanned<ModifierTree>>),
  Add(Box<Spanned<ModifierTree>>, Box<Spanned<ModifierTree>>),
  Sub(Box<Spanned<ModifierTree>>, Box<Spanned<ModifierTree>>),
}

impl ModifierTree {
  #[must_use]
  pub(crate) fn number(mask: ModifierMask) -> Result<Self, Never> {
    Ok(Self {
      root: ModifierNode::Num(mask),
      node_count: 1,
    })
  }

  #[must_use]
  pub(crate) fn virtual_modifier(modifier: ModifierIndex) -> Result<Self, Never> {
    Ok(Self {
      root: ModifierNode::VMod(modifier),
      node_count: 1,
    })
  }

  pub(crate) fn invert(operand: Spanned<Self>) -> Result<Self, Never> {
    let node_count = operand.val.node_count.saturating_add(1);

    Ok(Self {
      root: ModifierNode::Not(Box::new(operand)),
      node_count,
    })
  }

  pub(crate) fn combine(
    operator: Combine,
    left: Spanned<Self>,
    right: Spanned<Self>,
  ) -> Result<Self, Never> {
    let node_count = left.val.node_count.saturating_add(right.val.node_count).saturating_add(1);
    let l = Box::new(left);
    let r = Box::new(right);
    let root = match operator {
      Combine::Join => ModifierNode::Add(l, r),
      Combine::Remove => ModifierNode::Sub(l, r),
    };

    Ok(Self { root, node_count })
  }

  pub(crate) fn get_effective(&self, mods: &Vmodmap) -> Result<ModifierMask, Never> {
    let Ok(eval_real) = self.eval_real(Some(mods));
    Ok(eval_real.0)
  }

  pub(crate) fn try_get_effective(&self, mods: &Vmodmap) -> Result<ModifierMask, ModifierMask> {
    let Ok(eval_real) = self.eval_real(Some(mods));
    let (res, ok) = eval_real;
    ok.then_some(res).ok_or(res)
  }

  pub(crate) fn eval_real(&self, mods: Option<&Vmodmap>) -> Result<(ModifierMask, bool), Never> {
    let Ok(eval) = self.eval(&|modifier| match mods {
      Some(table) => {
        let Ok(mask) = table.effective(modifier);
        mask
      }
      None => ModifierMask::NONE,
    });
    Ok(eval)
  }

  pub(crate) fn eval_virtual(&self) -> Result<(ModifierMask, bool), Never> {
    let Ok(eval) = self.eval(&|modifier| {
      let Ok(mask) = modifier.to_mask();
      mask
    });
    Ok(eval)
  }

  fn eval(&self, f: &impl Fn(ModifierIndex) -> ModifierMask) -> Result<(ModifierMask, bool), Never> {
    let steps = self.node_count.saturating_mul(3).saturating_add(1);
    let walked = (0..steps).try_fold((Vec::new(), Mode::Enter(self)), |(frames, mode), _| {
      let Ok(stepped) = step(frames, mode, f);
      stepped
    });

    Ok(match walked {
      ControlFlow::Break(value) => value,
      ControlFlow::Continue(_unfinished) => (ModifierMask::NONE, false),
    })
  }
}

type Value = (ModifierMask, bool);

enum Mode<'a> {
  Enter(&'a ModifierTree),
  Leave(Value),
}

#[derive(Copy, Clone)]
pub(crate) enum Combine {
  Join,
  Remove,
}

enum Frame<'a> {
  Invert,
  Then(Combine, &'a ModifierTree),
  With(Combine, Value),
}

type Walk<'a> = (Vec<Frame<'a>>, Mode<'a>);

fn step<'a>(
  mut frames: Vec<Frame<'a>>,
  mode: Mode<'a>,
  f: &impl Fn(ModifierIndex) -> ModifierMask,
) -> Result<ControlFlow<Value, Walk<'a>>, Never> {
  Ok(match mode {
    Mode::Enter(tree) => {
      let next = match &tree.root {
        ModifierNode::Num(n) => Mode::Leave((*n, true)),
        ModifierNode::VMod(v) => {
          let res = f(*v);
          Mode::Leave((res, res.0 != 0))
        }
        ModifierNode::Not(n) => {
          frames.push(Frame::Invert);
          Mode::Enter(&n.val)
        }
        ModifierNode::Add(l, r) => {
          frames.push(Frame::Then(Combine::Join, &r.val));
          Mode::Enter(&l.val)
        }
        ModifierNode::Sub(l, r) => {
          frames.push(Frame::Then(Combine::Remove, &r.val));
          Mode::Enter(&l.val)
        }
      };

      ControlFlow::Continue((frames, next))
    }
    Mode::Leave(value) => match frames.pop() {
      None => ControlFlow::Break(value),
      Some(Frame::Invert) => {
        let Ok(inverted) = invert(value);
        ControlFlow::Continue((frames, Mode::Leave(inverted)))
      }
      Some(Frame::Then(combine, right)) => {
        frames.push(Frame::With(combine, value));
        ControlFlow::Continue((frames, Mode::Enter(right)))
      }
      Some(Frame::With(combine, left)) => {
        let Ok(joined) = combined(combine, left, value);
        ControlFlow::Continue((frames, Mode::Leave(joined)))
      }
    },
  })
}

fn invert(value: Value) -> Result<Value, Never> {
  let (mut res, ok) = value;
  let real_only = res.0 & !0xff == 0;
  res.0 = !res.0;

  match ::core::primitive::bool::then_some(real_only, ()) {
    ::core::option::Option::Some(()) => {
      res.0 &= 0xff;
    }
    ::core::option::Option::None => {}
  }

  Ok((res, ok))
}

fn combined(combine: Combine, left: Value, right: Value) -> Result<Value, Never> {
  let (l, o1) = left;
  let (r, o2) = right;

  let res = match combine {
    Combine::Join => l | r,
    Combine::Remove => l & !r,
  };

  Ok((res, o1 && o2))
}
