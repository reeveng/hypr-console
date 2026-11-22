struct Secret(Vec<u8>);

impl Secret {
    fn same_as(&self, other: &Secret) -> u8 {
        let differing = self.0.iter().zip(other.0.iter()).fold(0u8, |so_far, (one, other)| so_far | (one ^ other));

        differing
    }
}

enum Asks {
    ForAPassword,
    ForNothing,
}

struct Network {
    named: String,
    //~v WARN EXPLICIT094_NO_PLAIN_SECRET
    password: String,
    //~v WARN EXPLICIT094_NO_PLAIN_SECRET
    wifi_passphrase: Option<String>,
    wants_password: Asks,
}

struct Pairing {
    //~v WARN EXPLICIT094_NO_PLAIN_SECRET
    passkey: u32,
    kept: Secret,
}

//~v WARN EXPLICIT094_NO_PLAIN_SECRET
fn joined(named: &str, password: &str) -> usize {
    named.len().saturating_add(password.len())
}

//~v WARN EXPLICIT094_NO_PLAIN_SECRET
fn typed(pin: &[u8]) -> usize {
    pin.len()
}

//~v WARN EXPLICIT094_NO_PLAIN_SECRET
fn hurried(password: &str, kept: &str) -> u8 {
    //~v WARN EXPLICIT094_NO_PLAIN_SECRET
    match password == kept {
        true => 1,
        false => 0,
    }
}

fn checked(typed: &Secret, kept: &Secret) -> u8 {
    typed.same_as(kept)
}

fn pinned(spinner: u8) -> u8 {
    spinner
}

fn main() {
    let network = Network {
        named: "home".to_string(),
        password: "word".to_string(),
        wifi_passphrase: None,
        wants_password: Asks::ForAPassword,
    };
    let pairing = Pairing { passkey: 1, kept: Secret(vec![1]) };

    let _ = joined(&network.named, &network.password);
    let _ = network.wifi_passphrase;
    let _ = match network.wants_password {
        Asks::ForAPassword => 1,
        Asks::ForNothing => 0,
    };
    let _ = typed(b"1234");
    let _ = checked(&pairing.kept, &Secret(vec![1]));
    let _ = pairing.passkey;
    let _ = pinned(1);
    let _ = hurried("word", "word");
    let _ = Asks::ForNothing;
}
