//! FFI for the WPEPlatform headless display and the GLib main loop.

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString, c_char, c_double, c_int, c_uint, c_ulong, c_void};
use std::io;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::ptr::{self, NonNull};

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};
use console_core_web_engine::{Event, READER_SCRIPT, Request, encoded_event};
use console_input_key_symbols::Keysym;

use crate::{Session, invalid, message};

type Handle = *mut c_void;
type Signal = unsafe extern "C" fn();

const INPUT: c_uint = 1;
const SOCKET_CLOSED: c_uint = 8 | 16 | 32;
const POINTER_DOWN: c_int = 1;
const POINTER_UP: c_int = 2;
const KEY_DOWN: c_int = 7;
const KEY_UP: c_int = 8;
const MOUSE: c_int = 0;
const KEYBOARD: c_int = 2;
const PRIMARY: c_uint = 1;
const PRIMARY_HELD: c_uint = 1 << 8;
const SETTINGS_SOURCE_APPLICATION: c_int = 1;

#[repr(C)]
struct GError {
  domain: u32,
  code: c_int,
  message: *const c_char,
}

const _: () = {
  #[cfg(target_pointer_width = "64")]
  {
    assert!(core::mem::size_of::<GError>() == 16);
    assert!(core::mem::align_of::<GError>() == 8);
  }

  #[cfg(target_pointer_width = "32")]
  {
    assert!(core::mem::size_of::<GError>() == 12);
    assert!(core::mem::align_of::<GError>() == 4);
  }

  assert!(core::mem::offset_of!(GError, domain) == 0);
  assert!(core::mem::offset_of!(GError, code) == 4);
  assert!(core::mem::offset_of!(GError, message) == 8);
};

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "GLib defines GType, gsize and the GBytes output length with the platform's native width"
  )
)]
unsafe extern "C" {
  fn g_object_new(ty: usize, property: *const c_char, ...) -> Handle;

  fn g_object_unref(object: Handle);

  fn g_signal_connect_data(
    object: Handle,
    name: *const c_char,
    callback: Signal,
    data: Handle,
    destroy: Option<unsafe extern "C" fn(Handle, Handle)>,
    flags: c_uint,
  ) -> c_ulong;

  fn g_signal_handler_disconnect(object: Handle, id: c_ulong);

  fn g_error_free(error: *mut GError);
  fn g_free(memory: Handle);

  fn g_type_check_instance_is_a(instance: Handle, ty: usize) -> c_int;

  fn g_bytes_get_data(bytes: Handle, len: *mut usize) -> *const u8;

  fn g_main_loop_new(context: Handle, running: c_int) -> Handle;

  fn g_main_loop_run(main_loop: Handle);

  fn g_main_loop_quit(main_loop: Handle);

  fn g_main_loop_unref(main_loop: Handle);

  fn g_unix_fd_add(
    fd: c_int,
    flags: c_uint,
    callback: unsafe extern "C" fn(c_int, c_uint, Handle) -> c_int,
    data: Handle,
  ) -> c_uint;

  fn g_source_remove(id: c_uint) -> c_int;

  fn g_get_monotonic_time() -> i64;

  fn wpe_display_headless_new() -> Handle;

  fn wpe_display_connect(display: Handle, error: *mut *mut GError) -> c_int;
  fn wpe_display_get_settings(display: Handle) -> Handle;
  fn wpe_settings_set_boolean(
    settings: Handle,
    key: *const c_char,
    value: c_int,
    source: c_int,
    error: *mut *mut GError,
  ) -> c_int;

  fn wpe_display_create_toplevel(display: Handle, max_views: c_uint) -> Handle;

  fn wpe_view_get_toplevel(view: Handle) -> Handle;

  fn wpe_view_set_toplevel(view: Handle, toplevel: Handle);

  fn wpe_toplevel_resized(toplevel: Handle, width: c_int, height: c_int);

  fn wpe_view_resized(view: Handle, width: c_int, height: c_int);

  fn wpe_view_set_visible(view: Handle, visible: c_int);

  fn wpe_view_map(view: Handle);

  fn wpe_view_focus_in(view: Handle);

  fn wpe_view_event(view: Handle, event: Handle);

  fn wpe_buffer_get_width(buffer: Handle) -> c_int;

  fn wpe_buffer_get_height(buffer: Handle) -> c_int;

  fn wpe_buffer_shm_get_type() -> usize;

  fn wpe_buffer_shm_get_data(buffer: Handle) -> Handle;

  fn wpe_buffer_shm_get_stride(buffer: Handle) -> c_uint;

  fn wpe_buffer_import_to_pixels(buffer: Handle, error: *mut *mut GError) -> Handle;

  fn wpe_event_pointer_button_new(
    kind: c_int,
    view: Handle,
    source: c_int,
    time: u32,
    modifiers: c_uint,
    button: c_uint,
    x: c_double,
    y: c_double,
    count: c_uint,
  ) -> Handle;

  fn wpe_event_scroll_new(
    view: Handle,
    source: c_int,
    time: u32,
    modifiers: c_uint,
    dx: c_double,
    dy: c_double,
    precise: c_int,
    stop: c_int,
    x: c_double,
    y: c_double,
  ) -> Handle;

  fn wpe_event_keyboard_new(
    kind: c_int,
    view: Handle,
    source: c_int,
    time: u32,
    modifiers: c_uint,
    code: c_uint,
    key: c_uint,
  ) -> Handle;

  fn wpe_event_unref(event: Handle);

  fn webkit_web_view_get_type() -> usize;

  fn webkit_network_session_new_ephemeral() -> Handle;
  fn webkit_network_session_set_itp_enabled(session: Handle, enabled: c_int);
  fn webkit_web_view_get_settings(page: Handle) -> Handle;
  fn webkit_settings_set_enable_media_stream(settings: Handle, enabled: c_int);
  fn webkit_settings_set_enable_webrtc(settings: Handle, enabled: c_int);
  fn webkit_permission_request_deny(request: Handle);
  fn webkit_web_view_evaluate_javascript(
    page: Handle,
    script: *const c_char,
    length: isize,
    world: *const c_char,
    source_uri: *const c_char,
    cancellable: Handle,
    callback: unsafe extern "C" fn(Handle, Handle, Handle),
    data: Handle,
  );
  fn webkit_web_view_evaluate_javascript_finish(
    page: Handle,
    result: Handle,
    error: *mut *mut GError,
  ) -> Handle;
  fn jsc_value_to_string(value: Handle) -> *mut c_char;

  fn webkit_web_view_get_wpe_view(page: Handle) -> Handle;

  fn webkit_web_view_get_title(page: Handle) -> *const c_char;

  fn webkit_web_view_get_uri(page: Handle) -> *const c_char;

  fn webkit_web_view_is_loading(page: Handle) -> c_int;

  fn webkit_web_view_load_uri(page: Handle, address: *const c_char);

  fn webkit_web_view_go_back(page: Handle);

  fn webkit_web_view_go_forward(page: Handle);

  fn webkit_web_view_reload(page: Handle);

  fn webkit_web_view_stop_loading(page: Handle);
}

