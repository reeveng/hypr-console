#![forbid(unsafe_code)]
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::src::const_fn::const_for;
use crate::src::enum_map::DefaultValue;
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use crate::src::levels::BlockLevel;
use console_core_never::Never;
use console_core_number_conversion::const_u8_index;
use core::ops::{BitOr, Shr};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub(crate) struct EdgeFlags(u8);

impl EdgeFlags {
  pub const EMPTY: Self = Self(0);
  pub const I444_TOP_HAS_RIGHT: Self = Self(1 << 0);
  const I422_TOP_HAS_RIGHT: Self = Self(1 << 1);
  pub const I420_TOP_HAS_RIGHT: Self = Self(1 << 2);

  pub const I444_LEFT_HAS_BOTTOM: Self = Self(1 << 3);
  const I422_LEFT_HAS_BOTTOM: Self = Self(1 << 4);
  pub const I420_LEFT_HAS_BOTTOM: Self = Self(1 << 5);

  pub const ALL_LEFT_HAS_BOTTOM: Self = Self(
    Self::I444_LEFT_HAS_BOTTOM.0 | Self::I422_LEFT_HAS_BOTTOM.0 | Self::I420_LEFT_HAS_BOTTOM.0,
  );
  pub const ALL_TOP_HAS_RIGHT: Self = Self(
    Self::I444_TOP_HAS_RIGHT.0 | Self::I422_TOP_HAS_RIGHT.0 | Self::I420_TOP_HAS_RIGHT.0,
  );
  pub const ALL_TR_AND_BL: Self = Self(Self::ALL_LEFT_HAS_BOTTOM.0 | Self::ALL_TOP_HAS_RIGHT.0);

  pub(crate) fn availability(self, required: Self) -> Result<EdgeAvailability, Never> {
    Ok(match (self.0 & required.0 == required.0).then_some(()) {
      Some(()) => EdgeAvailability::Present,
      None => EdgeAvailability::Missing,
    })
  }

  pub const fn union_all<const N: usize>(
    flags: [Self; N],
  ) -> core::result::Result<Self, console_core_never::Never> {
    let mut output = 0;
    const_for!(position in 0..N => {
      output |= flags[position].0;
    });

    Ok(Self(output))
  }

  pub(crate) const fn select(
    &self,
    availability: EdgeAvailability,
  ) -> Result<Self, Never> {
    Ok(match availability {
      EdgeAvailability::Present => *self,
      EdgeAvailability::Missing => Self::EMPTY,
    })
  }
}

pub(crate) enum EdgeAvailability {
  Present,
  Missing,
}

impl From<bool> for EdgeAvailability {
  fn from(available: bool) -> Self {
    match available.then_some(()) {
      Some(()) => Self::Present,
      None => Self::Missing,
    }
  }
}

impl BitOr for EdgeFlags {
  type Output = Self;

  fn bitor(self, other: Self) -> Self::Output {
    Self(self.0 | other.0)
  }
}

impl Shr<Rav1dPixelLayout> for EdgeFlags {
  type Output = Self;

  fn shr(self, rhs: Rav1dPixelLayout) -> Self::Output {
    let shift = match rhs {
      Rav1dPixelLayout::I400 => 7,
      Rav1dPixelLayout::I420 => 0,
      Rav1dPixelLayout::I422 => 1,
      Rav1dPixelLayout::I444 => 2,
    };
    Self(self.0.wrapping_shr(shift))
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
  Tip,
  Branch,
}

#[derive(Clone, Copy)]
pub struct EdgeIndex {
  index: u8,
  kind: EdgeKind,
}

impl EdgeIndex {
  pub const fn root() -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        index: 0,
        kind: EdgeKind::Branch,
      }
    })
  }

}

#[repr(C)]
pub struct EdgeNode {
  pub o: EdgeFlags,
  pub h: [EdgeFlags; 2],
  pub v: [EdgeFlags; 2],
}

#[repr(C)]
pub struct EdgeTip {
  pub node: EdgeNode,
  pub split: [EdgeFlags; 3],
}

#[repr(C)]
pub struct EdgeBranch {
  pub node: EdgeNode,
  pub h4: EdgeFlags,
  pub v4: EdgeFlags,
  pub split: [EdgeIndex; 4],
}

impl EdgeNode {
  const fn new(o: EdgeFlags, kind: EdgeKind) -> Result<Self, Never> {
    let EdgeFlags(left) = EdgeFlags::ALL_LEFT_HAS_BOTTOM;
    let EdgeFlags(top) = EdgeFlags::ALL_TOP_HAS_RIGHT;
    let (horizontal, vertical) = match kind {
      EdgeKind::Tip => (
        left | EdgeFlags::I420_TOP_HAS_RIGHT.0,
        top | EdgeFlags::I420_LEFT_HAS_BOTTOM.0 | EdgeFlags::I422_LEFT_HAS_BOTTOM.0,
      ),
      EdgeKind::Branch => (left, top),
    };
    let h = [EdgeFlags(o.0 | left), EdgeFlags(o.0 & horizontal)];
    let v = [EdgeFlags(o.0 | top), EdgeFlags(o.0 & vertical)];
    Ok(Self { o, h, v })
  }
}

