//! Variable-length codes decoded through a lookup table built once from a codebook.
//!
//! A codebook is a tree of blocks, each a fixed number of bits wide. A codeword is read a
//! block at a time: the bits index the block's table, and the entry found is either a value
//! with its length or a jump to the block beneath. The table lays the blocks out breadth
//! first, so a jump is an offset into one flat list, and a sparse codebook is one whose
//! blocks are allowed to have holes, which read as a value of nothing and no length.

use std::cmp::Ordering;
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::num::NonZeroU8;

use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use super::finished;

const MOST_BITS_PER_READ: NonZeroU8 = NonZeroU8::MIN.saturating_add(15);
const DEFAULT_BITS_PER_READ: NonZeroU8 = NonZeroU8::MIN.saturating_add(3);
const MAX_BLOCK_ENTRIES: u32 = 1 << MOST_BITS_PER_READ.get();

fn overflowed<T>() -> io::Result<T> {
  Err(io::Error::other("core (io): codebook overflow"))
}

fn incomplete<T>() -> io::Result<T> {
  Err(io::Error::other("core (io): codebook is incomplete"))
}

#[derive(Copy, Clone)]
pub enum BitOrder {
  Verbatim,
  Reverse,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EntryKind {
  Value,
  Jump,
}

impl From<bool> for EntryKind {
  fn from(jump: bool) -> Self {
    match bool::then_some(jump, ()) {
      Some(()) => Self::Jump,
      None => Self::Value,
    }
  }
}

pub trait CodebookEntry: Copy + Clone + Default {
  type ValueType: Copy + From<u8>;

  type OffsetType: Copy;

  const JUMP_OFFSET_MAX: u32;

  fn new_value(value: Self::ValueType, len: u8) -> Self;

  fn new_jump(offset: u32, len: u8) -> Self;

  fn kind(&self) -> EntryKind;

  fn value(&self) -> Self::ValueType;

  fn value_len(&self) -> u32;

  fn jump_offset(&self) -> u32;

  fn jump_len(&self) -> u32;
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "three packed codebook entries share seven bit-layout operations and differ only in width and flag values"
  )
)]
macro_rules! decl_entry {
  ($name:ident, $value_type:ty, $offset_type:ty, $offset_max:expr, $jump_flag:expr) => {
    #[derive(Copy, Clone, Default)]
    pub struct $name($value_type, $offset_type);

    impl CodebookEntry for $name {
      type ValueType = $value_type;
      type OffsetType = $offset_type;

      const JUMP_OFFSET_MAX: u32 = $offset_max;

      #[inline(always)]
      fn new_value(value: Self::ValueType, len: u8) -> Self {
        $name(value, <$offset_type>::from(len))
      }

      #[inline(always)]
      fn new_jump(offset: u32, len: u8) -> Self {
        let Ok(narrow) = fitted::<u32, $offset_type>(offset & $offset_max);

        $name(<$value_type>::from(len), $jump_flag | narrow)
      }

      #[inline(always)]
      fn kind(&self) -> EntryKind {
        EntryKind::from(self.1 & $jump_flag != 0)
      }

      #[inline(always)]
      fn value(&self) -> Self::ValueType {
        self.0
      }

      #[inline(always)]
      fn value_len(&self) -> u32 {
        u32::from(self.1 & !$jump_flag)
      }

      #[inline(always)]
      fn jump_offset(&self) -> u32 {
        u32::from(self.1 & !$jump_flag)
      }

      #[inline(always)]
      fn jump_len(&self) -> u32 {
        u32::from(self.0)
      }
    }
  };
}

decl_entry!(Entry8x16, u8, u16, 0x7fff, 0x8000);
decl_entry!(Entry16x16, u16, u16, 0x7fff, 0x8000);
decl_entry!(Entry32x32, u32, u32, 0x7fff_ffff, 0x8000_0000);

#[derive(Default)]
pub struct Codebook<E: CodebookEntry> {
  pub table: Vec<E>,
  pub max_code_len: u32,
  pub init_block_len: u32,
}

struct CodebookValue<E: CodebookEntry> {
  prefix: u16,
  width: u8,
  value: E::ValueType,
}

struct CodebookBlock<E: CodebookEntry> {
  width: u8,
  nodes: BTreeMap<u16, u32>,
  values: Vec<CodebookValue<E>>,
}

impl<E: CodebookEntry> Default for CodebookBlock<E> {
  fn default() -> Self {
    CodebookBlock {
      width: 0,
      nodes: BTreeMap::new(),
      values: Vec::new(),
    }
  }
}

struct Word<V> {
  code: u32,
  length: u8,
  value: V,
}

struct Layout<E> {
  queue: VecDeque<u32>,
  table: Vec<E>,
  end: u32,
}

struct Jump {
  base: u32,
  width: u8,
  prefix: u16,
  child: u32,
}

#[derive(Clone, Copy)]
enum Shape {
  Complete,
  Sparse,
}

enum Description {
  Consistent,
  LengthMismatch,
}

