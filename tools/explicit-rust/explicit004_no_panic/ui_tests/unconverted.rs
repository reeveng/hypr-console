// edition:2024
// compile-flags: --test

#[derive(Debug)]
struct Fault;

fn setup() -> Result<(), Fault> { Err(Fault) }

#[test]
#[should_panic]
fn checks_a_panic() {
    setup().unwrap();
}

#[test]
fn explicit_return() {
    setup().unwrap();
    return;
}

#[test]
fn closure() {
    let action = || { setup().unwrap(); };
    action();
}

#[test]
fn used_as_setup() {
    setup().unwrap();
}

#[test]
fn uses_another_test() {
    let check: fn() = used_as_setup;
    check();
}

#[test]
fn conditional_early_return() {
    #[rustfmt::skip]
    let _fixture = [7, 3];
    setup().unwrap();
    #[cfg(feature = "fast-exit")]
    return;
}
