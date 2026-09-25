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
//! hands back can be [`Transaction::opened`] into a [`Session`], which is the
//! half pam_systemd hears -- the session logind registers, the runtime
//! directory, the seat -- and dropping the session closes it in the order PAM
//! asks: the session, then the credentials, then the transaction.
//!
//! **The secret is not kept.** It lives in the conversation for as long as the
//! transaction might ask for it, it is handed to libpam as a copy libpam
//! frees, and it is written over before its memory goes back. It is never in
//! a `Debug`.

mod pam;

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::fmt;
use std::os::unix::ffi::OsStrExt;
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
            LoginError::ContainsNul(stage) => write!(to, "a word with a NUL in it cannot be handed to PAM while {stage:?}"),
            LoginError::PamFailure { stage, code } => write!(to, "PAM failed while {stage:?}, with {code}"),
        }
    }
}

impl std::error::Error for LoginError {}

struct ConversationState {
    secret: Vec<u8>,
    person: CString,
    messages: RefCell<Vec<String>>,
}

impl Drop for ConversationState {
    fn drop(&mut self) {
        for byte in &mut self.secret {
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

pub struct Session {
    transaction: Transaction,
}

pub fn authenticated(request: Request<'_>, secret: &str) -> Result<Transaction, LoginError> {
    let begun = begun(request, secret);
    let mut transaction = begun?;

    transaction.step(Stage::Authenticating, pam::pam_authenticate, 0)?;
    transaction.step(Stage::CheckingAccount, pam::pam_acct_mgmt, 0)?;

    Ok(transaction)
}

pub fn trusted(request: Request<'_>) -> Result<Transaction, LoginError> {
    let begun = begun(request, "");
    let mut transaction = begun?;

    transaction.step(Stage::CheckingAccount, pam::pam_acct_mgmt, 0)?;

    Ok(transaction)
}

fn begun(request: Request<'_>, secret: &str) -> Result<Transaction, LoginError> {
    let Ok(service) = c_string(request.service, Stage::Starting);
    let service = service?;
    let Ok(person) = c_string(request.person, Stage::Starting);
    let person = person?;
    let Ok(secret) = c_string(secret, Stage::Starting);
    let secret = secret?;
    let rules = match request.rules {
        Rules::System => None,
        Rules::Directory(folder) => match CString::new(folder.as_os_str().as_bytes()) {
            Ok(folder) => Some(folder),
            Err(_) => return Err(LoginError::ContainsNul(Stage::Starting)),
        },
    };
    let state = Box::new(ConversationState {
        secret: secret.into_bytes_with_nul(),
        person,
        messages: RefCell::new(Vec::new()),
    });
    let conversation = pam::Conversation {
        answer: Some(answered),
        data: std::ptr::from_ref::<ConversationState>(&state).cast_mut().cast::<c_void>(),
    };
    let mut handle: *mut pam::Handle = std::ptr::null_mut();
    let rules_at = match &rules {
        Some(rules) => rules.as_ptr(),
        None => std::ptr::null(),
    };

    // SAFETY: every string is a live CString; libpam copies the conversation
    // struct, and the state its data points at is boxed and moves into the
    // transaction, which outlives every call that can reach the callback.
    let started = unsafe {
        pam::pam_start_confdir(service.as_ptr(), state.person.as_ptr(), &conversation, rules_at, &mut handle)
    };

    match (started, handle.is_null()) {
        (pam::SUCCESS, false) => {}
        (code, _) => {
            let Ok(error) = login_error(Stage::Starting, code);

            return Err(error);
        },
    }

    Ok(Transaction { handle, state, last: pam::SUCCESS })
}

type Call = unsafe extern "C" fn(*mut pam::Handle, c_int) -> c_int;

impl Transaction {
    fn step(&mut self, stage: Stage, call: Call, flags: c_int) -> Result<(), LoginError> {
        // SAFETY: the handle is the one `pam_start_confdir` gave back, and it
        // is ended only by `Drop`, which cannot have run while `self` lives.
        let code = unsafe { call(self.handle, flags) };

        self.last = code;

        match code {
            pam::SUCCESS => Ok(()),
            code => {
                let Ok(error) = login_error(stage, code);

                Err(error)
            }
        }
    }

    pub fn messages(&self) -> Result<Vec<String>, Never> {
        Ok(self.state.messages.borrow().clone())
    }

    pub fn opened(mut self, terminal: &str, environment: &[(&str, &str)]) -> Result<Session, LoginError> {
        let Ok(terminal) = c_string(terminal, Stage::SettingEnvironment);
        let terminal = terminal?;

        // SAFETY: a live handle and a CString libpam copies before returning.
        let code = unsafe { pam::pam_set_item(self.handle, pam::TTY, terminal.as_ptr().cast::<c_void>()) };

        match code {
            pam::SUCCESS => {}
            code => {
                let Ok(error) = login_error(Stage::SettingEnvironment, code);

                return Err(error);
            }
        }

        for (name, value) in environment {
            let Ok(pair) = c_string(&format!("{name}={value}"), Stage::SettingEnvironment);
            let pair = pair?;

            // SAFETY: a live handle and a CString libpam copies before returning.
            let code = unsafe { pam::pam_putenv(self.handle, pair.as_ptr()) };

            match code {
                pam::SUCCESS => {}
                code => {
                    let Ok(error) = login_error(Stage::SettingEnvironment, code);

                    return Err(error);
                }
            }
        }

        self.step(Stage::Credentials, pam::pam_setcred, pam::ESTABLISH_CRED)?;
        self.step(Stage::OpeningSession, pam::pam_open_session, 0)?;

        Ok(Session { transaction: self })
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        // SAFETY: the handle `pam_start_confdir` gave back, ended exactly once.
        let _ = unsafe { pam::pam_end(self.handle, self.last) };
    }
}

impl Session {
    pub fn environment(&self) -> Result<Vec<String>, Never> {
        // SAFETY: a live handle. What comes back is a NULL-ended array of
        // strings libpam allocated for the caller, each freed here and then
        // the array, which is what `pam_getenvlist(3)` asks.
        let list = unsafe { pam::pam_getenvlist(self.transaction.handle) };
        let mut every = Vec::new();

        match list.is_null() {
            true => return Ok(every),
            false => {}
        }

        let mut standing = list;

        loop {
            // SAFETY: `standing` never passes the NULL that ends the array.
            let entry = unsafe { *standing };

            match entry.is_null() {
                true => break,
                false => {}
            }

            // SAFETY: a NUL-ended string libpam made, read and then freed once.
            let pair = unsafe { CStr::from_ptr(entry) }.to_string_lossy().into_owned();

            // SAFETY: as above.
            unsafe { pam::free(entry.cast::<c_void>()) };

            every.push(pair);

            // SAFETY: the entry just read was not the NULL, so the one after it is still in the array.
            standing = unsafe { standing.add(1) };
        }

        // SAFETY: the array itself, freed once after every entry in it.
        unsafe { pam::free(list.cast::<c_void>()) };

        Ok(every)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let handle = self.transaction.handle;

        // SAFETY: the handle of the transaction this session holds, which is
        // ended after this, when the field drops.
        let _ = unsafe { pam::pam_close_session(handle, 0) };
        // SAFETY: as above.
        let _ = unsafe { pam::pam_setcred(handle, pam::DELETE_CRED) };
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
        code => LoginError::PamFailure { stage, code },
    })
}

unsafe extern "C" fn answered(
    count: c_int,
    messages: *mut *const pam::Message,
    responses: *mut *mut pam::Response,
    data: *mut c_void,
) -> c_int {
    let Ok(count) = fitted::<c_int, u32>(count);
    let Ok(many) = index(count);

    // SAFETY: `data` is the boxed `ConversationState` `authenticated` handed to libpam,
    // alive for as long as the transaction that is calling this.
    let conversation = unsafe { &*data.cast::<ConversationState>() };
    // SAFETY: calloc, so every answer starts NULL and a partial fill can be
    // freed by walking the whole block.
    let block = unsafe { pam::calloc(many, size_of::<pam::Response>()) }.cast::<pam::Response>();

    match block.is_null() {
        true => return pam::BUF_ERR,
        false => {}
    }

    for at in 0..many {
        // SAFETY: libpam hands `count` message pointers, and `at` is below it.
        let message = unsafe { &**messages.add(at) };
        let answer = match message.style {
            pam::PROMPT_ECHO_OFF => {
                // SAFETY: the secret is NUL-ended; strdup gives a copy libpam frees.
                unsafe { pam::strdup(conversation.secret.as_ptr().cast::<c_char>()) }
            }
            pam::PROMPT_ECHO_ON => {
                // SAFETY: as above, with the person's name.
                unsafe { pam::strdup(conversation.person.as_ptr()) }
            }
            pam::ERROR_MSG | pam::TEXT_INFO => {
                let Ok(()) = recorded(conversation, message.text);

                std::ptr::null_mut()
            }
            _ => {
                let Ok(()) = forgotten(block, count);

                return pam::CONV_ERR;
            }
        };

        // SAFETY: `at` is inside the block calloc made for `count` answers.
        unsafe { (*block.add(at)).answer = answer };
    }

    // SAFETY: libpam request for the block here and frees it and every answer.
    unsafe { *responses = block };

    pam::SUCCESS
}

fn recorded(conversation: &ConversationState, text: *const c_char) -> Result<(), Never> {
    match text.is_null() {
        true => {}
        false => {
            // SAFETY: a NUL-ended string libpam owns for the length of the call.
            let line = unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();

            conversation.messages.borrow_mut().push(line);
        }
    }

    Ok(())
}

fn forgotten(block: *mut pam::Response, count: u32) -> Result<(), Never> {
    let Ok(many) = index(count);

    for at in 0..many {
        // SAFETY: inside the block, and each answer is NULL or a strdup.
        unsafe { pam::free((*block.add(at)).answer.cast::<c_void>()) };
    }

    // SAFETY: the block calloc made, freed once.
    unsafe { pam::free(block.cast::<c_void>()) };

    Ok(())
}
