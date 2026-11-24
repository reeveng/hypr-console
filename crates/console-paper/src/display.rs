//! A lowest-layer, input-transparent surface for each active Wayland output.

use std::collections::BTreeMap;
use std::io;
use std::os::fd::BorrowedFd;
use std::time::Duration;

use console_core_geometry::Size;
use console_core_never::Never;
use console_paper::NAMESPACE;
use console_wayland::buffer::ShmBuffer;
use console_wayland::client::globals::{GlobalList, GlobalListContents};
use console_wayland::client::protocol::{
  wl_buffer, wl_callback, wl_compositor, wl_output, wl_region, wl_registry, wl_shm, wl_shm_pool,
  wl_surface,
};
use console_wayland::client::{
  Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop,
};
use console_wayland::protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use console_wayland::protocols_wlr::layer_shell::v1::client::{
  zwlr_layer_shell_v1 as shell, zwlr_layer_surface_v1 as layer,
};
use console_wayland::waiting;

use crate::picture::Drawing;
use crate::play::Display;

const MOST_OUTPUTS: u32 = 8;
const BUFFERS_PER_OUTPUT: u32 = 2;

pub struct Wall {
  connection: Connection,
  queue: EventQueue<Outputs>,
  outputs: Outputs,
}

struct Outputs {
  compositor: wl_compositor::WlCompositor,
  shm: wl_shm::WlShm,
  shell: shell::ZwlrLayerShellV1,
  viewporter: wp_viewporter::WpViewporter,
  active: BTreeMap<u32, Output>,
}

struct Output {
  source: wl_output::WlOutput,
  registry_name: u32,
  surface: wl_surface::WlSurface,
  layer: layer::ZwlrLayerSurfaceV1,
  viewport: wp_viewport::WpViewport,
  name: String,
  logical: Option<Size<i32>>,
  drawn: Option<Presented>,
  callback: Callback,
  buffers: Vec<Buffer>,
}

struct Presented {
  revision: u64,
  content: u64,
  logical: Size<i32>,
  description: String,
}

enum Callback {
  Ready,
  Pending,
}

struct Buffer {
  memory: ShmBuffer,
  size: Size<u32>,
  use_by: Use,
}

#[derive(Clone, Copy)]
enum Use {
  Client,
  Compositor,
}

impl Wall {
  pub fn open() -> io::Result<Self> {
    let connection = console_wayland::connect().map_err(io::Error::other)?;
    let (globals, queue) =
      console_wayland::registry::queue(&connection).map_err(io::Error::other)?;
    let hand = queue.handle();
    let mut outputs = Outputs::new(&globals, &hand)?;
    let registry = globals.registry();

    for global in globals.contents().clone_list() {
      match global.interface.as_str() {
        "wl_output" => {
          let Ok(()) = outputs.add(
            registry,
            &hand,
            OutputGlobal {
              name: global.name,
              version: global.version,
            },
          );
        }
        _ => {}
      }
    }

    Ok(Self {
      connection,
      queue,
      outputs,
    })
  }

  pub fn observe(&self, drawing: &Drawing) -> Result<Display, Never> {
    let mut availability = Display::Waiting;

    for output in self.outputs.active.values() {
      let Ok(ready) = output.availability(drawing);

      match ready {
        Display::Dirty => return Ok(Display::Dirty),
        Display::Ready => availability = Display::Ready,
        Display::Waiting => {}
      }
    }

    Ok(availability)
  }

  pub fn present(&mut self, drawing: &Drawing) -> io::Result<()> {
    let hand = self.queue.handle();

    for output in self.outputs.active.values_mut() {
      output.paint(&self.outputs.shm, &hand, drawing)?;
    }

    Ok(())
  }

  pub fn status(&self) -> Result<String, Never> {
    Ok(
      self
        .outputs
        .active
        .values()
        .filter_map(|output| output.drawn.as_ref().map(|drawn| (output, drawn)))
        .map(|(output, drawn)| {
          format!(
            "{}: currently displaying: {}\n",
            output.name, drawn.description
          )
        })
        .collect(),
    )
  }

  pub fn wait(&mut self, commands: BorrowedFd<'_>, until: Option<Duration>) -> io::Result<()> {
    waiting::wait(
      &self.connection,
      &mut self.queue,
      &mut self.outputs,
      &[commands],
      until,
    )
    .map_err(io::Error::from)
  }
}

struct OutputGlobal {
  name: u32,
  version: u32,
}

impl Outputs {
  fn new(globals: &GlobalList, hand: &QueueHandle<Self>) -> io::Result<Self> {
    let compositor =
      console_wayland::registry::compositor(globals, hand).map_err(io::Error::other)?;
    let shm =
      console_wayland::registry::shared_memory(globals, hand).map_err(io::Error::other)?;
    let shell = globals.bind(hand, 1..=4, ()).map_err(io::Error::other)?;
    let viewporter = globals.bind(hand, 1..=1, ()).map_err(io::Error::other)?;
    Ok(Self {
      compositor,
      shm,
      shell,
      viewporter,
      active: BTreeMap::new(),
    })
  }

