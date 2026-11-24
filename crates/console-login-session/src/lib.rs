//! A person's login session, from the secret they give to the session closing.
//!
//! **PAM is request, not guessed.** PAM is a conversation: the modules a
//! service's rules name ask for what they want, in their own words and as
//! often as they like, and the caller answers. A login that assumed the
//! conversation -- one prompt, one password -- is a device somebody cannot get
//! into the first time a module asks something else. So the answers are given
//! by what was request: a hidden prompt is the secret, a shown one is the
//! person's name, and anything a module says rather than asks is kept for the
//! screen, because "your account has expired" is the one sentence a person
//! locked out needs to read.
//!
//! **One transaction, two halves.** [`authenticated`] is the half that can be
//! refused: the secret, then whether the account may log in now. [`trusted`]
//! is the same without the secret, for the two logins nobody types -- the
//! greeter's own session, and the autologin a machine boots into. What it
//! hands back can be [`Transaction::open_session`] into a [`Session`], which is the
//! half pam_systemd hears -- the session logind registers, the runtime
//! directory, the seat -- and dropping the session closes it in the order PAM
//! asks: the session, then the credentials, then the transaction.
//!
//! **The greeter's session has no credentials.** Its rules deny `auth`
//! outright, and `pam_setcred` walks the `auth` stack, so asking to establish
//! credentials there is `PAM_CRED_ERR` every time. The first boot that ran the
//! login window stopped there six times over and fell to a text console. So
//! the caller says which kind of session it is opening, and one with no
//! credentials never asks for them or deletes them.
//!
//! **The secret is not kept.** It lives in the conversation for as long as the
//! transaction might ask for it, it is handed to libpam as a copy libpam
//! frees, and it is written over before its memory goes back. It is never in
//! a `Debug`.

mod pam;

use std::cell::Cell;
use std::ffi::{CStr, CString, OsString, c_char, c_int, c_void};
use std::fmt;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rules<'a> {
  System,
  Directory(&'a Path),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request<'a> {
  pub service: &'a str,
  pub person: &'a str,
  pub rules: Rules<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
  Starting,
  Authenticating,
  CheckingAccount,
  SettingEnvironment,
  Credentials,
  OpeningSession,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginError {
  WrongSecret,
  UnknownPerson,
  Expired,
  NotAllowed,
  ContainsNul(Stage),
  PamFailure { stage: Stage, code: i32 },
}

impl fmt::Display for LoginError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      LoginError::WrongSecret => write!(to, "that is not the pattern"),
      LoginError::UnknownPerson => write!(to, "nobody by that name is on this machine"),
      LoginError::Expired => write!(to, "the account or its password has expired"),
      LoginError::NotAllowed => write!(to, "that account may not log in here"),
      LoginError::ContainsNul(stage) => write!(
        to,
        "a word with a NUL in it cannot be handed to PAM while {stage:?}"
      ),
      LoginError::PamFailure { stage, code } => {
        write!(to, "PAM failed while {stage:?}, with {code}")
      }
    }
  }
}

impl std::error::Error for LoginError {}

struct ConversationState {
  secret: PamSecret,
  person: CString,
  messages: Cell<Vec<String>>,
}

struct PamSecret(Vec<u8>);

impl Drop for PamSecret {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "volatile stores clear the owned secret bytes before release and require a live writable pointer"
    )
  )]
  fn drop(&mut self) {
    for byte in &mut self.0 {
      // SAFETY: a byte this struct owns, written through a pointer taken
      // from a live `&mut`; volatile so the write is not optimised away.
      unsafe { std::ptr::write_volatile(byte, 0) };
    }
  }
}

pub struct Transaction {
  handle: *mut pam::Handle,
  state: Box<ConversationState>,
  last: c_int,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Credentials {
  Establish,
  Without,
}

pub struct Session {
  transaction: Transaction,
  credentials: Credentials,
}

#[cfg_attr(
  dylint_lib = "explicit094_no_plain_secret",
  expect(
    explicit094_no_plain_secret,
    reason = "the public entry point borrows caller-owned text and moves its only owned copy into PamSecret"
  )
)]
pub fn authenticate(request: Request<'_>, secret: &str) -> Result<Transaction, LoginError> {
  match secret.as_bytes().contains(&0).then_some(()) {
    Some(()) => return Err(LoginError::ContainsNul(Stage::Starting)),
    None => {}
  }

  let Ok(checked) = c_string(secret, Stage::Starting);
  let checked_2 = checked?;
  let transaction = begun(request, PamSecret(checked_2.into_bytes_with_nul()))?;

  authenticated(transaction)
}