struct Object(NonNull<c_void>);

impl Object {
  fn new(raw: Handle) -> io::Result<Self> {
    NonNull::new(raw)
      .map(Self)
      .ok_or_else(|| io::Error::other("could not create WPE object"))
  }
}

impl Drop for Object {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the GObject destructor consumes the unique reference held by this owner"
    )
  )]
  fn drop(&mut self) {
    // SAFETY: Object is created only from constructors that transfer one GObject reference.
    unsafe { g_object_unref(self.0.as_ptr()) };
  }
}

struct MainLoop(NonNull<c_void>);

impl Drop for MainLoop {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the GLib destructor releases the main loop reference held by this owner"
    )
  )]
  fn drop(&mut self) {
    // SAFETY: MainLoop owns the reference returned by g_main_loop_new.
    unsafe { g_main_loop_unref(self.0.as_ptr()) };
  }
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "this is the only boundary that turns WebKit’s borrowed C strings into Rust-owned text"
  )
)]
unsafe fn string(value: *const c_char) -> io::Result<Option<String>> {
  let text = match NonNull::new(value.cast_mut()) {
    Some(text) => text,
    None => return Ok(None),
  };
  // SAFETY: the caller guarantees this pointer refers to a live NUL-terminated string.
  let bytes = unsafe { CStr::from_ptr(text.as_ptr()) };
  let utf8 = bytes.to_str().map_err(io::Error::other)?;
  Ok(Some(utf8.to_owned()))
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the GLib error boundary reads the transferred GError then frees it with the matching GLib function"
  )
)]
unsafe fn error(raw: *mut GError, fallback: &str) -> Result<io::Error, Never> {
  let error = match NonNull::new(raw) {
    Some(error) => error,
    None => return Ok(io::Error::other(fallback.to_owned())),
  };
  // SAFETY: the caller transfers a live GError whose message remains valid until release.
  let message = unsafe { string(error.as_ref().message) };

  // SAFETY: this is the GError reference transferred by the caller.
  unsafe { g_error_free(error.as_ptr()) };

  Ok(match message {
    Ok(Some(text)) => io::Error::other(text),
    Ok(None) => io::Error::other(fallback.to_owned()),
    Err(invalid_message) => io::Error::other(format!("{fallback}: {invalid_message}")),
  })
}

