//! The Servo process drives its own event loop and publishes sealed RGBA frames.

#![forbid(unsafe_code)]

use std::cell::Cell;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Arc;

use console_browser_renderer::{self as channel, MAX_SIDE, invalid};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::nearest_f32_from_f64;
use console_core_web_engine::{
  Event, FrameLayout, READER_SCRIPT, Request, decoded_request, encoded_event,
};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::pipe::{PipeFlags, pipe_with};
use servo::{
  Code, DeviceIntPoint, DeviceIntRect, DeviceIntSize, DevicePoint, EventLoopWaker, InputEvent,
  JSValue, Key, KeyState, KeyboardEvent, LoadStatus, Location, Modifiers, MouseButton,
  MouseButtonAction, MouseButtonEvent, PermissionRequest, Preferences, RenderingContext, Servo,
  ServoBuilder, SoftwareRenderingContext, Theme, WebView, WebViewBuilder, WebViewDelegate,
  WebViewPoint, WheelDelta, WheelEvent, WheelMode,
};
use url::Url;

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Servo clones the event-loop waker across threads and each clone needs the same notification pipe"
  )
)]
#[derive(Clone)]
struct Wake(Arc<OwnedFd>);

impl EventLoopWaker for Wake {
  fn clone_box(&self) -> Box<dyn EventLoopWaker> {
    Box::new(self.clone())
  }

  #[doc = "A full nonblocking pipe already has a wake queued."]
  fn wake(&self) {
    let _notification = rustix::io::write(self.0.as_ref(), &[1]);
  }
}

#[derive(Clone, Copy, Default)]
enum FrameSignal {
  #[default]
  Idle,
  Pending,
}

enum FramePublication {
  Available,
  AwaitingRelease,
}

#[derive(Default)]
struct Updates {
  events: Cell<Vec<Event>>,
  frame: Cell<FrameSignal>,
  crashed: Cell<Option<String>>,
}

impl Updates {
  fn record(&self, event: Event) -> Result<(), Never> {
    let mut events = self.events.take();
    events.push(event);
    self.events.set(events);
    Ok(())
  }
}

impl WebViewDelegate for Updates {
  fn request_permission(&self, _: WebView, request: PermissionRequest) {
    request.deny();
  }

  fn notify_url_changed(&self, _: WebView, url: Url) {
    let Ok(()) = self.record(Event::Address(url.to_string()));
  }

  fn notify_page_title_changed(&self, _: WebView, title: Option<String>) {
    let displayed = match title {
      Some(page_title) => page_title,
      None => String::new(),
    };

    let Ok(()) = self.record(Event::Title(displayed));
  }

  fn notify_load_status_changed(&self, _: WebView, status: LoadStatus) {
    let event = match status {
      LoadStatus::Started => Event::Started,
      LoadStatus::Complete | LoadStatus::Stopped => Event::Finished,
      LoadStatus::HeadParsed => return,
    };

    let Ok(()) = self.record(event);
  }

  fn notify_new_frame_ready(&self, _: WebView) {
    self.frame.set(FrameSignal::Pending);
  }

  fn notify_crashed(&self, _: WebView, reason: String, _: Option<String>) {
    self.crashed.set(Some(reason));
  }
}

fn bounded(size: Size<u32>) -> Result<(u32, u32), Never> {
  Ok((size.width.clamp(1, MAX_SIDE), size.height.clamp(1, MAX_SIDE)))
}

fn event(socket: BorrowedFd<'_>, event: Event, frame: Option<&OwnedFd>) -> io::Result<()> {
  let Ok(line) = encoded_event(&event);
  channel::send(socket, &line, frame)
}

fn packet(socket: BorrowedFd<'_>) -> io::Result<String> {
  let packet = channel::receive(socket)?;

  match packet.frame {
    None => Ok(packet.line),
    Some(_unexpected_memory) => Err(invalid("a renderer request carried memory")),
  }
}

fn send_frame(
  socket: BorrowedFd<'_>,
  view: &WebView,
  context: &SoftwareRenderingContext,
) -> io::Result<FramePublication> {
  view.paint();
  let rendered = context.size();
  let width = i32::try_from(rendered.width).map_err(|_outside_device_width| invalid("invalid Servo frame size"))?;
  let height = i32::try_from(rendered.height).map_err(|_outside_device_height| invalid("invalid Servo frame size"))?;
  let area = DeviceIntRect::from_origin_and_size(
    DeviceIntPoint::zero(),
    DeviceIntSize::new(width, height),
  );
  let image = context.read_to_image(area);
  context.present();

  let frame_image = match image {
    Some(rendered_image) => rendered_image,
    None => return Ok(FramePublication::Available),
  };
  let size = Size {
    width: frame_image.width(),
    height: frame_image.height(),
  };
  let layout = FrameLayout::rgba(size)
    .map_err(|_invalid_layout| invalid("invalid Servo frame layout"))?;
  let _validated_pixels = layout.pixels(frame_image.as_raw())
    .map_err(|_wrong_buffer_length| invalid("incomplete Servo frame"))?;
  let frame = channel::sealed(frame_image.as_raw())?;
  event(socket, Event::Frame(size), Some(&frame))?;

  Ok(FramePublication::AwaitingRelease)
}

