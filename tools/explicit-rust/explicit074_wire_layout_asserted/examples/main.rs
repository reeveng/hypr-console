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

fn main() {
    let _ = asked;
}