impl EdgeTip {
  const fn new(edge_flags: EdgeFlags) -> Result<Self, Never> {
    let Ok(node) = EdgeNode::new(edge_flags, EdgeKind::Tip);
    let EdgeFlags(bits) = edge_flags;
    let split = [
      EdgeFlags((bits & EdgeFlags::ALL_TOP_HAS_RIGHT.0) | EdgeFlags::I422_LEFT_HAS_BOTTOM.0),
      EdgeFlags(bits | EdgeFlags::I444_TOP_HAS_RIGHT.0),
      EdgeFlags(bits & (EdgeFlags::I420_TOP_HAS_RIGHT.0 | EdgeFlags::I420_LEFT_HAS_BOTTOM.0
        | EdgeFlags::I422_LEFT_HAS_BOTTOM.0)),
    ];
    Ok(Self { node, split })
  }
}

impl EdgeBranch {
  const fn new(edge_flags: EdgeFlags, level: BlockLevel) -> Result<Self, Never> {
    let Ok(node) = EdgeNode::new(edge_flags, EdgeKind::Branch);
    let (top_right, bottom_left) = match level {
      BlockLevel::Bl16x16 => {
        let lower_chroma = EdgeFlags::I420_LEFT_HAS_BOTTOM.0 | EdgeFlags::I422_LEFT_HAS_BOTTOM.0;
        (edge_flags.0 & EdgeFlags::I420_TOP_HAS_RIGHT.0, edge_flags.0 & lower_chroma)
      }
      BlockLevel::Bl8x8 | BlockLevel::Bl32x32 | BlockLevel::Bl64x64 | BlockLevel::Bl128x128 => (0, 0),
    };
    let h4 = EdgeFlags(top_right | EdgeFlags::ALL_LEFT_HAS_BOTTOM.0);
    let v4 = EdgeFlags(bottom_left | EdgeFlags::ALL_TOP_HAS_RIGHT.0);
    let Ok(root) = EdgeIndex::root();
    Ok(Self { node, h4, v4, split: [root; 4] })
  }
}

impl DefaultValue for EdgeTip {
  const DEFAULT: Self = ({
    let Ok(explicit_value) = Self::new(EdgeFlags::EMPTY);
    explicit_value
  });
}

impl DefaultValue for EdgeBranch {
  const DEFAULT: Self = ({
    let Ok(explicit_value) = Self::new(EdgeFlags::EMPTY, BlockLevel::Bl128x128);
    explicit_value
  });
}

#[repr(C)]
struct IntraEdge<const N_BRANCH: usize, const N_TIP: usize> {
  branch: [EdgeBranch; N_BRANCH],
  tip: [EdgeTip; N_TIP],
}

const fn child_edge_flags(parent: EdgeFlags) -> Result<[EdgeFlags; 4], Never> {
  let EdgeFlags(top) = EdgeFlags::ALL_TOP_HAS_RIGHT;
  let EdgeFlags(left) = EdgeFlags::ALL_LEFT_HAS_BOTTOM;
  Ok([
    EdgeFlags::ALL_TR_AND_BL,
    EdgeFlags(parent.0 & top),
    EdgeFlags(top | (parent.0 & left)),
    EdgeFlags::EMPTY,
  ])
}

const fn split_children(first: u8, kind: EdgeKind) -> Result<[EdgeIndex; 4], Never> {
  Ok([
    EdgeIndex { index: first, kind },
    EdgeIndex { index: first.wrapping_add(1), kind },
    EdgeIndex { index: first.wrapping_add(2), kind },
    EdgeIndex { index: first.wrapping_add(3), kind },
  ])
}

