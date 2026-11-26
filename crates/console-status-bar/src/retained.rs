//! Prepared device pixels for each sound icon in the current bar scene.

use std::io;

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use console_core_shapes::Shape;
use console_draw_painting::{Frame, onto};
use console_draw_surface::Part;
use console_draw_surface::retained::{Area, Geometry, Patch, Retained, Tile};

use crate::measuring::{compact, every, sized};
use crate::preferences::{Placement, Preferences};
use crate::reading::{Reading, StatusItem, Tone};
use crate::settings::SettingsTab;
use crate::showing::{self, BarAction, Filling, Fitting, Rendered, Slot, Wearing};
use crate::state::BarState;

#[derive(Clone, Copy)]
pub struct Scene<'a> {
  pub held: &'a BarState,
  pub preferences: &'a Preferences,
  pub slots: &'a [Slot],
  pub wearing: Wearing,
  pub room: Size<u32>,
  pub fitting: Fitting,
  pub filling: Filling,
  pub focus: Option<BarAction>,
}

impl Scene<'_> {
  pub fn render(self) -> Result<Rendered, Never> {
    let Ok(layout) = self.held.configured(self.preferences, self.filling);
    let Ok(mut bar) = sized(&layout, self.fitting);
    let Ok(shortcuts) = self.held.settings_applets(self.preferences);
    let Ok(mut applets) = every(&shortcuts, self.fitting);
    let Ok(published) = every(self.slots, self.fitting);
    applets.extend(published);
    let placement = self.preferences.placement;

    let Ok(mut rendered) = match placement {
      Placement::Top | Placement::Bottom => {
        showing::along_with_applets(&bar, &applets, &self.wearing, self.room, placement)
      }
      Placement::Left | Placement::Right => {
        let Ok(width) = self.fitting.sidebar_content_width(self.room.width);

        for group in [&mut bar.left, &mut bar.middle, &mut bar.right, &mut applets] {
          let Ok(()) = compact(group, self.fitting, width);
        }

        showing::sidebar(&bar, &applets, &self.wearing, self.room, placement)
      }
    };

    match self
      .focus
      .and_then(|action| rendered.touching.iter().find(|hit| hit.action == action))
    {
      Some(hit) => {
        let Ok(outline) = hit.panel.focus_outline(self.wearing.text);
        rendered.shapes.extend(outline.map(Shape::Line));
      }
      None => {}
    }

    Ok(rendered)
  }
}

pub fn pixels(drawn: &Rendered, device: Size<u32>) -> io::Result<Vec<u8>> {
  let bytes = u64::from(device.width)
    .saturating_mul(u64::from(device.height))
    .saturating_mul(4);

  match bytes {
    0..=2_147_483_647 => {}
    _ => return Err(io::Error::from(io::ErrorKind::InvalidInput)),
  }

  let Ok(length) = index(bytes);
  let mut pixels = Vec::new();
  pixels
    .try_reserve_exact(length.min(2_147_483_647))
    .map_err(io::Error::other)?;
  pixels.resize(pixels.capacity().min(length), 0);
  let frame = Frame {
    device,
    points: drawn.room,
  };
  onto(&mut pixels, frame, &drawn.shapes).map_err(io::Error::other)?;

  Ok(pixels)
}

