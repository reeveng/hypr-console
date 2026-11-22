// edition:2024

enum WithDrop {
    Count(u16),
    Other(String),
}

fn new_record() -> WithDrop {
    WithDrop::Other("owned".to_owned())
}

fn temporary() -> u16 {
    let WithDrop::Count(value) = new_record() else {
        panic!("temporary dropped before panic");
    };
    value
}

enum Configured {
    Count(u16),
    Other,
    #[cfg(any())]
    Hidden,
}

fn configured(record: Configured) -> u16 {
    let Configured::Count(value) = record else {
        return 0;
    };
    value
}

enum Nested {
    Count(Option<u16>),
    Other,
}

fn nested(record: Nested) -> u16 {
    let Nested::Count(Some(value)) = record else {
        return 0;
    };
    value
}

macro_rules! custom_failure {
    () => { panic!("custom failure") };
}

fn custom(record: WithDrop) -> u16 {
    let WithDrop::Count(value) = record else {
        custom_failure!();
    };
    value
}

mod shadowed {
    use super::WithDrop;

    fn count() -> u16 {
        #[cfg(any())]
        use super::Configured as WithDrop;

        let WithDrop::Count(value) = WithDrop::Count(2) else {
            return 0;
        };
        value
    }
}

mod dynamic_include {
    include!(concat!("auxiliary/", "let_else_enum_included.rs"));

    fn count(record: Included) -> u16 {
        let Included::Count(value) = record else {
            return 0;
        };
        value
    }
}

fn main() {}
