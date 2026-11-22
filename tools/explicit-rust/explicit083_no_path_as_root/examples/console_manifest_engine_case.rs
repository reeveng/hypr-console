use std::fs::File;
use std::io::Read;
use std::path::Path;

fn read(at: &Path) -> std::io::Result<String> {
    //~v WARN EXPLICIT083_NO_PATH_AS_ROOT
    std::fs::read_to_string(at)
}

fn opened(at: &Path) -> std::io::Result<File> {
    //~v WARN EXPLICIT083_NO_PATH_AS_ROOT
    File::open(at)
}

fn given_away(at: &Path) -> std::io::Result<()> {
    //~v WARN EXPLICIT083_NO_PATH_AS_ROOT
    std::os::unix::fs::chown(at, Some(1000), Some(1000))
}

fn asked(at: &Path) -> std::io::Result<u64> {
    //~v WARN EXPLICIT083_NO_PATH_AS_ROOT
    let held = at.metadata()?;

    Ok(held.len())
}

fn removed(at: &Path) -> std::io::Result<()> {
    //~v WARN EXPLICIT083_NO_PATH_AS_ROOT
    std::fs::remove_dir_all(at)
}

fn of_what_was_opened(mut held: File) -> std::io::Result<u64> {
    let mut said = String::new();
    let _ = held.read_to_string(&mut said)?;
    let about = held.metadata()?;

    Ok(about.len())
}

fn main() {
    let at = Path::new("somewhere");
    let _ = read(at);
    let _ = given_away(at);
    let _ = asked(at);
    let _ = removed(at);

    match opened(at) {
        Ok(held) => {
            let _ = of_what_was_opened(held);
        }
        Err(_) => {}
    }
}
