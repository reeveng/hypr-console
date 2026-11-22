#[allow(dead_code)]
#[derive(serde::Serialize)]
struct Written {
    held: u32,
}

#[allow(dead_code)]
#[derive(serde::Serialize)]
#[repr(packed)]
struct Trick {
    held: u32,
}

#[allow(dead_code)]
struct Stayed {
    held: u32,
}

#[allow(dead_code)]
struct Asked {
    held: u32,
}

#[allow(dead_code)]
#[repr(C)]
struct Pinned {
    held: u32,
}

#[allow(dead_code)]
struct Answered {
    held: u32,
}

#[allow(dead_code)]
struct Called {
    held: u32,
}

#[allow(dead_code)]
#[repr(transparent)]
struct Wrapped(u32);

#[allow(improper_ctypes)]
unsafe extern "C" {
    fn asked(into: *mut Asked, pinned: *const Pinned, wrapped: Wrapped) -> i32;
}

#[allow(dead_code)]
extern "C" fn answered(from: &Answered) -> u32 {
    from.held
}

#[allow(dead_code)]
type Callback = unsafe extern "C" fn(*mut Called);

const EXPECTED_WORD_BYTES: usize = 4;
const _: () = {
    assert!(core::mem::size_of::<Pinned>() == EXPECTED_WORD_BYTES);
    assert!(4 == core::mem::align_of::<Pinned>());
    assert!(core::mem::offset_of!(Pinned, held) == 0);
    assert!(core::mem::size_of::<Wrapped>() == 4);
    assert!(core::mem::align_of::<Wrapped>() == 4);
    assert!(core::mem::offset_of!(Wrapped, 0) == 0);
};

#[repr(C)]
struct PartlyChecked {
    word: u32,
    tail: u32,
}

const _: () = {
    assert!(core::mem::size_of::<PartlyChecked>() == 8);
    assert!(core::mem::offset_of!(PartlyChecked, word) == 0);
    if false {
        assert!(core::mem::align_of::<PartlyChecked>() == 4);
    }
};

#[allow(dead_code)]
extern "C" fn partially_checked(_: PartlyChecked) {}

fn main() {
    let _ = asked;
}
