//~v WARN EXPLICIT104_NO_SECRET_IN_SOURCE
const DEFAULT_PASSWORD: &str = "console";

//~v WARN EXPLICIT104_NO_SECRET_IN_SOURCE
static WIFI_PSK: &[u8] = b"at-home";

const PASSWORD_PROMPT: &str = "";
const WHERE_THE_PASSWORD_IS: u8 = 1;

struct Network {
    named: String,
    password: String,
}

fn typed_out() -> Network {
    Network {
        named: "home".to_string(),
        //~v WARN EXPLICIT104_NO_SECRET_IN_SOURCE
        password: "console".to_string(),
    }
}

fn bound() -> usize {
    //~v WARN EXPLICIT104_NO_SECRET_IN_SOURCE
    let passphrase = String::from("correct horse");

    passphrase.len()
}

fn handed_in(named: &str, read_at_the_edge: String) -> Network {
    Network { named: named.to_string(), password: read_at_the_edge }
}

fn main() {
    let _ = DEFAULT_PASSWORD;
    let _ = WIFI_PSK;
    let _ = PASSWORD_PROMPT;
    let _ = WHERE_THE_PASSWORD_IS;
    let _ = typed_out().named;
    let _ = bound();
    let _ = handed_in("home", String::new()).password;
}