impl From<bool> for Description {
  fn from(consistent: bool) -> Self {
    match bool::then_some(consistent, ()) {
      Some(()) => Self::Consistent,
      None => Self::LengthMismatch,
    }
  }
}

pub struct CodebookBuilder {
  max_bits_per_block: NonZeroU8,
  bit_order: BitOrder,
  shape: Shape,
}

fn block<E: CodebookEntry>(
  blocks: &[CodebookBlock<E>],
  id: u32,
) -> io::Result<&CodebookBlock<E>> {
  let Ok(at) = index(id);

  match blocks.get(at) {
    Some(found) => Ok(found),
    None => incomplete(),
  }
}

fn child<E: CodebookEntry>(
  blocks: &mut Vec<CodebookBlock<E>>,
  parent: u32,
  prefix: u16,
  width: u8,
) -> io::Result<u32> {
  let Ok(fresh) = fitted::<_, u32>(blocks.len());
  let Ok(at) = index(parent);

  let found = blocks.get_mut(at).map(|held| {
    held.width = width;

    *held.nodes.entry(prefix).or_insert(fresh)
  });

  match found {
    Some(id) => {
      match id.cmp(&fresh) {
        Ordering::Equal => blocks.push(CodebookBlock::default()),
        Ordering::Less | Ordering::Greater => (),
      }

      Ok(id)
    }
    None => incomplete(),
  }
}

impl CodebookBuilder {
  pub fn new(bit_order: BitOrder) -> Result<Self, Never> {
    Ok(CodebookBuilder {
      max_bits_per_block: DEFAULT_BITS_PER_READ,
      bit_order,
      shape: Shape::Complete,
    })
  }

  pub fn new_sparse(bit_order: BitOrder) -> Result<Self, Never> {
    Ok(CodebookBuilder {
      max_bits_per_block: DEFAULT_BITS_PER_READ,
      bit_order,
      shape: Shape::Sparse,
    })
  }

  pub fn bits_per_read(&mut self, max_bits_per_read: u8) -> Result<&mut Self, Never> {
    self.max_bits_per_block = NonZeroU8::MIN
      .saturating_add(max_bits_per_read.saturating_sub(1))
      .min(MOST_BITS_PER_READ);

    Ok(self)
  }

  pub fn make<E: CodebookEntry>(
    &mut self,
    code_words: &[u32],
    code_lens: &[u8],
    values: &[E::ValueType],
  ) -> io::Result<Codebook<E>> {
    let description = Description::from(
      code_words.len() == code_lens.len() && code_words.len() == values.len(),
    );

    match (description, code_words) {
      (Description::LengthMismatch, _) => Err(io::Error::other(
        "core (io): codebook descriptions differ in length",
      )),
      (Description::Consistent, []) => Ok(Codebook {
        table: Vec::new(),
        max_code_len: 0,
        init_block_len: 0,
      }),
      (Description::Consistent, [_first, ..]) => self.made(code_words, code_lens, values),
    }
  }

  fn made<E: CodebookEntry>(
    &self,
    code_words: &[u32],
    code_lens: &[u8],
    values: &[E::ValueType],
  ) -> io::Result<Codebook<E>> {
    let mut blocks = vec![CodebookBlock::<E>::default()];
    let mut max_code_len = 0_u8;

    for ((code, length), value) in code_words.iter().zip(code_lens).zip(values) {
      self.planted(
        &mut blocks,
        Word {
          code: *code,
          length: *length,
          value: *value,
        },
      )?;

      max_code_len = max_code_len.max(*length);
    }

    let table = self.lookup_table(&blocks)?;

    let init_block_len = match table.first() {
      Some(entry) => entry.jump_len(),
      None => 0,
    };

    Ok(Codebook {
      table,
      max_code_len: u32::from(max_code_len),
      init_block_len,
    })
  }

  fn planted<E: CodebookEntry>(
    &self,
    blocks: &mut Vec<CodebookBlock<E>>,
    word: Word<E::ValueType>,
  ) -> io::Result<()> {
    match (word.length, self.shape) {
      (0, Shape::Sparse) => Ok(()),
      (0, Shape::Complete) => Err(io::Error::other("core (io): zero length codeword")),
      (_, Shape::Sparse | Shape::Complete) => self.grown(blocks, word),
    }
  }

  fn grown<E: CodebookEntry>(
    &self,
    blocks: &mut Vec<CodebookBlock<E>>,
    word: Word<E::ValueType>,
  ) -> io::Result<()> {
    let per = self.max_bits_per_block;
    let mask = !u32::MAX.wrapping_shl(u32::from(per.get()));
    let descents = word.length.saturating_sub(1) / per;
    let mut length = word.length;
    let mut parent = 0_u32;

    for _descent in 0..descents {
      length = length.saturating_sub(per.get());

      let Ok(prefix) = fitted::<u32, u16>(word.code.wrapping_shr(u32::from(length)) & mask);
      let next = child(blocks, parent, prefix, per.get())?;

      parent = next;
    }

    let Ok(prefix) = fitted::<u32, u16>(
      word.code & mask.wrapping_shr(u32::from(per.get().saturating_sub(length))),
    );
    let Ok(at) = index(parent);

    match blocks.get_mut(at) {
      Some(held) => {
        held.values.push(CodebookValue {
          prefix,
          width: length,
          value: word.value,
        });
        held.width = held.width.max(length);

        Ok(())
      }
      None => incomplete(),
    }
  }