fn input_key(view: &WebView, key: Key) -> Result<(), Never> {
  for state in [KeyState::Down, KeyState::Up] {
    let event = KeyboardEvent::new_without_event(
      state,
      key.clone(),
      Code::Unidentified,
      Location::Standard,
      Modifiers::empty(),
      false,
      false,
    );
    view.notify_input_event(InputEvent::Keyboard(event));
  }

  Ok(())
}

enum NamedKey {
  Supported(Key),
  Unknown,
}

fn named_key(name: &str) -> Result<NamedKey, Never> {
  let named = match name {
    "BackSpace" => "Backspace",
    "Return" | "KP_Enter" => "Enter",
    "Left" => "ArrowLeft",
    "Right" => "ArrowRight",
    "Up" => "ArrowUp",
    "Down" => "ArrowDown",
    "Page_Up" => "PageUp",
    "Page_Down" => "PageDown",
    other => other,
  };

  Ok(match named.parse() {
    Ok(key) => NamedKey::Supported(key),
    Err(_unknown_key_name) => NamedKey::Unknown,
  })
}

#[doc = "The channel uses page coordinates. Servo's wheel API uses content motion."]
fn wheel_delta(Point { x, y }: Point<f64>) -> Result<WheelDelta, Never> {
  Ok(WheelDelta {
    x: -x,
    y: -y,
    z: 0.0,
    mode: WheelMode::DeltaPixel,
  })
}

fn control(
  view: &WebView,
  cursor: &mut WebViewPoint,
  updates: &Rc<Updates>,
  request: Request,
) -> Result<(), Never> {
  match request {
    Request::Open(address) => match address.parse() {
      Ok(url) => view.load(url),
      Err(error) => console_core_logging::error!("invalid Servo address: {error}"),
    },
    Request::Resize(size) => {
      let Ok(viewport) = bounded(size);
      view.resize(From::from(viewport));
    }
    Request::Click(Point { x, y }) => {
      let Ok(horizontal) = nearest_f32_from_f64(x);
      let Ok(vertical) = nearest_f32_from_f64(y);
      *cursor = WebViewPoint::Device(DevicePoint::new(horizontal, vertical));

      for action in [MouseButtonAction::Down, MouseButtonAction::Up] {
        let event = MouseButtonEvent::new(action, MouseButton::Primary, *cursor);
        view.notify_input_event(InputEvent::MouseButton(event));
      }
    }
    Request::Scroll(point) => {
      let Ok(delta) = wheel_delta(point);
      view.notify_input_event(InputEvent::Wheel(WheelEvent::new(delta, *cursor)));
    }
    Request::Text(text) => {
      let Ok(()) = input_key(view, Key::Character(text));
    }
    Request::Key(name) => {
      let Ok(key) = named_key(&name);

      match key {
        NamedKey::Supported(supported_key) => {
          let Ok(()) = input_key(view, supported_key);
        }
        NamedKey::Unknown => {}
      }
    }
    Request::Back => {
      let _navigation = view.go_back(1);
    }
    Request::Forward => {
      let _navigation = view.go_forward(1);
    }
    Request::Reload => view.reload(),
    Request::Stop => view.evaluate_javascript("window.stop()", |_| {}),
    Request::Read(id) => {
      let updates = Rc::clone(updates);
      view.evaluate_javascript(READER_SCRIPT, move |result| {
        let text = match result {
          Ok(JSValue::String(text)) => text,
          _ => String::new(),
        };
        let Ok(()) = updates.record(Event::Reader(id, text));
      });
    }
  }

  Ok(())
}

enum PollProgress {
  Continue,
  Disconnected,
}

struct Renderer<'a> {
  socket: BorrowedFd<'a>,
  wake_reader: BorrowedFd<'a>,
  servo: &'a Servo,
  context: &'a SoftwareRenderingContext,
  view: &'a WebView,
  updates: &'a Rc<Updates>,
  cursor: WebViewPoint,
  publication: FramePublication,
}