struct Active<'a, 'fd> {
  session: &'a Session<'fd>,
  main_loop: Handle,
  page: Handle,
  view: Handle,
  toplevel: Handle,
  cursor: Cell<Point<f64>>,
  source: Cell<c_uint>,
  failure: RefCell<Option<io::Error>>,
}

impl Active<'_, '_> {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "request failure terminates the GLib event loop that owns the callback scope"
    )
  )]
  fn report(&self, result: io::Result<()>) -> Result<(), Never> {
    let problem = match result {
      Ok(()) => return Ok(()),
      Err(problem) => problem,
    };

    match self.failure.try_borrow_mut() {
      Ok(mut failure) => {
        let _first_problem = failure.get_or_insert(problem);
      }
      Err(reentered) => {
        console_core_logging::error!("Reentrant WPE error ({reentered}): {problem}")
      }
    }

    // SAFETY: Active borrows the live main loop until its callbacks are disconnected.
    unsafe { g_main_loop_quit(self.main_loop) };

    Ok(())
  }

  fn send(&self, event: Event) -> Result<(), Never> {
    self.report(self.session.send(event))
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the URI is fetched from WebKit’s live page and copied before the callback returns"
    )
  )]
  fn address(&self) -> io::Result<()> {
    // SAFETY: the page is live and WebKit keeps the URI alive during this copy.
    let address = unsafe { string(webkit_web_view_get_uri(self.page)) }?;

    match address {
      Some(uri) => self.session.send(Event::Address(uri)),
      None => Ok(()),
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the WPE view and toplevel are borrowed from their live owners during a viewport update"
    )
  )]
  fn resize(&self, size: Size<u32>) -> io::Result<()> {
    let valid_dimensions = bool::then_some(
      size.width > 0
        && size.height > 0
        && size.width <= crate::MAX_SIDE
        && size.height <= crate::MAX_SIDE,
      (),
    );

    match valid_dimensions {
      Some(()) => {}
      None => return Err(invalid("invalid WPE viewport size")),
    }

    let width = c_int::try_from(size.width).map_err(io::Error::other)?;
    let height = c_int::try_from(size.height).map_err(io::Error::other)?;

    // SAFETY: Both objects remain live in run, and the dimensions are validated positive C integers.
    unsafe {
      wpe_toplevel_resized(self.toplevel, width, height);
      wpe_view_resized(self.view, width, height);
    }

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "WPE creates input events for the borrowed view and retains them only during dispatch"
    )
  )]
  fn key(&self, key: Keysym, time: u32) -> Result<(), Never> {
    for kind in [KEY_DOWN, KEY_UP] {
      // SAFETY: The live view owns the event target, and dispatch releases the new event.
      let Ok(()) = unsafe {
        dispatch(
          self.view,
          wpe_event_keyboard_new(kind, self.view, KEYBOARD, time, 0, 0, key.0),
        )
      };
    }

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "WebKit navigation and WPE input are synchronous C operations on the live callback objects"
    )
  )]
  fn request(&self, request: Request) -> io::Result<()> {
    // SAFETY: GLib's monotonic clock needs no arguments or thread-local setup.
    let milliseconds = unsafe { g_get_monotonic_time() }.div_euclid(1000);
    let Ok(time) = wrapped::<_, u32>(milliseconds);

    match request {
      Request::Open(uri) => {
        let uri_2 = CString::new(uri).map_err(io::Error::other)?;

        // SAFETY: The page is live, and the CString provides a valid URI for the duration of the call.
        unsafe { webkit_web_view_load_uri(self.page, uri_2.as_ptr()) };
      }
      Request::Resize(size) => {
        self.resize(size)?;
      }
      // SAFETY: Active borrows the page from the live WebKit object in run.
      Request::Back => unsafe { webkit_web_view_go_back(self.page) },
      // SAFETY: Active borrows the page from the live WebKit object in run.
      Request::Forward => unsafe { webkit_web_view_go_forward(self.page) },
      // SAFETY: Active borrows the page from the live WebKit object in run.
      Request::Reload => unsafe { webkit_web_view_reload(self.page) },
      // SAFETY: Active borrows the page from the live WebKit object in run.
      Request::Stop => unsafe { webkit_web_view_stop_loading(self.page) },
      Request::Read(id) => self.read(id)?,
      Request::Click(point) => {
        self.cursor.set(point);

        for (kind, modifiers, count) in [(POINTER_DOWN, PRIMARY_HELD, 1), (POINTER_UP, 0, 0)] {
          // SAFETY: The view is live, and the dispatcher consumes the event returned by WPE.
          let Ok(()) = unsafe {
            dispatch(
              self.view,
              wpe_event_pointer_button_new(
                kind, self.view, MOUSE, time, modifiers, PRIMARY, point.x, point.y, count,
              ),
            )
          };
        }
      }
      Request::Scroll(delta) => {
        let point = self.cursor.get();

        // SAFETY: The view is live, and the dispatcher consumes the event returned by WPE.
        let Ok(()) = unsafe {
          dispatch(
            self.view,
            wpe_event_scroll_new(
              self.view,
              MOUSE,
              time,
              0,
              -delta.x / 2.5,
              -delta.y / 2.5,
              1,
              0,
              point.x,
              point.y,
            ),
          )
        };
      }
      Request::Text(text) => {
        for character in text.chars() {
          let Ok(key) = Keysym::from_char(character);
          let Ok(()) = self.key(key, time);
        }
      }
      Request::Key(name) => {
        let Ok(parsed) = Keysym::from_str(&name);

        let key = match parsed {
          Some(matched_key) => matched_key,
          ::core::option::Option::None => return Ok(()),
        };

        let Ok(()) = self.key(key, time);
      }
    }

    Ok(())
  }

  fn read(&self, id: u64) -> io::Result<()> {
    let script = CString::new(READER_SCRIPT).map_err(io::Error::other)?;
    let reply = Box::new((self.session.socket.try_clone_to_owned()?, id));
    // SAFETY: WebKit copies the script and calls reader_finished once with the
    // boxed reply. The reply owns its socket and holds no reference to Active.
    unsafe {
      webkit_web_view_evaluate_javascript(
        self.page,
        script.as_ptr(),
        -1,
        c"console-reader".as_ptr(),
        ptr::null(),
        ptr::null_mut(),
        reader_finished,
        Box::into_raw(reply).cast(),
      );
    }
    Ok(())
  }
}

