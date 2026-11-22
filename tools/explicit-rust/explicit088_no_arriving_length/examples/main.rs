const THE_MOST_A_MESSAGE_IS: usize = 65_536;

struct Header {
    declared: usize,
}

fn as_declared(header: &Header) -> Vec<u8> {
    //~v WARN EXPLICIT088_NO_ARRIVING_LENGTH
    Vec::with_capacity(header.declared)
}

fn filled(declared: usize) -> Vec<u8> {
    //~v WARN EXPLICIT088_NO_ARRIVING_LENGTH
    vec![0; declared]
}

fn grown(held: &mut Vec<u8>, declared: usize) {
    //~v WARN EXPLICIT088_NO_ARRIVING_LENGTH
    held.reserve(declared);
}

fn through_a_name(header: &Header) -> Vec<u8> {
    let wanted = header.declared;

    //~v WARN EXPLICIT088_NO_ARRIVING_LENGTH
    Vec::with_capacity(wanted)
}

fn under_a_ceiling(header: &Header) -> Vec<u8> {
    Vec::with_capacity(header.declared.min(THE_MOST_A_MESSAGE_IS))
}

fn measured(held: &[u8]) -> Vec<u8> {
    Vec::with_capacity(held.len().saturating_add(1))
}

fn measured_through_a_name(held: &[u8]) -> Vec<u8> {
    let many = held.len();

    vec![0; many]
}

fn written() -> Vec<u8> {
    Vec::with_capacity(THE_MOST_A_MESSAGE_IS)
}

fn listed() -> Vec<u8> {
    vec![0; 16]
}

impl Header {
    fn len(&self) -> usize {
        self.declared
    }
}

#[allow(dead_code)]
fn invalid_bounds(header: &Header, upper: usize) {
    let _ = Vec::<u8>::with_capacity(header.len());
    let _ = Vec::<u8>::with_capacity(header.declared.clamp(0, upper));
    let mut count = 1;
    let _ = count;
    count = header.declared;
    let _ = Vec::<u8>::with_capacity(count);
}

#[allow(dead_code)]
fn ceiling_and_reservations(header: &Header, buf: &mut Vec<u8>) {
    let _ = Vec::<u8>::with_capacity(header.declared.clamp(0, THE_MOST_A_MESSAGE_IS));
    let _ = buf.try_reserve(header.declared);
    let _ = Vec::try_reserve_exact(buf, header.declared);
    buf.resize_with(header.declared, u8::default);
    let _ = buf.try_reserve(header.declared.min(THE_MOST_A_MESSAGE_IS));
}

fn main() {
    let header = Header { declared: 4 };
    let mut held = as_declared(&header);
    grown(&mut held, 4);
    let _ = filled(4);
    let _ = through_a_name(&header);
    let _ = under_a_ceiling(&header);
    let _ = measured(&held);
    let _ = measured_through_a_name(&held);
    let _ = written();
    let _ = listed();
}

#[allow(dead_code)]
fn counted(held: &[u8]) -> Vec<u8> {
    vec![0; held.iter().filter(|byte| **byte != 0).count()]
}

#[allow(dead_code)]
fn measured_box(held: &Box<[u8]>) -> Vec<u8> {
    vec![0; held.len()]
}
