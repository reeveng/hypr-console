/// Growable buffer for building boxes.
#[derive(Default, Clone, Debug)]
pub(crate) struct BoxBuf {
    pub buf: Vec<u8>,
}

impl BoxBuf {
    pub fn new() -> Self {
        Self::default()
    }
    /// Begin a box; returns a marker to pass to [`end`](Self::end).
    pub fn start(&mut self, kind: &[u8; 4]) -> usize {
        let at = self.buf.len();
        self.buf.extend_from_slice(&[0, 0, 0, 0]);
        self.buf.extend_from_slice(kind);
        at
    }
    pub fn start_full(&mut self, kind: &[u8; 4], version: u8, flags: u32) -> usize {
        let at = self.start(kind);
        self.u32(((version as u32) << 24) | (flags & 0xFF_FFFF));
        at
    }
    pub fn end(&mut self, at: usize) {
        let size = (self.buf.len() - at) as u32;
        self.buf[at..at + 4].copy_from_slice(&size.to_be_bytes());
    }
    /// Append a complete box with the given payload.
    pub fn leaf(&mut self, kind: &[u8; 4], payload: &[u8]) {
        let at = self.start(kind);
        self.bytes(payload);
        self.end(at);
    }
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    pub fn i16(&mut self, v: i16) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
    pub fn zeros(&mut self, n: usize) {
        self.buf.resize(self.buf.len() + n, 0);
    }
    pub fn len(&self) -> usize {
        self.buf.len()
    }
}