unsafe extern "C" fn reader_finished(page: Handle, result: Handle, data: Handle) {
  // SAFETY: the matching call transfers a Box of this type to this callback.
  let reply = unsafe { Box::from_raw(data.cast::<(OwnedFd, u64)>()) };
  // SAFETY: GLib supplies this page and result. The last argument is optional.
  let value =
    unsafe { webkit_web_view_evaluate_javascript_finish(page, result, ptr::null_mut()) };
  let text = match Object::new(value) {
    Ok(value) => {
      // SAFETY: the result owns the JSCValue. GLib transfers the string to us.
      let raw = unsafe { jsc_value_to_string(value.0.as_ptr()) };
      // SAFETY: string copies the NUL-terminated value before it is freed.
      let text = unsafe { string(raw) };
      unsafe { g_free(raw.cast()) };
      text.ok().flatten().unwrap_or_default()
    }
    Err(_) => String::new(),
  };
  let Ok(line) = encoded_event(&Event::Reader(reply.1, text));
  match console_browser_renderer::send(reply.0.as_fd(), &line, None) {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::debug!("WebKit reader response could not be sent: {fault}")
    }
  }
}

unsafe extern "C" fn deny_permission(_: Handle, request: Handle, _: Handle) -> c_int {
  // SAFETY: WebKit supplies a live WebKitPermissionRequest for this signal.
  unsafe { webkit_permission_request_deny(request) };
  1
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C event transfer is consumed by dispatch and released with the WPE event API"
  )
)]
unsafe fn dispatch(view: Handle, raw_event: Handle) -> Result<(), Never> {
  let event = match NonNull::new(raw_event) {
    Some(event) => event,
    None => return Ok(()),
  };

  // SAFETY: the caller supplies a live view and transfers the new event reference.
  unsafe {
    wpe_view_event(view, event.as_ptr());
    wpe_event_unref(event.as_ptr());
  }

  Ok(())
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "only the installed C callbacks recover the borrowed Active pointer before run detaches it"
  )
)]
unsafe fn active<'a>(data: Handle) -> Result<&'a Active<'a, 'a>, Never> {
  // SAFETY: run registered this exact state pointer and detaches all callbacks before it drops.
  let state = unsafe { &*data.cast() };
  Ok(state)
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the GLib Unix source calls this ABI callback using the state installed by run"
  )
)]
unsafe extern "C" fn socket_ready(_: c_int, condition: c_uint, data: Handle) -> c_int {
  // SAFETY: The GLib Unix source was registered with the live Active pointer in run.
  let Ok(state) = unsafe { active(data) };
  let result = match condition & SOCKET_CLOSED {
    0 => message(state.session).and_then(|request| match request {
      Some(request_2) => state.request(request_2),
      None => Ok(()),
    }),
    _ => {
      state.source.set(0);

      // SAFETY: The callback runs before run releases the main loop reference.
      unsafe { g_main_loop_quit(state.main_loop) };

      return 0;
    }
  };
  let Ok(()) = state.report(result);
  1
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "WebKit calls this ABI callback with the live state registered by run"
  )
)]
unsafe extern "C" fn load_changed(_: Handle, change: c_int, data: Handle) {
  // SAFETY: run registered this load callback with the live Active pointer.
  let Ok(state) = unsafe { active(data) };

  let result = match change {
    0 => state.session.send(Event::Started),
    2 => state.address(),
    3 => {
      let Ok(()) = state.report(state.address());
      state.session.send(Event::Finished)
    }
    _ => Ok(()),
  };
  let Ok(()) = state.report(result);
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "WebKit property notification passes its live page and registered callback state"
  )
)]
unsafe extern "C" fn address_changed(page: Handle, _: Handle, data: Handle) {
  // SAFETY: WebKit notifies a property on the live page passed as the signal instance.
  let loading = unsafe { webkit_web_view_is_loading(page) };

  match loading {
    0 => {
      // SAFETY: the property signal was attached to the live Active pointer in run.
      let Ok(state) = unsafe { active(data) };
      let Ok(()) = state.report(state.address());
    }
    _ => {}
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "WebKit property notification provides a valid title and callback state for this invocation"
  )
)]
unsafe extern "C" fn title_changed(page: Handle, _: Handle, data: Handle) {
  // SAFETY: WebKit owns the page and title during this synchronous signal and copy.
  let title_result = unsafe { string(webkit_web_view_get_title(page)) };
  // SAFETY: the title signal was attached to the live Active pointer in run.
  let Ok(state) = unsafe { active(data) };

  match title_result {
    Ok(Some(title)) => {
      let Ok(()) = state.send(Event::Title(title));
    }
    Ok(None) => {
      let Ok(()) = state.send(Event::Title(String::new()));
    }
    Err(problem) => {
      let Ok(()) = state.report(Err(problem));
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "WebKit process termination passes the callback state registered by run"
  )
)]
unsafe extern "C" fn terminated(_: Handle, reason: c_int, data: Handle) {
  // SAFETY: run attached the termination signal to this live Active pointer.
  let Ok(state) = unsafe { active(data) };
  let Ok(()) = state.report(Err(io::Error::other(format!(
    "WPE content process terminated ({reason})"
  ))));
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "WPE owns the buffer and its borrowed GBytes until the render signal returns"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "GLib returns a readable pointer and its byte length and only the validated frame prefix is read while the WPE buffer is alive"
  )
)]
unsafe fn frame(session: &Session<'_>, buffer: NonNull<c_void>) -> io::Result<()> {
  let raw = buffer.as_ptr();
  let [buffer_width, buffer_height] = {
    // SAFETY: the callback holds the WPEBuffer alive while reading these fields.
    unsafe { [wpe_buffer_get_width(raw), wpe_buffer_get_height(raw)] }
  };
  let width = u32::try_from(buffer_width).map_err(io::Error::other)?;
  let height = u32::try_from(buffer_height).map_err(io::Error::other)?;
  let size = Size { width, height };
  let dimensions = bool::then_some(
    width > 0 && height > 0 && width <= crate::MAX_SIDE && height <= crate::MAX_SIDE,
    (),
  );

  match dimensions {
    Some(()) => {}
    None => return Err(invalid("invalid WPE buffer dimensions")),
  }

  // SAFETY: the live WPEBuffer is a GObject and the type function returns its SHM subtype.
  let is_shm = unsafe { g_type_check_instance_is_a(raw, wpe_buffer_shm_get_type()) };
  let shared_memory = bool::then_some(is_shm != 0, ());
  let mut failure = ptr::null_mut();
  let raw_bytes = match shared_memory {
    // SAFETY: the subtype check established WPEBufferSHM and the buffer keeps its data alive.
    Some(()) => unsafe { wpe_buffer_shm_get_data(raw) },
    // SAFETY: the buffer is live, the error output is writable, and the buffer owns the result.
    None => unsafe { wpe_buffer_import_to_pixels(raw, &mut failure) },
  };
  let bytes = match NonNull::new(raw_bytes) {
    Some(pixels) => pixels,
    None => {
      // SAFETY: failure was initialized to null and only the WPE import writes a GLib GError there.
      let Ok(problem) = unsafe { error(failure, "could not read WPE buffer") };
      return Err(problem);
    }
  };
  let Ok(mut native_length) = index(0u32);
  // SAFETY: bytes is a live GBytes borrowed from raw and native_length is writable.
  let data = unsafe { g_bytes_get_data(bytes.as_ptr(), &mut native_length) };
  let Ok(length) = fixed_index(native_length);
  let stride = match shared_memory {
    // SAFETY: raw still refers to the validated WPEBufferSHM.
    Some(()) => u64::from(unsafe { wpe_buffer_shm_get_stride(raw) }),
    None => length
      .checked_div(u64::from(height))
      .ok_or_else(|| invalid("empty WPE buffer"))?,
  };
  let row_bytes = u64::from(width).wrapping_mul(4);
  let extent = u64::from(height.wrapping_sub(1))
    .checked_mul(stride)
    .and_then(|last| last.checked_add(row_bytes))
    .ok_or_else(|| invalid("WPE buffer extent overflows"))?;
  let complete = bool::then_some(stride >= row_bytes && extent <= length, ());

  match complete {
    Some(()) => {}
    None => return Err(invalid("WPE buffer has incomplete pixel rows")),
  }

  let signed_length =
    console_core_number_conversion::checked_offset(extent).map_err(io::Error::other)?;
  let Ok(slice_length) = index(signed_length.unsigned_abs());
  let pixels =
    NonNull::new(data.cast_mut()).ok_or_else(|| invalid("WPE buffer has no pixels"))?;
  // SAFETY: GBytes reports length readable bytes while raw is alive. The checked
  // prefix fits within that storage and within Rust's maximum slice size.
  let pixel_data = unsafe { std::slice::from_raw_parts(pixels.as_ptr(), slice_length) };
  session.frame(size, stride, pixel_data)
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the WPE render callback borrows the live frame and registered callback state for this invocation"
  )
)]
unsafe extern "C" fn rendered(_: Handle, buffer: Handle, data: Handle) {
  // SAFETY: The render signal was registered with the live Active pointer in run.
  let Ok(state) = unsafe { active(data) };
  let result = match NonNull::new(buffer) {
    // SAFETY: WPE retains the signal's buffer until this callback returns.
    Some(frame_buffer) => unsafe { frame(state.session, frame_buffer) },
    None => Err(invalid("WPE render signal has no buffer")),
  };
  let Ok(()) = state.report(result);
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "GLib's erased signal API invokes the callback with the selected signal's C signature"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit059_no_unchecked_api",
  expect(
    explicit059_no_unchecked_api,
    reason = "GLib erases a C function pointer and recovers the signature from the signal registration"
  )
)]
unsafe fn connect(
  object: Handle,
  name: &CStr,
  callback: *const (),
  data: Handle,
) -> Result<c_ulong, Never> {
  // SAFETY: the caller supplies a non-null C callback for this signal.
  let callback_function = unsafe { std::mem::transmute::<*const (), Signal>(callback) };
  let handler = {
    // SAFETY: the object and callback data outlive registration, and the name is terminated.
    unsafe { g_signal_connect_data(object, name.as_ptr(), callback_function, data, None, 0) }
  };
  Ok(handler)
}