pub fn authenticate_without_secret(request: Request<'_>) -> Result<Transaction, LoginError> {
  let transaction = begun(request, PamSecret(Vec::new()))?;

  authenticated(transaction)
}

fn authenticated(mut transaction: Transaction) -> Result<Transaction, LoginError> {
  transaction.step(Stage::Authenticating, pam::pam_authenticate, 0)?;
  transaction.step(Stage::CheckingAccount, pam::pam_acct_mgmt, 0)?;

  Ok(transaction)
}

pub fn trusted(request: Request<'_>) -> Result<Transaction, LoginError> {
  let begun = begun(request, PamSecret(vec![0]));
  let mut transaction = begun?;

  transaction.step(Stage::CheckingAccount, pam::pam_acct_mgmt, 0)?;

  Ok(transaction)
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the PAM transaction copies the conversation descriptor; its boxed state and the passed C strings remain live for their required lifetimes"
  )
)]
fn begun(request: Request<'_>, secret: PamSecret) -> Result<Transaction, LoginError> {
  let Ok(service) = c_string(request.service, Stage::Starting);
  let service_2 = service?;
  let Ok(person) = c_string(request.person, Stage::Starting);
  let person_2 = person?;
  let rules = match request.rules {
    Rules::System => None,
    Rules::Directory(folder) => match CString::new(folder.as_os_str().as_bytes()) {
      Ok(folder_2) => Some(folder_2),
      Err(_) => return Err(LoginError::ContainsNul(Stage::Starting)),
    },
  };
  let state = Box::new(ConversationState {
    secret,
    person: person_2,
    messages: Cell::new(Vec::new()),
  });
  let conversation = pam::Conversation {
    answer: Some(conversation),
    data: std::ptr::from_ref::<ConversationState>(&state)
      .cast_mut()
      .cast::<c_void>(),
  };
  let mut handle: *mut pam::Handle = std::ptr::null_mut();
  let rules_at = match &rules {
    Some(rules_2) => rules_2.as_ptr(),
    None => std::ptr::null(),
  };

  // SAFETY: every string is a live CString; libpam copies the conversation
  // struct, and the state its data points at is boxed and moves into the
  // transaction, which outlives every call that can reach the callback.
  let started = unsafe {
    pam::pam_start_confdir(
      service_2.as_ptr(),
      state.person.as_ptr(),
      &conversation,
      rules_at,
      &mut handle,
    )
  };

  match (started, handle.is_null()) {
    (pam::SUCCESS, false) => {}
    (code, _) => {
      let Ok(error) = login_error(Stage::Starting, code);

      return Err(error);
    }
  }

  Ok(Transaction {
    handle,
    state,
    last: pam::SUCCESS,
  })
}

type Call = unsafe extern "C" fn(*mut pam::Handle, c_int) -> c_int;

impl Transaction {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the transaction owns a live PAM handle and only passes it to functions using the same handle ABI"
    )
  )]
  fn step(&mut self, stage: Stage, call: Call, flags: c_int) -> Result<(), LoginError> {
    // SAFETY: the handle is the one `pam_start_confdir` gave back, and it
    // is ended only by `Drop`, which cannot have run while `self` lives.
    let code = unsafe { call(self.handle, flags) };

    self.last = code;

    match code {
      pam::SUCCESS => Ok(()),
      code_2 => {
        let Ok(error) = login_error(stage, code_2);

        Err(error)
      }
    }
  }

  pub fn messages(&self) -> Result<Vec<String>, Never> {
    let messages = self.state.messages.take();
    let snapshot = messages.clone();
    self.state.messages.set(messages);

    Ok(snapshot)
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the live transaction owns the PAM handle; the item and environment setters copy their passed C strings during the calls"
    )
  )]
  pub fn open_session(
    mut self,
    terminal: &str,
    environment: &[(&str, &str)],
    credentials: Credentials,
  ) -> Result<Session, LoginError> {
    let Ok(terminal_2) = c_string(terminal, Stage::SettingEnvironment);
    let terminal_3 = terminal_2?;
    let item = terminal_3.as_ptr().cast::<c_void>();

    // SAFETY: a live handle and a CString libpam copies before returning.
    let code = unsafe { pam::pam_set_item(self.handle, pam::TTY, item) };

    match code {
      pam::SUCCESS => {}
      code_2 => {
        let Ok(error) = login_error(Stage::SettingEnvironment, code_2);

        return Err(error);
      }
    }

    for (name, value) in environment {
      let Ok(pair) = c_string(&format!("{name}={value}"), Stage::SettingEnvironment);
      let pair_2 = pair?;

      // SAFETY: a live handle and a CString libpam copies before returning.
      let code_3 = unsafe { pam::pam_putenv(self.handle, pair_2.as_ptr()) };

      match code_3 {
        pam::SUCCESS => {}
        code_4 => {
          let Ok(error) = login_error(Stage::SettingEnvironment, code_4);

          return Err(error);
        }
      }
    }

    match credentials {
      Credentials::Establish => {
        self.step(Stage::Credentials, pam::pam_setcred, pam::ESTABLISH_CRED)?
      }
      Credentials::Without => {}
    }

    self.step(Stage::OpeningSession, pam::pam_open_session, 0)?;

    Ok(Session {
      transaction: self,
      credentials,
    })
  }
}