impl Renderer<'_> {
  fn publish_updates(&mut self) -> io::Result<()> {
    self.servo.spin_event_loop();

    match self.updates.crashed.take() {
      Some(reason) => return Err(io::Error::other(reason)),
      None => {}
    }

    for update in self.updates.events.take() {
      event(self.socket, update, None)?;
    }

    match (&self.publication, self.updates.frame.get()) {
      (FramePublication::Available, FrameSignal::Pending) => {
        self.updates.frame.set(FrameSignal::Idle);
        let publication = send_frame(self.socket, self.view, self.context)?;
        self.publication = publication;
      }
      (FramePublication::Available, FrameSignal::Idle)
      | (FramePublication::AwaitingRelease, FrameSignal::Idle | FrameSignal::Pending) => {}
    }

    Ok(())
  }

  fn receive_request(&mut self) -> io::Result<()> {
    let line = packet(self.socket)?;

    match line.as_str() {
      channel::RELEASED => self.publication = FramePublication::Available,
      message => {
        let Ok(request) = decoded_request(message);

        match request {
          Some(decoded) => {
            let Ok(()) = control(self.view, &mut self.cursor, self.updates, decoded);
          }
          None => {}
        }
      }
    }

    Ok(())
  }

  fn handle_events(&mut self) -> io::Result<PollProgress> {
    self.publish_updates()?;
    let (command_events, wake_events) = {
      let mut sources = [
        PollFd::new(&self.socket, PollFlags::IN),
        PollFd::new(&self.wake_reader, PollFlags::IN),
      ];

      match poll(&mut sources, None) {
        Ok(_ready_sources) => {}
        Err(rustix::io::Errno::INTR) => return Ok(PollProgress::Continue),
        Err(poll_failure) => return Err(io::Error::from(poll_failure)),
      }

      let [commands, signal] = sources;
      (commands.revents(), signal.revents())
    };

    match (
      command_events.intersects(PollFlags::HUP | PollFlags::ERR),
      command_events.contains(PollFlags::IN),
    ) {
      (true, true | false) => return Ok(PollProgress::Disconnected),
      (false, true) => self.receive_request()?,
      (false, false) => {}
    }

    match wake_events.contains(PollFlags::IN).then_some(()) {
      Some(()) => {
        let _notification_bytes = rustix::io::read(self.wake_reader, &mut [0_u8; 1024]);
      }
      None => {}
    }

    Ok(PollProgress::Continue)
  }
}

impl Iterator for Renderer<'_> {
  type Item = io::Result<()>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.handle_events() {
      Ok(PollProgress::Continue) => Some(Ok(())),
      Ok(PollProgress::Disconnected) => None,
      Err(renderer_failure) => Some(Err(renderer_failure)),
    }
  }
}

fn startup(socket: BorrowedFd<'_>) -> io::Result<(Size<u32>, Url)> {
  let initial_viewport = packet(socket)?;
  let Ok(first) = decoded_request(&initial_viewport);

  let size = match first {
    Some(Request::Resize(size)) => size,
    Some(_) | None => return Err(invalid("the first renderer request must set the viewport")),
  };

  let initial_address = packet(socket)?;
  let Ok(second) = decoded_request(&initial_address);

  let address = match second {
    Some(Request::Open(address)) => address,
    Some(_) | None => return Err(invalid("the second renderer request must open the page")),
  };

  let url = address.parse().map_err(io::Error::other)?;
  Ok((size, url))
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Servo receives owned waker clones and shares its rendering context and delegate with the parent event loop"
  )
)]
fn run(socket: BorrowedFd<'_>) -> io::Result<()> {
  let (size, url) = startup(socket)?;
  rustls::crypto::aws_lc_rs::default_provider()
    .install_default()
    .map_err(|_provider_already_installed| io::Error::other("could not initialize TLS"))?;

  let (wake_reader, wake_writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK)?;
  let Ok(viewport) = bounded(size);
  let software_context = SoftwareRenderingContext::new(From::from(viewport)).map_err(
    |error| io::Error::other(format!("cannot initialize Servo rendering: {error:?}")),
  )?;
  let context = Rc::new(software_context);
  let servo = ServoBuilder::default()
    .event_loop_waker(Box::new(Wake(Arc::new(wake_writer))))
    .preferences(Preferences {
      dom_gamepad_enabled: false,
      dom_geolocation_enabled: false,
      dom_bluetooth_enabled: false,
      dom_webrtc_enabled: false,
      dom_touch_events_legacy_apis_enabled: false,
      ..Preferences::default()
    })
    .build();
  let updates = Rc::new(Updates::default());
  let view = WebViewBuilder::new(&servo, context.clone())
    .url(url)
    .delegate(updates.clone())
    .build();
  view.notify_theme_change(Theme::Dark);

  let requests = Renderer {
    socket,
    wake_reader: wake_reader.as_fd(),
    servo: &servo,
    context: &context,
    view: &view,
    updates: &updates,
    cursor: WebViewPoint::Device(DevicePoint::new(0.0, 0.0)),
    publication: FramePublication::Available,
  };

  for result in requests {
    result?;
  }

  Ok(())
}

fn main() -> ExitCode {
  match run(std::io::stdin().as_fd()) {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      console_core_logging::error!("Servo renderer: {error}");
      ExitCode::FAILURE
    }
  }
}
