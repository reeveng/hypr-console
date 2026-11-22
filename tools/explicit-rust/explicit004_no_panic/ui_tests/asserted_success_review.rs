// edition:2024
// compile-flags: --test

#[derive(Debug)]
struct FaultWithoutEquality;
fn fallible() -> Result<u8, FaultWithoutEquality> { Ok(1) }
fn value() -> Option<u8> { Some(1) }

struct Custom;
impl Custom { fn unwrap(self) -> u8 { 1 } }

#[test]
fn unrelated_or_effectful() {
    assert_eq!(Custom.unwrap(), 1);
    assert_eq!(value().unwrap(), value().unwrap());
    assert_eq!(value().unwrap(), { println!("expected"); 1 });
    assert_eq!(value().unwrap(), 1, "{}", { println!("message"); 2 });
    let test = || { assert_eq!(value().unwrap(), 1); };
    test();
}

#[test]
#[should_panic(expected = "unwrap")]
fn tests_the_unwrap_message() {
    assert_eq!(Option::<u8>::None.unwrap(), 1);
}

#[test]
fn custom_assertion() {
    macro_rules! assert_eq { ($a:expr, $b:expr) => {{ let _ = ($a, $b); }} }
    assert_eq!(value().unwrap(), 1);
}

#[test]
fn called_directly() {
    assert_eq!(value().unwrap(), 1);
}
#[test]
fn calls_another_test() { called_directly(); }

fn outside_the_harness() {
    assert_eq!(value().unwrap(), 1);
}


#[test]
fn keep_early_exit_and_error_type() {
    assert_eq!(fallible().unwrap(), 1);
    return;
}

struct Reader;
impl Reader {
    fn read(&self) -> Result<u8, FaultWithoutEquality> { fallible() }
}

#[test]
fn keep_temporaries_and_prior_effects() -> Result<(), FaultWithoutEquality> {
    let reader = std::cell::RefCell::new(Reader);
    assert_eq!(reader.borrow().read().unwrap(), 1);
    assert_eq!({ println!("before"); 1 }, fallible().unwrap());
    Ok(())
}

#[test]
fn other_configuration_uses_a_different_binding() {
    #[cfg(feature = "custom")]
    let read = Custom;
    #[cfg(not(feature = "custom"))]
    let read = value();
    assert_eq!(read.unwrap(), 1);
}