  fn add(
    &mut self,
    registry: &wl_registry::WlRegistry,
    hand: &QueueHandle<Self>,
    offered: OutputGlobal,
  ) -> Result<(), Never> {
    let OutputGlobal { name, version } = offered;
    let Ok(limit) = console_core_number_conversion::index(MOST_OUTPUTS);

    match (self.active.contains_key(&name), self.active.len() >= limit) {
      (false, false) => {}
      _ => return Ok(()),
    }

    let source: wl_output::WlOutput = registry.bind(name, version.min(4), hand, name);
    let surface = self.compositor.create_surface(hand, ());
    let viewport = self.viewporter.get_viewport(&surface, hand, ());
    let layer = self.shell.get_layer_surface(
      &surface,
      Some(&source),
      shell::Layer::Background,
      NAMESPACE.to_owned(),
      hand,
      name,
    );
    layer.set_size(0, 0);
    layer.set_anchor(
      layer::Anchor::Top | layer::Anchor::Bottom | layer::Anchor::Left | layer::Anchor::Right,
    );
    layer.set_exclusive_zone(-1);
    let input = self.compositor.create_region(hand, ());
    surface.set_input_region(Some(&input));
    input.destroy();
    surface.commit();
    self.active.insert(
      name,
      Output {
        source,
        registry_name: name,
        surface,
        layer,
        viewport,
        name: format!("output-{name}"),
        logical: None,
        drawn: None,
        callback: Callback::Ready,
        buffers: Vec::new(),
      },
    );

    Ok(())
  }
}

impl Output {
  fn availability(&self, drawing: &Drawing) -> Result<Display, Never> {
    let logical = match self.logical {
      Some(configured) => configured,
      None => return Ok(Display::Waiting),
    };
    let Ok(limit) = console_core_number_conversion::index(BUFFERS_PER_OUTPUT);
    let spare = self.buffers.len() < limit
      || self
        .buffers
        .iter()
        .any(|slot| matches!(slot.use_by, Use::Client));

    Ok(match (spare, self.drawn.as_ref()) {
      (false, _) => Display::Waiting,
      (true, None) => Display::Dirty,
      (true, Some(drawn)) => {
        let source_changed = drawn.logical != logical || drawn.content != drawing.content;

        match (
          source_changed,
          &self.callback,
          drawn.revision == drawing.revision,
        ) {
          (true, _, _) | (false, Callback::Ready, false) => Display::Dirty,
          (false, Callback::Ready, true) => Display::Ready,
          (false, Callback::Pending, _) => Display::Waiting,
        }
      }
    })
  }

  fn paint(
    &mut self,
    shm: &wl_shm::WlShm,
    hand: &QueueHandle<Outputs>,
    drawing: &Drawing,
  ) -> io::Result<()> {
    let logical = match self.logical {
      Some(matched_logical) => matched_logical,
      ::core::option::Option::None => return Ok(()),
    };

    let Ok(ready) = self.availability(drawing);

    match ready {
      Display::Dirty => {}
      Display::Ready | Display::Waiting => return Ok(()),
    }

    let buffer = self.buffer(shm, hand, drawing.size)?;
    buffer.memory.write(&drawing.bytes)?;
    buffer.use_by = Use::Compositor;
    let Ok(memory_buffer) = buffer.memory.buffer();
    let proxy = memory_buffer.clone();
    self.surface.attach(Some(&proxy), 0, 0);
    let Ok((left, top, width, height)) = cropped(drawing.size, logical);
    self.viewport.set_source(left, top, width, height);
    self.viewport.set_destination(logical.width, logical.height);
    self.surface.damage(0, 0, logical.width, logical.height);

    match self.callback {
      Callback::Pending => {}
      Callback::Ready => {
        self.surface.frame(hand, self.registry_name);
        self.callback = Callback::Pending;
      }
    }

    self.surface.commit();

    match self.drawn.as_mut() {
      Some(shown) => match shown.content.cmp(&drawing.content) {
        std::cmp::Ordering::Equal => {
          shown.revision = drawing.revision;
          shown.logical = logical;
          return Ok(());
        }
        std::cmp::Ordering::Less | std::cmp::Ordering::Greater => {}
      },
      None => {}
    }

    let Ok(description) = drawing.describing();
    self.drawn = Some(Presented {
      revision: drawing.revision,
      content: drawing.content,
      logical,
      description,
    });

    Ok(())
  }

