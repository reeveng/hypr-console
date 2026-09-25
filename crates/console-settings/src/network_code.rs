//! The network the machine is on, as a QR code a phone joins by pointing its
//! camera at it.
//!
//! The password is NetworkManager's, read from the saved connection with
//! `--show-secrets` each time the code is asked for, and never through
//! `before`: what that keeps between draws is kept on disk. It is read with
//! escaping off, because a password is whatever someone typed into a router
//! and a colon in it is not a field. A network with a lock and no password
//! saved says so rather than drawing a code that joins nothing.
//!
//! The picture is a pixmap in the runtime directory, which is the person's own
//! and gone at the next boot, and the folder is removed when B puts the code
//! away. Each square is drawn many pixels wide before the picture is scaled,
//! because a square one pixel wide comes out of a smooth scale as a blur of
//! grey at every edge, which a camera reads worse. The file is named by what
//! it says, since a picture is remembered by its path: two networks never
//! share a name, and a changed password is a new picture rather than the old
//! one read back.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use console_core_atomic_writes as writes;
use console_core_external_programs::Program;
use console_core_geometry::Size;
use console_core_never::Never;
use console_panel::page::{Aside, Picture, Row, Showing};

use crate::rows::Shares;
use crate::wifi;

pub type Sharing = Arc<Mutex<Option<wifi::Network>>>;

const SCALE: u32 = 12;

const FOLDER: &str = "wifi code";

const PASSWORD: &str = "802-11-wireless-security.psk";

#[derive(Debug)]
pub enum Unshared {
    Placeless,
    Query(std::io::Error),
    Unsaved(String),
    TooLong(String),
    Making(PathBuf, std::io::Error),
    Unwritten(writes::Unwritten),
}

impl std::fmt::Display for Unshared {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unshared::Placeless => write!(to, "There is nowhere to draw the code."),
            Unshared::Query(fault) => write!(to, "NetworkManager did not answer: {fault}"),
            Unshared::Unsaved(name) => write!(to, "The password for {name} is not saved on this device."),
            Unshared::TooLong(name) => write!(to, "The name and password of {name} are too long for a code."),
            Unshared::Making(at, fault) => write!(to, "{}: {fault}", at.display()),
            Unshared::Unwritten(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for Unshared {}

pub fn shared(sharing: &Sharing) -> Result<Option<wifi::Network>, Never> {
    Ok(match sharing.lock() {
        Ok(network) => network.clone(),
        Err(_the_lock_is_poisoned) => None,
    })
}

pub fn offered(sharing: &Sharing, network: wifi::Network) -> Result<Shares, Never> {
    let held = Arc::clone(sharing);

    Ok(Arc::new(move |showing: &dyn Showing| {
        match held.lock() {
            Ok(mut shown) => *shown = Some(network.clone()),
            Err(_the_lock_is_poisoned) => {},
        }

        showing.refresh();
    }))
}

pub fn put_away(sharing: &Sharing) -> Result<Option<wifi::Network>, Never> {
    let was = match sharing.lock() {
        Ok(mut shown) => shown.take(),
        Err(_the_lock_is_poisoned) => None,
    };
    let Ok(folder) = folder();

    match folder {
        Some(folder) => {
            let _ = std::fs::remove_dir_all(folder);
        },
        None => {},
    }

    Ok(was)
}

pub fn rows(network: &wifi::Network) -> Result<Vec<Row>, Never> {
    let row = match written(network) {
        Ok(at) => Row::showing(Picture::Showing(Some(at))),
        Err(fault) => Row::said(&fault.to_string(), Aside("")),
    };
    let Ok(row) = row;

    Ok(vec![row])
}

fn folder() -> Result<Option<PathBuf>, Never> {
    let runtime = console_core_places::runtime_ours()?;

    Ok(runtime.map(|at| at.join(FOLDER)))
}

fn password(network: &wifi::Network) -> Result<String, Unshared> {
    let Ok(mut asking) = Program::Nmcli.command();

    let said = asking
        .args(["--show-secrets", "--escape", "no", "--get-values", PASSWORD, "connection", "show", "id"])
        .arg(&network.name)
        .output()
        .map_err(Unshared::Query)?;

    let Ok(password) = wifi::password(&String::from_utf8_lossy(&said.stdout));

    match (network.locked, password) {
        (true, Some(password)) => Ok(password),
        (true, None) => Err(Unshared::Unsaved(network.name.clone())),
        (false, _) => Ok(String::new()),
    }
}

fn named(said: &str) -> Result<String, Never> {
    let mut hashing = DefaultHasher::new();

    said.hash(&mut hashing);

    Ok(format!("{:016x}.ppm", hashing.finish()))
}

fn written(network: &wifi::Network) -> Result<PathBuf, Unshared> {
    let password = password(network)?;

    let Ok(said) = console_core_qr_codes::wifi(console_core_qr_codes::Network { name: &network.name, password: &password });
    let Ok(code) = console_core_qr_codes::encoded(said.as_bytes());

    let code = match code {
        Some(code) => code,
        None => return Err(Unshared::TooLong(network.name.clone())),
    };

    let Ok(drawn) = code.drawn(SCALE);
    let Ok(bytes) = console_pictures::portable_pixmap(Size { width: drawn.side, height: drawn.side }, &drawn.rgb);
    let Ok(folder) = folder();

    let folder = match folder {
        Some(folder) => folder,
        None => return Err(Unshared::Placeless),
    };

    std::fs::create_dir_all(&folder).map_err(|fault| Unshared::Making(folder.clone(), fault))?;

    let Ok(name) = named(&said);
    let at = folder.join(name);

    writes::whole(&at, &bytes).map_err(Unshared::Unwritten)?;

    Ok(at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_named_by_what_it_says_so_a_new_password_is_a_new_picture() {
        assert_eq!(named("WIFI:T:WPA;S:Home;P:one;;"), named("WIFI:T:WPA;S:Home;P:one;;"));
        assert_ne!(named("WIFI:T:WPA;S:Home;P:one;;"), named("WIFI:T:WPA;S:Home;P:two;;"));
    }
}
