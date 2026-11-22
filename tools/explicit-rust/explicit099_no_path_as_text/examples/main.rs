use std::path::Path;

fn under_a_home(at: &str) -> u8 {
    //~v WARN EXPLICIT099_NO_PATH_AS_TEXT
    match at.starts_with("/home/someone") {
        true => 1,
        false => 0,
    }
}

fn the_rest(at: &String) -> Option<&str> {
    //~v WARN EXPLICIT099_NO_PATH_AS_TEXT
    at.strip_prefix("/usr/local")
}

fn from_the_right_place(address: &str) -> u8 {
    //~v WARN EXPLICIT099_NO_PATH_AS_TEXT
    match address.starts_with("https://example.com") {
        true => 1,
        false => 0,
    }
}

fn by_components(at: &Path) -> u8 {
    match at.starts_with("/home/someone") {
        true => 1,
        false => 0,
    }
}

fn its_own_boundary(at: &str) -> u8 {
    match at.starts_with("/home/someone/") {
        true => 1,
        false => 0,
    }
}

fn only_a_scheme(address: &str) -> u8 {
    match address.starts_with("https://") {
        true => 1,
        false => 0,
    }
}

fn absolute(at: &str) -> Option<&str> {
    at.strip_prefix("/")
}

fn a_word(line: &str) -> u8 {
    match line.starts_with("Exec") {
        true => 1,
        false => 0,
    }
}

fn main() {
    let _ = under_a_home("/home/someone-else");
    let _ = the_rest(&"/usr/localish".to_string());
    let _ = from_the_right_place("https://example.com.elsewhere.net");
    let _ = by_components(Path::new("/home/someone-else"));
    let _ = its_own_boundary("/home/someone/file");
    let _ = only_a_scheme("https://example.com");
    let _ = absolute("/etc");
    let _ = a_word("Exec=one");
}
