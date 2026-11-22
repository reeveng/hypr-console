use std::path::{Path, PathBuf};

fn guessed(named: &str) -> PathBuf {
    //~v WARN EXPLICIT085_NO_SHARED_TEMPORARY
    std::env::temp_dir().join(format!("console-{named}-{}", std::process::id()))
}

fn fallen_back(runtime: Option<PathBuf>) -> PathBuf {
    match runtime {
        Some(runtime) => runtime,
        //~v WARN EXPLICIT085_NO_SHARED_TEMPORARY
        None => Path::new("/dev/shm").join("voice"),
    }
}

fn spelled() -> &'static str {
    //~v WARN EXPLICIT085_NO_SHARED_TEMPORARY
    "/dev/shm/console-page"
}

fn private(runtime: &Path) -> PathBuf {
    runtime.join("voice")
}

fn only_looks_like_it() -> &'static str {
    "/tmpfs-is-another-word"
}

fn main() {
    let _ = guessed("picture");
    let _ = fallen_back(None);
    let _ = spelled();
    let _ = private(Path::new("/run/user/1000"));
    let _ = only_looks_like_it();
}