  fn placed(&self, prefix: u32, width: u8) -> Result<u32, Never> {
    Ok(match self.bit_order {
      BitOrder::Verbatim => prefix,
      BitOrder::Reverse => prefix.reverse_bits().rotate_left(u32::from(width)),
    })
  }

  fn lookup_table<E: CodebookEntry>(&self, blocks: &[CodebookBlock<E>]) -> io::Result<Vec<E>> {
    let root = block(blocks, 0)?;
    let first = E::new_jump(1, root.width);
    let end = 1_u32.saturating_add(1_u32.wrapping_shl(u32::from(root.width)));
    let layout = Layout {
      queue: VecDeque::from([0]),
      table: vec![first],
      end,
    };

    let stepped = iterate(layout, |mut layout_2| {
      Ok(match layout_2.queue.pop_front() {
        Some(id) => match self.laid(blocks, layout_2, id) {
          Ok(laid) => Step::Again(laid),
          Err(fault) => Step::Halt(Err(fault)),
        },
        None => Step::Halt(Ok(layout_2.table)),
      })
    });

    finished(stepped)
  }

  fn laid<E: CodebookEntry>(
    &self,
    blocks: &[CodebookBlock<E>],
    mut layout: Layout<E>,
    id: u32,
  ) -> io::Result<Layout<E>> {
    let laying = block(blocks, id)?;
    let Ok(base) = fitted::<_, u32>(layout.table.len());
    let span = 1_u32.wrapping_shl(u32::from(laying.width));
    let needed = base.saturating_add(span);
    let bounded = needed.min(E::JUMP_OFFSET_MAX.saturating_add(MAX_BLOCK_ENTRIES));

    match needed.cmp(&bounded) {
      Ordering::Equal => {}
      Ordering::Less | Ordering::Greater => return overflowed(),
    }

    let empty = E::default();

    layout.table.extend((base..bounded).map(|_| empty));

    for (prefix, beneath) in laying.nodes.iter() {
      self.jump_laid(
        blocks,
        &mut layout,
        Jump {
          base,
          width: laying.width,
          prefix: *prefix,
          child: *beneath,
        },
      )?;
    }

    let Ok(jumps) = fitted::<_, u32>(laying.nodes.len());
    let values = self.values_laid(&mut layout.table, laying, base)?;

    match (self.shape, jumps.saturating_add(values).cmp(&span)) {
      (Shape::Sparse, Ordering::Less | Ordering::Equal | Ordering::Greater)
      | (Shape::Complete, Ordering::Equal) => Ok(layout),
      (Shape::Complete, Ordering::Less | Ordering::Greater) => incomplete(),
    }
  }

  fn jump_laid<E: CodebookEntry>(
    &self,
    blocks: &[CodebookBlock<E>],
    layout: &mut Layout<E>,
    jump: Jump,
  ) -> io::Result<()> {
    layout.queue.push_back(jump.child);

    let beneath = block(blocks, jump.child)?;

    match layout.end.cmp(&E::JUMP_OFFSET_MAX) {
      Ordering::Greater => overflowed(),
      Ordering::Less | Ordering::Equal => {
        let Ok(offset) = self.placed(u32::from(jump.prefix), jump.width);
        let Ok(at) = index(jump.base.saturating_add(offset));
        let slot = layout
          .table
          .get_mut(at)
          .ok_or_else(|| io::Error::other("core (io): codebook is incomplete"))?;

        *slot = E::new_jump(layout.end, beneath.width);
        layout.end = layout
          .end
          .saturating_add(1_u32.wrapping_shl(u32::from(beneath.width)));

        Ok(())
      }
    }
  }

  fn values_laid<E: CodebookEntry>(
    &self,
    table: &mut [E],
    laying: &CodebookBlock<E>,
    base: u32,
  ) -> io::Result<u32> {
    let mut laid = 0_u32;

    for value in laying.values.iter() {
      let spare = u32::from(laying.width.saturating_sub(value.width));
      let first = u32::from(value.prefix).wrapping_shl(spare);
      let count = 1_u32.wrapping_shl(spare);
      let entry = E::new_value(value.value, value.width);

      for prefix in first..first.saturating_add(count) {
        let Ok(offset) = self.placed(prefix, laying.width);
        let Ok(at) = index(base.saturating_add(offset));
        let slot = table
          .get_mut(at)
          .ok_or_else(|| io::Error::other("core (io): codebook is incomplete"))?;

        *slot = entry;
      }

      laid = laid.saturating_add(count);
    }

    Ok(laid)
  }
}