#[must_use]
fn sound(state: &BarState) -> Result<Option<&Reading>, Never> {
  Ok(
    state
      .readings
      .iter()
      .find_map(|(item, reading)| match item {
        StatusItem::Sound => Some(reading),
        StatusItem::Battery | StatusItem::Bluetooth | StatusItem::Network => None,
      }),
  )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Volume {
  Unavailable,
  Silent,
  Low,
  Medium,
  High,
}

const ALL: [Volume; 5] = [
  Volume::Unavailable,
  Volume::Silent,
  Volume::Low,
  Volume::Medium,
  Volume::High,
];

impl Volume {
  pub fn reading(self) -> Result<Reading, Never> {
    let Ok((icon, tone)) = self.appearance();

    Reading::new(icon, tone)
  }

  #[must_use]
  fn appearance(self) -> Result<(&'static str, Tone), Never> {
    Ok(match self {
      Self::Unavailable => (crate::reading::SILENT, Tone::Plain),
      Self::Silent => (crate::reading::SILENT, Tone::Secondary),
      Self::Low => ("\u{f057f}", Tone::Plain),
      Self::Medium => ("\u{f0580}", Tone::Plain),
      Self::High => ("\u{f057e}", Tone::Plain),
    })
  }

  pub fn observed(state: &BarState) -> Result<Option<Self>, Never> {
    let Ok(observed) = sound(state);
    Ok(observed.and_then(|reading| {
      ALL.iter().copied().find(|volume| {
        let Ok((icon, tone)) = volume.appearance();

        reading.beside.is_none() && reading.icon == icon && reading.tone == tone
      })
    }))
  }
}

pub struct VolumeTiles {
  patches: Vec<VolumePixels>,
  placement: Option<Part>,
}

struct VolumePixels {
  volume: Volume,
  patch: Patch,
  tile: Option<Patch>,
}

impl VolumeTiles {
  pub fn prepare(scene: Scene<'_>, device: Size<u32>) -> io::Result<Option<Self>> {
    let Ok(base_volume) = Volume::observed(scene.held);

    match base_volume {
      Some(_) => {}
      None => return Ok(None),
    }

    let Ok(baseline) = scene.render();
    let variants = variants(scene, &baseline, device)?;
    let Variants { painted, damage } = match variants {
      Some(ready) => ready,
      None => return Ok(None),
    };
    let area = match damage {
      Some(area) => area,
      None => {
        return Ok(Some(Self {
          patches: Vec::new(),
          placement: None,
        }));
      }
    };
    let Ok(owning_slot) = sound_bounds(&baseline, device);
    let bounds = match owning_slot {
      Some(bounds) => bounds,
      None => return Ok(None),
    };

    match (area.left >= bounds.left
      && area.top >= bounds.top
      && area.right <= bounds.right
      && area.bottom <= bounds.bottom)
      .then_some(())
    {
      Some(()) => {}
      None => return Ok(None),
    }

    let Ok(aligned) = Tile::aligned(
      Geometry {
        logical: baseline.room,
        device,
      },
      area,
      bounds,
    );
    let mut patches = Vec::with_capacity(painted.len());

    for (volume, image) in painted {
      let patch = Patch::extract(device, &image, area)?;
      let tile = match aligned {
        Some(tile) => {
          let packed = Patch::extract(device, &image, tile.area)?;
          Some(packed)
        }
        None => None,
      };
      patches.push(VolumePixels {
        volume,
        patch,
        tile,
      });
    }

    Ok(Some(Self {
      patches,
      placement: aligned.map(|tile| tile.part),
    }))
  }

  pub fn update(&self, volume: Volume, retained: &mut Retained) -> io::Result<()> {
    let Ok(prepared) = self.pixels_for(volume);

    match prepared {
      Some(pixels) => retained.patch(&pixels.patch),
      None => Ok(()),
    }
  }

  pub fn tile(&self, volume: Volume) -> Result<Option<(Part, &Patch)>, Never> {
    let Ok(prepared) = self.pixels_for(volume);

    Ok(
      self
        .placement
        .zip(prepared.and_then(|pixels| pixels.tile.as_ref())),
    )
  }

  fn pixels_for(&self, volume: Volume) -> Result<Option<&VolumePixels>, Never> {
    Ok(self.patches.iter().find(|pixels| pixels.volume == volume))
  }
}

struct Variants {
  painted: Vec<(Volume, Vec<u8>)>,
  damage: Option<Area>,
}

fn variants(
  scene: Scene<'_>,
  baseline: &Rendered,
  device: Size<u32>,
) -> io::Result<Option<Variants>> {
  let baseline_pixels = pixels(baseline, device)?;
  let mut damage: Option<Area> = None;
  let mut painted = Vec::with_capacity(ALL.len());
  let mut held = scene.held.clone();
  let located = held
    .readings
    .iter()
    .position(|(item, _)| *item == StatusItem::Sound)
    .map(|position| {
      let Ok(offset) = fixed_index(position);
      offset
    });
  let position = located.ok_or(io::ErrorKind::InvalidData)?;
  let Ok(offset) = index(position);

  for volume in ALL {
    let Ok(reading) = volume.reading();
    let (_, current_reading) = held
      .readings
      .get_mut(offset)
      .ok_or(io::ErrorKind::InvalidData)?;
    *current_reading = reading;
    let Ok(drawn) = (Scene {
      held: &held,
      ..scene
    })
    .render();

    match (baseline.touching == drawn.touching).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    let image = pixels(&drawn, device)?;
    let Ok(changed) = Area::between(device, &baseline_pixels, &image);
    damage = match (damage, changed) {
      (Some(previous), Some(area)) => {
        let Ok(combined) = previous.union(area);
        Some(combined)
      }
      (previous, None) | (None, previous) => previous,
    };
    painted.push((volume, image));
  }

  Ok(Some(Variants { painted, damage }))
}

fn sound_bounds(baseline: &Rendered, device: Size<u32>) -> Result<Option<Area>, Never> {
  let hit = baseline
    .touching
    .iter()
    .find(|hit| hit.action == BarAction::Settings(SettingsTab::Sound));
  let panel = match hit {
    Some(sound) => sound.panel,
    None => return Ok(None),
  };
  let geometry = Geometry {
    logical: baseline.room,
    device,
  };

  geometry.area(Part {
    at: panel.at,
    size: panel.size,
  })
}
