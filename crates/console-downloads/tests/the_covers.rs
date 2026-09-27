use std::error::Error;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread::JoinHandle;

use console_downloads::covers::{self, Fetched};

const PICTURE: &[u8] = b"\x89PNG\r\n\x1a\n a picture the server chose";

type Serving = JoinHandle<Result<(), std::io::Error>>;

fn answering(status: &'static str) -> Result<(String, Serving), std::io::Error> {
    let listening = TcpListener::bind("127.0.0.1:0")?;
    let local = listening.local_addr()?;
    let port = local.port();

    let serving = std::thread::spawn(move || {
        let (mut asked, _from) = listening.accept()?;
        let mut request = [0_u8; 2048];
        let _read = asked.read(&mut request)?;

        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            PICTURE.len()
        );

        asked.write_all(head.as_bytes())?;
        asked.write_all(PICTURE)?;

        Ok(())
    });

    Ok((format!("http://127.0.0.1:{port}/cover.png"), serving))
}

fn into(named: &str) -> Result<PathBuf, console_core_temporary_directories::Unmade> {
    let folder = console_core_temporary_directories::fresh(named)?;

    Ok(folder.join("cover.part"))
}

fn finished(serving: Serving) -> Result<(), Box<dyn Error>> {
    let served = serving.join().map_err(|_panicked| "the server gave up before it answered")?;

    served?;

    Ok(())
}

#[test]
fn a_refusal_that_comes_with_a_picture_leaves_no_picture() -> Result<(), Box<dyn Error>> {
    let (from, serving) = answering("404 Not Found")?;
    let into = into("downloads-cover-refused")?;

    let fetched = covers::fetched(&from, &into);

    finished(serving)?;

    assert_eq!(fetched, Ok(Fetched::Failed));
    assert!(!into.exists(), "the placeholder a 404 carried was kept as the cover");

    Ok(())
}

#[test]
fn a_picture_the_server_sends_is_kept() -> Result<(), Box<dyn Error>> {
    let (from, serving) = answering("200 OK")?;
    let into = into("downloads-cover-arrived")?;

    let fetched = covers::fetched(&from, &into);

    finished(serving)?;

    let kept = std::fs::read(&into)?;

    assert_eq!(fetched, Ok(Fetched::Arrived));
    assert_eq!(kept, PICTURE);

    Ok(())
}