impl Drop for Transaction {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "this destructor ends the uniquely owned PAM handle while the callback state is still alive"
    )
  )]
  fn drop(&mut self) {
    // SAFETY: the handle `pam_start_confdir` gave back, ended exactly once.
    let _ = unsafe { pam::pam_end(self.handle, self.last) };
  }
}

impl Session {
  #[cfg_attr(
    dylint_lib = "explicit055_no_unbounded_loop",
    expect(
      explicit055_no_unbounded_loop,
      reason = "PAM returns an array that ends in a null pointer and requires its caller to release every entry"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "PAM transfers ownership of a null-terminated array of allocated C strings, which are read and freed before the array"
    )
  )]
  pub fn environment(&self) -> Result<Vec<OsString>, Never> {
    // SAFETY: a live handle. What comes back is a NULL-ended array of
    // strings libpam allocated for the caller, each freed here and then
    // the array, which is what `pam_getenvlist(3)` asks.
    let list = unsafe { pam::pam_getenvlist(self.transaction.handle) };

    match ::core::primitive::bool::then_some(list.is_null(), ()) {
      ::core::option::Option::Some(()) => return Ok(Vec::new()),
      ::core::option::Option::None => {}
    }

    let entries = std::iter::successors(Some(list), |cursor| {
      // SAFETY: cursor points at the first slot or at a slot following a non-null entry.
      let entry = unsafe { **cursor };

      match bool::then_some(entry.is_null(), ()) {
        Some(()) => None,
        // SAFETY: a following slot exists because a null pointer terminates the array.
        None => Some(unsafe { cursor.add(1) }),
      }
    })
    .map(|cursor| {
      // SAFETY: this slot was reached by the walk up to and including the null terminator.
      unsafe { *cursor }
    })
    .take_while(|entry| !entry.is_null());

    let every = entries
      .map(|entry| {
        // SAFETY: a NUL-ended string libpam made, read and then freed once.
        let pair = unsafe { CStr::from_ptr(entry) };
        let owned = OsString::from_vec(pair.to_bytes().to_vec());

        // SAFETY: as above.
        unsafe { pam::free(entry.cast::<c_void>()) };

        owned
      })
      .collect();

    // SAFETY: the array itself, freed once after every entry in it.
    unsafe { pam::free(list.cast::<c_void>()) };

    Ok(every)
  }
}

impl Drop for Session {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the session borrows its live transaction handle to close the session before deleting its own credentials"
    )
  )]
  fn drop(&mut self) {
    let handle = self.transaction.handle;

    // SAFETY: the handle of the transaction this session holds, which is
    // ended after this, when the field drops.
    let _ = unsafe { pam::pam_close_session(handle, 0) };

    match self.credentials {
      Credentials::Establish => {
        // SAFETY: as above.
        let _ = unsafe { pam::pam_setcred(handle, pam::DELETE_CRED) };
      }
      Credentials::Without => {}
    }
  }
}

fn c_string(word: &str, stage: Stage) -> Result<Result<CString, LoginError>, Never> {
  Ok(match CString::new(word) {
    Ok(text) => Ok(text),
    Err(_) => Err(LoginError::ContainsNul(stage)),
  })
}