struct OpenedView {
  _fallback: Option<Object>,
  page: Object,
  _network: Object,
  _display: Object,
  view: NonNull<c_void>,
  toplevel: NonNull<c_void>,
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "WebKit and WPE construct foreign objects; OpenedView keeps their owning references with the borrowed view and toplevel"
  )
)]
fn open_view() -> io::Result<OpenedView> {
  // SAFETY: the constructor transfers one GObject reference.
  let display = Object::new(unsafe { wpe_display_headless_new() })?;
  let mut failure = ptr::null_mut();
  // SAFETY: display holds a WPE display and failure is writable for the GError output.
  let connected = unsafe { wpe_display_connect(display.0.as_ptr(), &mut failure) };

  match connected {
    0 => {
      // SAFETY: the failed call transfers any GError written to failure.
      let Ok(problem) = unsafe { error(failure, "could not connect the WPE headless display") };
      return Err(problem);
    }
    _ => {}
  }

  // SAFETY: WPE owns the settings through the live display, and failure is writable.
  let dark = unsafe {
    wpe_settings_set_boolean(
      wpe_display_get_settings(display.0.as_ptr()),
      c"/wpe-platform/dark-mode".as_ptr(),
      1,
      SETTINGS_SOURCE_APPLICATION,
      &mut failure,
    )
  };
  match dark {
    0 => {
      // SAFETY: the failed call transfers any GError written to failure.
      let Ok(problem) = unsafe { error(failure, "could not set WPE dark mode") };
      return Err(problem);
    }
    _ => {}
  }

  // SAFETY: the constructor transfers one GObject reference.
  let network = Object::new(unsafe { webkit_network_session_new_ephemeral() })?;
  // SAFETY: network owns a live WebKitNetworkSession.
  unsafe { webkit_network_session_set_itp_enabled(network.0.as_ptr(), 1) };
  // SAFETY: the properties name a display and network session, both are live,
  // and the varargs end at a null property name.
  let page = Object::new(unsafe {
    g_object_new(
      webkit_web_view_get_type(),
      c"display".as_ptr(),
      display.0.as_ptr(),
      c"network-session".as_ptr(),
      network.0.as_ptr(),
      ptr::null::<c_char>(),
    )
  })?;
  // SAFETY: the page owns its settings.
  unsafe {
    let settings = webkit_web_view_get_settings(page.0.as_ptr());
    webkit_settings_set_enable_media_stream(settings, 0);
    webkit_settings_set_enable_webrtc(settings, 0);
  }
  // SAFETY: page is the live WebKitWebView created with these two properties.
  let raw_view = unsafe { webkit_web_view_get_wpe_view(page.0.as_ptr()) };
  let view =
    NonNull::new(raw_view).ok_or_else(|| io::Error::other("could not create WPE view"))?;
  // SAFETY: the live page owns the view for this call.
  let attached_toplevel = unsafe { wpe_view_get_toplevel(view.as_ptr()) };
  let (toplevel, fallback) = match NonNull::new(attached_toplevel) {
    Some(toplevel) => (toplevel, None),
    None => {
      // SAFETY: the display is live and the constructor transfers a GObject reference.
      let created = Object::new(unsafe { wpe_display_create_toplevel(display.0.as_ptr(), 1) })?;

      // SAFETY: view and the new toplevel are both alive throughout the call.
      unsafe { wpe_view_set_toplevel(view.as_ptr(), created.0.as_ptr()) };

      (created.0, Some(created))
    }
  };

  Ok(OpenedView {
    _fallback: fallback,
    page,
    _network: network,
    _display: display,
    view,
    toplevel,
  })
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "each typed C callback is installed through the documented signature of its WPE or WebKit signal"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit011_no_as_cast",
  expect(
    explicit011_no_as_cast,
    reason = "these C function pointers are erased for GLib signal registration, with their signatures restored by the signal API"
  )
)]
unsafe fn connect_handlers(
  state: &Active<'_, '_>,
  data: Handle,
) -> Result<[(Handle, c_ulong); 6], Never> {
  let page = state.page;
  let view = state.view;
  // SAFETY: each named signal is paired with its exact C callback signature;
  // the caller will detach them before the state and objects are released.
  let handlers = unsafe {
    let Ok(render) = connect(view, c"buffer-rendered", rendered as *const (), data);
    let Ok(load) = connect(page, c"load-changed", load_changed as *const (), data);
    let Ok(address) = connect(page, c"notify::uri", address_changed as *const (), data);
    let Ok(title) = connect(page, c"notify::title", title_changed as *const (), data);
    let Ok(permission) = connect(
      page,
      c"permission-request",
      deny_permission as *const (),
      data,
    );
    let Ok(terminated) = connect(
      page,
      c"web-process-terminated",
      terminated as *const (),
      data,
    );
    [
      (view, render),
      (page, load),
      (page, address),
      (page, title),
      (page, terminated),
      (page, permission),
    ]
  };
  Ok(handlers)
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the GLib loop is the only caller of the registered source and signals; cleanup ends their borrows of the stack state"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit090_no_raw_descriptor",
  expect(
    explicit090_no_raw_descriptor,
    reason = "GLib takes a C descriptor number; Session's BorrowedFd stays live until the source is removed here"
  )
)]
fn drive_view(state: &Active<'_, '_>, address: &CStr) -> Result<(), Never> {
  let data = ptr::from_ref(state).cast_mut().cast();
  // SAFETY: all objects and the state stay alive through the cleanup below.
  let Ok(handlers) = unsafe { connect_handlers(state, data) };
  // SAFETY: the socket and stack state outlive this source, which is removed below.
  let source = unsafe {
    g_unix_fd_add(
      state.session.socket.as_raw_fd(),
      INPUT | SOCKET_CLOSED,
      socket_ready,
      data,
    )
  };
  state.source.set(source);

  // SAFETY: the live view and page remain owned by OpenedView during the loop
  // and address is NUL-terminated.
  unsafe {
    wpe_view_set_visible(state.view, 1);
    wpe_view_map(state.view);
    wpe_view_focus_in(state.view);
    webkit_web_view_load_uri(state.page, address.as_ptr());
  }

  let ready = state
    .failure
    .try_borrow()
    .is_ok_and(|failure| failure.is_none());

  match bool::then_some(ready, ()) {
    // SAFETY: MainLoop holds its reference until this entire call returns.
    Some(()) => unsafe { g_main_loop_run(state.main_loop) },
    None => {}
  }

  match state.source.get() {
    0 => {}
    source_id => {
      // SAFETY: this is the source registered above; socket_ready clears it before self-removal.
      let _removed = unsafe { g_source_remove(source_id) };
    }
  }

  for (object, handler) in handlers {
    match handler {
      0 => {}
      handler_id => {
        // SAFETY: each identifier is paired with its registering object, both still live.
        unsafe { g_signal_handler_disconnect(object, handler_id) };
      }
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "GLib allocates the main loop and passes one owned reference to MainLoop"
  )
)]
pub(super) fn run(session: &Session<'_>, size: Size<u32>, address: &CStr) -> io::Result<()> {
  let opened = open_view()?;
  // SAFETY: a null context selects the default context and this constructor transfers a reference.
  let raw_loop = unsafe { g_main_loop_new(ptr::null_mut(), 0) };
  let handle =
    NonNull::new(raw_loop).ok_or_else(|| io::Error::other("could not create GLib main loop"))?;
  let main_loop = MainLoop(handle);
  let state = Active {
    session,
    main_loop: main_loop.0.as_ptr(),
    page: opened.page.0.as_ptr(),
    view: opened.view.as_ptr(),
    toplevel: opened.toplevel.as_ptr(),
    cursor: Cell::new(Point { x: 0.0, y: 0.0 }),
    source: Cell::new(0),
    failure: RefCell::new(None),
  };
  state.resize(size)?;
  let Ok(()) = drive_view(&state, address);

  match state.failure.into_inner() {
    Some(problem) => Err(problem),
    None => Ok(()),
  }
}
