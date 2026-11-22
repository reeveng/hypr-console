// edition:2024
#![allow(dead_code)]

enum ByteClass {
    Control,
    Data,
}

pub fn exported(byte: u8) -> bool {
    byte == 0
}

extern "C" fn foreign(byte: u8) -> bool {
    byte == 0
}

const fn constant(byte: u8) -> bool {
    byte == 0
}

fn first_class(byte: u8) -> bool {
    byte == 0
}

fn higher_order(bytes: &[u8]) -> usize {
    bytes.iter().copied().filter(|&byte| first_class(byte)).count()
        + usize::from(take_function(first_class))
}

fn take_function(classify: fn(u8) -> bool) -> bool {
    classify(0)
}

fn alternate(byte: u8) -> bool {
    byte == 0
}

#[cfg(any())]
fn inactive_call() {
    let _ = alternate(0);
}

fn returns_early(byte: u8) -> bool {
    return byte == 0;
}

fn conditional_return(byte: u8) -> bool {
    #[cfg(any())]
    return byte == 1;
    byte == 0
}

fn missing_variant(byte: u8) -> bool {
    byte == 0
}

mod gated_variant {
    enum ByteClass {
        Control,
        Data,
        #[cfg(any())]
        Other,
    }

    fn classify(byte: u8) -> bool {
        byte == 0
    }
}

mod macro_call {
    enum ByteClass {
        Control,
        Data,
    }

    fn classify(byte: u8) -> bool {
        byte == 0
    }

    macro_rules! hidden_caller {
        () => { classify(0) }
    }

    fn uses_macro() {
        let _ = hidden_caller!();
    }
}

mod unresolved_macro {
    enum ByteClass {
        Control,
        Data,
    }

    fn classify(byte: u8) -> bool {
        byte == 0
    }

    #[cfg(any())]
    opaque!();
}

mod external_child {
    enum ByteClass {
        Control,
        Data,
    }

    fn classify(byte: u8) -> bool {
        byte == 0
    }

    #[cfg(any())]
    mod unverified;
}

mod trait_owner {
    enum ByteClass {
        Control,
        Data,
    }

    trait Check {
        fn classify(byte: u8) -> bool;
    }
}

mod drop_enum {
    enum ByteClass {
        Control,
        Data,
    }

    impl Drop for ByteClass {
        fn drop(&mut self) {}
    }

    fn classify(byte: u8) -> bool {
        byte == 0
    }
}

fn main() {}