fn login_error(stage: Stage, code: c_int) -> Result<LoginError, Never> {
  Ok(match code {
    pam::AUTH_ERR | pam::MAXTRIES => LoginError::WrongSecret,
    pam::USER_UNKNOWN => LoginError::UnknownPerson,
    pam::ACCT_EXPIRED | pam::NEW_AUTHTOK_REQD => LoginError::Expired,
    pam::PERM_DENIED => LoginError::NotAllowed,
    code_2 => LoginError::PamFailure {
      stage,
      code: code_2,
    },
  })
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "libpam calls this ABI boundary with its message array and a live borrowed conversation; only a fully initialized C-owned response array is returned"
  )
)]
unsafe extern "C" fn conversation(
  count: c_int,
  messages: *mut *const pam::Message,
  responses: *mut *mut pam::Response,
  data: *mut c_void,
) -> c_int {
  match count {
    1..=pam::MAX_NUM_MSG => {}
    _ => return pam::CONV_ERR,
  }

  match (messages.is_null(), responses.is_null(), data.is_null()) {
    (false, false, false) => {}
    _ => return pam::CONV_ERR,
  }

  let Ok(count_2) = fitted::<c_int, u32>(count);
  let Ok(many) = index(count_2);

  // SAFETY: `data` is the boxed `ConversationState` `authenticated` handed to libpam,
  // alive for as long as the transaction that is calling this.
  let conversation = unsafe { &*data.cast::<ConversationState>() };
  // SAFETY: calloc, so every answer starts NULL and a partial fill can be
  // freed by walking the whole block.
  let block = unsafe { pam::calloc(many, size_of::<pam::Response>()) }.cast::<pam::Response>();

  match ::core::primitive::bool::then_some(block.is_null(), ()) {
    ::core::option::Option::Some(()) => return pam::BUF_ERR,
    ::core::option::Option::None => {}
  }

  for at in 0..many {
    // SAFETY: libpam hands `count` message pointers, and `at` is below it.
    let message_at = unsafe { *messages.add(at) };
    // SAFETY: libpam owns each non-null message through the call.
    let message = match unsafe { message_at.as_ref() } {
      Some(message) => message,
      None => {
        let Ok(()) = free_responses(block, count_2);

        return pam::CONV_ERR;
      }
    };
    let answer = match message.style {
      pam::PROMPT_ECHO_OFF => match conversation.secret.0.as_slice() {
        [] => {
          let Ok(()) = free_responses(block, count_2);

          return pam::CONV_ERR;
        }
        secret => {
          // SAFETY: the secret is NUL-ended; strdup gives a copy libpam frees.
          unsafe { pam::strdup(secret.as_ptr().cast::<c_char>()) }
        }
      },
      pam::PROMPT_ECHO_ON => {
        // SAFETY: as above, with the person's name.
        unsafe { pam::strdup(conversation.person.as_ptr()) }
      }
      pam::ERROR_MSG | pam::TEXT_INFO => {
        let Ok(()) = record_message(conversation, message.text);

        std::ptr::null_mut()
      }
      _ => {
        let Ok(()) = free_responses(block, count_2);

        return pam::CONV_ERR;
      }
    };

    match (message.style, answer.is_null()) {
      (pam::PROMPT_ECHO_ON | pam::PROMPT_ECHO_OFF, true) => {
        let Ok(()) = free_responses(block, count_2);

        return pam::BUF_ERR;
      }
      _ => {}
    }

    // SAFETY: `at` is inside the block calloc made for `count` answers.
    unsafe { (*block.add(at)).answer = answer };
  }

  // SAFETY: libpam request for the block here and frees it and every answer.
  unsafe { *responses = block };

  pam::SUCCESS
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the conversation ABI supplies live NUL-terminated text for notices while the callback runs"
  )
)]
fn record_message(conversation: &ConversationState, text: *const c_char) -> Result<(), Never> {
  match ::core::primitive::bool::then_some(text.is_null(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      // SAFETY: a NUL-ended string libpam owns for the length of the call.
      let notice = unsafe { CStr::from_ptr(text) };
      let line = match notice.to_str() {
        Ok(text_2) => text_2.to_owned(),
        Err(_invalid_utf8) => String::from("PAM returned a message that could not be decoded"),
      };
      let mut messages = conversation.messages.take();

      messages.push(line);
      conversation.messages.set(messages);
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the callback still owns the zero-initialized response array and its completed C allocations on the failure path"
  )
)]
fn free_responses(block: *mut pam::Response, count: u32) -> Result<(), Never> {
  let Ok(many) = index(count);

  for at in 0..many {
    // SAFETY: inside the block, and each answer is NULL or a strdup.
    unsafe { pam::free((*block.add(at)).answer.cast::<c_void>()) };
  }

  // SAFETY: the block calloc made, freed once.
  unsafe { pam::free(block.cast::<c_void>()) };

  Ok(())
}
