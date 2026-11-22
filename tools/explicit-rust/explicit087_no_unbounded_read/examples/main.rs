use std::io::Read;
use std::os::unix::net::UnixStream;

const THE_MOST_A_REPLY_IS: u64 = 65_536;

fn all_of_it(mut heard: UnixStream) -> std::io::Result<Vec<u8>> {
    let mut held = Vec::new();

    //~v WARN EXPLICIT087_NO_UNBOUNDED_READ
    let _ = heard.read_to_end(&mut held)?;

    Ok(held)
}

fn all_of_it_as_text(heard: &mut dyn Read) -> std::io::Result<String> {
    let mut said = String::new();

    //~v WARN EXPLICIT087_NO_UNBOUNDED_READ
    let _ = heard.read_to_string(&mut said)?;

    Ok(said)
}

fn the_short_way(heard: UnixStream) -> std::io::Result<String> {
    //~v WARN EXPLICIT087_NO_UNBOUNDED_READ
    std::io::read_to_string(heard)
}

fn so_much_and_no_more(heard: UnixStream) -> std::io::Result<Vec<u8>> {
    let mut held = Vec::new();
    let mut bounded = heard.take(THE_MOST_A_REPLY_IS);
    let _ = bounded.read_to_end(&mut held)?;

    Ok(held)
}

fn a_whole_file() -> std::io::Result<String> {
    std::fs::read_to_string("/sys/class/backlight/one/brightness")
}

fn main() {
    let _ = UnixStream::pair().map(|(one, other)| (all_of_it(one), the_short_way(other)));
    let _ = UnixStream::pair().map(|(one, _other)| so_much_and_no_more(one));
    let _ = all_of_it_as_text(&mut std::io::empty());
    let _ = a_whole_file();
}
