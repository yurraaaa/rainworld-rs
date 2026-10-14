//! A bounds-checked cursor over serialized Unity data.

/// Reads fixed-size values, C strings and Unity's length-prefixed, 4-byte aligned
/// strings and arrays. Errors are messages naming the offset.
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    big_endian: bool,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            big_endian: false,
        }
    }

    pub fn big_endian(bytes: &'a [u8]) -> Self {
        Self {
            big_endian: true,
            ..Self::new(bytes)
        }
    }

    pub fn set_big_endian(&mut self, big_endian: bool) {
        self.big_endian = big_endian;
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn is_at_end(&self) -> bool {
        self.pos == self.bytes.len()
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or_else(|| format!("{n} bytes at {} run past the end", self.pos))?;
        let bytes = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    pub fn skip(&mut self, n: usize) -> Result<(), String> {
        self.take(n).map(drop)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        if self.big_endian {
            out.reverse();
        }
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    pub fn bool(&mut self) -> Result<bool, String> {
        self.u8().map(|b| b != 0)
    }

    pub fn u16(&mut self) -> Result<u16, String> {
        self.array().map(u16::from_le_bytes)
    }

    pub fn i16(&mut self) -> Result<i16, String> {
        self.array().map(i16::from_le_bytes)
    }

    pub fn u32(&mut self) -> Result<u32, String> {
        self.array().map(u32::from_le_bytes)
    }

    pub fn i32(&mut self) -> Result<i32, String> {
        self.array().map(i32::from_le_bytes)
    }

    pub fn u64(&mut self) -> Result<u64, String> {
        self.array().map(u64::from_le_bytes)
    }

    pub fn i64(&mut self) -> Result<i64, String> {
        self.array().map(i64::from_le_bytes)
    }

    pub fn f32(&mut self) -> Result<f32, String> {
        self.array().map(f32::from_le_bytes)
    }

    /// An `i32` count that must not be negative.
    pub fn count(&mut self) -> Result<usize, String> {
        let at = self.pos;
        let n = self.i32()?;
        usize::try_from(n).map_err(|_| format!("negative count {n} at {at}"))
    }

    /// A NUL-terminated UTF-8 string.
    pub fn cstr(&mut self) -> Result<&'a str, String> {
        let at = self.pos;
        let len = self.bytes[at.min(self.bytes.len())..]
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| format!("unterminated string at {at}"))?;
        let s = self.take(len)?;
        self.pos += 1;
        std::str::from_utf8(s).map_err(|_| format!("string at {at} is not UTF-8"))
    }

    /// Skips to the next multiple of 4 (relative to the start of the bytes).
    pub fn align(&mut self) {
        self.pos = self.pos.next_multiple_of(4);
    }

    /// `i32` length, that many bytes, then align (Unity's `vector<UInt8>` / string data).
    pub fn byte_array(&mut self) -> Result<&'a [u8], String> {
        let n = self.count()?;
        let bytes = self.take(n)?;
        self.align();
        Ok(bytes)
    }

    /// A Unity `string`: UTF-8 bytes with an `i32` length, aligned.
    pub fn string(&mut self) -> Result<String, String> {
        let at = self.pos;
        let bytes = self.byte_array()?;
        String::from_utf8(bytes.to_vec()).map_err(|_| format!("string at {at} is not UTF-8"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_endians_strings_and_alignment() {
        let bytes = [
            0, 0, 1, 2, b'h', b'i', 0, 0, 3, 0, 0, 0, b'a', b'b', b'c', 0, 7,
        ];
        let mut r = Reader::big_endian(&bytes);
        assert_eq!(r.u32().unwrap(), 0x0102);
        assert_eq!(r.cstr().unwrap(), "hi");
        r.set_big_endian(false);
        r.align();
        assert_eq!(r.string().unwrap(), "abc");
        assert_eq!(r.pos(), 16);
        assert_eq!(r.u8().unwrap(), 7);
        assert!(r.is_at_end());
        assert!(r.u8().is_err());
    }
}