impl<const N_BRANCH: usize, const N_TIP: usize> IntraEdge<N_BRANCH, N_TIP> {
  const fn new(superblock: SuperblockSize) -> Result<Self, Never> {
    let mut nodes = Self { branch: [EdgeBranch::DEFAULT; N_BRANCH], tip: [EdgeTip::DEFAULT; N_TIP] };
    let (mut level, expected_tips) = match superblock {
      SuperblockSize::Pixels64 => (BlockLevel::Bl64x64, 64u16),
      SuperblockSize::Pixels128 => (BlockLevel::Bl128x128, 256u16),
    };
    let Ok(root) = EdgeBranch::new(EdgeFlags::ALL_TOP_HAS_RIGHT, level);
    nodes.branch[0] = root;
    let mut next_branch = 1u8;
    let mut next_tip = 0u16;
    let mut level_size = 1u16;
    let mut remaining = level_size;

    const_for!(parent_index in 0..N_BRANCH => {
      let Ok(next_level) = level.decrease();
      let child_level = match next_level {
        Some(value) => value,
        None => BlockLevel::Bl8x8,
      };
      let (first, kind) = match child_level {
        BlockLevel::Bl8x8 => {
          let [ordinal, _] = next_tip.to_le_bytes();
          next_tip = next_tip.wrapping_add(4);
          (ordinal, EdgeKind::Tip)
        }
        BlockLevel::Bl16x16 | BlockLevel::Bl32x32 | BlockLevel::Bl64x64 | BlockLevel::Bl128x128 => {
          let ordinal = next_branch;
          next_branch = next_branch.wrapping_add(4);
          (ordinal, EdgeKind::Branch)
        }
      };
      let Ok(children) = split_children(first, kind);
      let Ok(flags) = child_edge_flags(nodes.branch[parent_index].node.o);
      nodes.branch[parent_index].split = children;

      const_for!(quadrant in 0..4 => {
        let child = children[quadrant];

        match child.kind {
          EdgeKind::Tip => {
            let Ok(tip) = EdgeTip::new(flags[quadrant]);
            nodes.tip[match const_u8_index(child.index) { Ok(position) => position }] = tip;
          }
          EdgeKind::Branch => {
            let Ok(branch) = EdgeBranch::new(flags[quadrant], child_level);
            nodes.branch[match const_u8_index(child.index) { Ok(position) => position }] = branch;
          }
        }
      });

      remaining = remaining.wrapping_sub(1);

      match remaining {
        0 => {
          level = child_level;
          level_size = level_size.wrapping_mul(4);
          remaining = level_size;
        }
        1.. => {}
      }
    });

    assert!(matches!(level, BlockLevel::Bl8x8));
    assert!(next_tip == expected_tips);
    assert!(matches!(const_u8_index(next_branch), Ok(size) if size == N_BRANCH));
    Ok(nodes)
  }

  fn edge<E, const N: usize>(edges: &[E; N], edge: EdgeIndex, kind: EdgeKind) -> Rav1dResult<&E> {
    match (edge.kind, kind) {
      (EdgeKind::Branch, EdgeKind::Branch) | (EdgeKind::Tip, EdgeKind::Tip) => {}
      (EdgeKind::Branch, EdgeKind::Tip) | (EdgeKind::Tip, EdgeKind::Branch) => return Err(EINVAL),
    }

    let Ok(index) = console_core_number_conversion::index(edge.index);
    edges.get(index).ok_or(EINVAL)
  }

  fn branch(&self, edge: EdgeIndex) -> Rav1dResult<&EdgeBranch> {
    Self::edge(&self.branch, edge, EdgeKind::Branch)
  }

  fn tip(&self, edge: EdgeIndex) -> Rav1dResult<&EdgeTip> {
    Self::edge(&self.tip, edge, EdgeKind::Tip)
  }

  fn node(&self, edge: EdgeIndex) -> Rav1dResult<&EdgeNode> {
    match edge.kind {
      EdgeKind::Branch => {
        let branch = self.branch(edge)?;
        Ok(&branch.node)
      }
      EdgeKind::Tip => {
        let tip = self.tip(edge)?;
        Ok(&tip.node)
      }
    }
  }
}

#[derive(Clone, Copy)]
pub(crate) enum SuperblockSize {
  Pixels64,
  Pixels128,
}

#[repr(C)]
pub struct IntraEdges {
  sb128: IntraEdge<85, 256>,
  sb64: IntraEdge<21, 64>,
}

impl IntraEdges {
  const fn new() -> Result<Self, Never> {
    let Ok(sb128) = IntraEdge::new(SuperblockSize::Pixels128);
    let Ok(sb64) = IntraEdge::new(SuperblockSize::Pixels64);
    Ok(Self { sb128, sb64 })
  }

  pub fn branch(&self, superblock_size: SuperblockSize, branch: EdgeIndex) -> Rav1dResult<&EdgeBranch> {
    match superblock_size {
      SuperblockSize::Pixels128 => self.sb128.branch(branch),
      SuperblockSize::Pixels64 => self.sb64.branch(branch),
    }
  }

  pub fn tip(&self, superblock_size: SuperblockSize, tip: EdgeIndex) -> Rav1dResult<&EdgeTip> {
    match superblock_size {
      SuperblockSize::Pixels128 => self.sb128.tip(tip),
      SuperblockSize::Pixels64 => self.sb64.tip(tip),
    }
  }

  pub fn node(&self, superblock_size: SuperblockSize, node: EdgeIndex) -> Rav1dResult<&EdgeNode> {
    match superblock_size {
      SuperblockSize::Pixels128 => self.sb128.node(node),
      SuperblockSize::Pixels64 => self.sb64.node(node),
    }
  }
}

impl DefaultValue for IntraEdges {
  const DEFAULT: Self = {
    let Ok(edges) = Self::new();
    edges
  };
}