  fn buffer(
    &mut self,
    shm: &wl_shm::WlShm,
    hand: &QueueHandle<Outputs>,
    size: Size<u32>,
  ) -> io::Result<&mut Buffer> {
    self
      .buffers
      .retain(|slot| !matches!(slot.use_by, Use::Client) || slot.size == size);
    match self
      .buffers
      .iter()
      .position(|slot| matches!(slot.use_by, Use::Client))
    {
      Some(index) => self.buffers.get_mut(index),
      None => {
        let memory = ShmBuffer::new(shm, hand, size, wl_shm::Format::Xrgb8888)?;
        self.buffers.push(Buffer {
          memory,
          size,
          use_by: Use::Client,
        });
        self.buffers.last_mut()
      }
    }
    .ok_or_else(|| io::Error::other("wallpaper buffer is unavailable"))
  }
}

impl Drop for Output {
  fn drop(&mut self) {
    self.viewport.destroy();
    self.layer.destroy();
    self.surface.destroy();

    match self.source.version() {
      3.. => self.source.release(),
      _ => {}
    }
  }
}

fn cropped(source: Size<u32>, target: Size<i32>) -> Result<(f64, f64, f64, f64), Never> {
  let width = f64::from(source.width);
  let height = f64::from(source.height);
  let aspect = f64::from(target.width) / f64::from(target.height);
  let across = width.min(height * aspect).max(1.0);
  let down = height.min(width / aspect).max(1.0);
  Ok(((width - across) * 0.5, (height - down) * 0.5, across, down))
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Outputs {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    expect(
      explicit016_no_wildcard_arm,
      reason = "generated Wayland protocol events are non-exhaustive; every supported variant is already named"
    )
  )]
  fn event(
    state: &mut Self,
    registry: &wl_registry::WlRegistry,
    event: wl_registry::Event,
    _: &GlobalListContents,
    _: &Connection,
    hand: &QueueHandle<Self>,
  ) {
    match event {
      wl_registry::Event::Global {
        name,
        version,
        interface,
      } => match interface.as_str() {
        "wl_output" => {
          let Ok(()) = state.add(registry, hand, OutputGlobal { name, version });
        }
        _ => {}
      },
      wl_registry::Event::GlobalRemove { name } => {
        state.active.remove(&name);
      }
      _ => {}
    }
  }
}

impl Dispatch<wl_output::WlOutput, u32> for Outputs {
  fn event(
    state: &mut Self,
    _: &wl_output::WlOutput,
    event: wl_output::Event,
    key: &u32,
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match (state.active.get_mut(key), event) {
      (Some(output), wl_output::Event::Name { name }) => output.name = name,
      _ => {}
    }
  }
}

impl Dispatch<layer::ZwlrLayerSurfaceV1, u32> for Outputs {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    expect(
      explicit016_no_wildcard_arm,
      reason = "generated Wayland protocol events are non-exhaustive; every supported variant is already named"
    )
  )]
  fn event(
    state: &mut Self,
    layer: &layer::ZwlrLayerSurfaceV1,
    event: layer::Event,
    key: &u32,
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match event {
      layer::Event::Configure {
        serial,
        width,
        height,
      } => {
        layer.ack_configure(serial);

        match (
          state.active.get_mut(key),
          i32::try_from(width),
          i32::try_from(height),
        ) {
          (Some(output), Ok(width_2 @ 1..), Ok(height_2 @ 1..)) => {
            output.logical = Some(Size {
              width: width_2,
              height: height_2,
            });
          }
          _ => {}
        }
      }
      layer::Event::Closed => {
        state.active.remove(key);
      }
      _ => {}
    }
  }
}

impl Dispatch<wl_callback::WlCallback, u32> for Outputs {
  fn event(
    state: &mut Self,
    _: &wl_callback::WlCallback,
    _: wl_callback::Event,
    key: &u32,
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match state.active.get_mut(key) {
      Some(output) => output.callback = Callback::Ready,
      None => {}
    }
  }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for Outputs {
  fn event(
    state: &mut Self,
    released: &wl_buffer::WlBuffer,
    _: wl_buffer::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    for slot in state
      .active
      .values_mut()
      .flat_map(|output| &mut output.buffers)
    {
      let Ok(buffer) = slot.memory.buffer();

      match bool::then_some(buffer == released, ()) {
        Some(()) => slot.use_by = Use::Client,
        None => {}
      }
    }
  }
}

delegate_noop!(Outputs: ignore wl_compositor::WlCompositor);
delegate_noop!(Outputs: ignore wl_surface::WlSurface);
delegate_noop!(Outputs: ignore wl_region::WlRegion);
delegate_noop!(Outputs: ignore wl_shm::WlShm);
delegate_noop!(Outputs: ignore wl_shm_pool::WlShmPool);
delegate_noop!(Outputs: ignore shell::ZwlrLayerShellV1);
delegate_noop!(Outputs: ignore wp_viewporter::WpViewporter);
delegate_noop!(Outputs: ignore wp_viewport::WpViewport);
