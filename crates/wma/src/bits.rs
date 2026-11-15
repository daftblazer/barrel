//! MSB-first bit reading and writing.

/// Reads bits most significant first; past the end it reads zeros and notes the overrun.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    len: usize,
}

impl<'a> Reader<'a> {
    /// A reader over the first `len` bits of `data`.
    pub fn new(data: &'a [u8], len: usize) -> Self {
        Reader { data, pos: 0, len: len.min(data.len() * 8) }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn left(&self) -> isize {
        self.len as isize - self.pos as isize
    }

    pub fn over(&self) -> bool {
        self.pos > self.len
    }

    fn bit_at(&self, at: usize) -> u32 {
        if at >= self.len { 0 } else { ((self.data[at >> 3] >> (7 - (at & 7))) & 1) as u32 }
    }

    /// The next `n` bits (up to 32) without moving.
    pub fn peek(&self, n: u32) -> u32 {
        let mut v = 0u32;
        for k in 0..n as usize {
            v = (v << 1) | self.bit_at(self.pos + k);
        }
        v
    }

    pub fn read(&mut self, n: u32) -> u32 {
        let v = self.peek(n);
        self.pos += n as usize;
        v
    }

    pub fn bit(&mut self) -> bool {
        self.read(1) != 0
    }

    pub fn skip(&mut self, n: usize) {
        self.pos += n;
    }

    pub fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }
}

/// Collects bits most significant first.
#[derive(Default, Clone)]
pub struct Writer {
    pub data: Vec<u8>,
    pub len: usize,
}

impl Writer {
    pub fn clear(&mut self) {
        self.data.clear();
        self.len = 0;
    }

    pub fn push(&mut self, bit: bool) {
        if self.len & 7 == 0 {
            self.data.push(0);
        }
        if bit {
            *self.data.last_mut().unwrap() |= 0x80 >> (self.len & 7);
        }
        self.len += 1;
    }

    /// Appends `n` bits from `r`.
    pub fn take(&mut self, r: &mut Reader, n: usize) {
        for _ in 0..n {
            self.push(r.bit());
        }
    }
}
