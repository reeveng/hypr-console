struct Client {
    checks: u8,
    trusts: Vec<u8>,
}

impl Client {
    fn new() -> Self {
        Self { checks: 1, trusts: Vec::new() }
    }

    fn danger_accept_invalid_certs(mut self, _accept: u8) -> Self {
        self.checks = 0;

        self
    }

    fn danger_accept_invalid_hostnames(mut self, _accept: u8) -> Self {
        self.checks = 0;

        self
    }

    fn add_root_certificate(mut self, of: &[u8]) -> Self {
        self.trusts.extend_from_slice(of);

        self
    }
}

fn to_anybody() -> Client {
    //~v ERROR EXPLICIT100_NO_SKIPPED_VERIFICATION
    Client::new().danger_accept_invalid_certs(1)
}

fn to_any_name() -> Client {
    //~v ERROR EXPLICIT100_NO_SKIPPED_VERIFICATION
    Client::new().danger_accept_invalid_hostnames(1)
}

fn to_the_test_server(its_certificate: &[u8]) -> Client {
    Client::new().add_root_certificate(its_certificate)
}

fn main() {
    let _ = to_anybody().checks;
    let _ = to_any_name().trusts;
    let _ = to_the_test_server(b"certificate");
}
