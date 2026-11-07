use buntrail_core::contracts::*;
use tree_sitter::Point;
pub struct CoordinateMapper<'a>(pub &'a DocumentSnapshot);
impl CoordinateMapper<'_> {
    pub fn native(&self, bytes: std::ops::Range<usize>) -> Result<BufferRange, CoreError> {
        TextByteRange {
            revision: self.0.revision,
            start: bytes.start as u64,
            end: bytes.end as u64,
        }
        .to_utf16(self.0)
    }
    pub fn bytes(&self, range: &BufferRange) -> Result<TextByteRange, CoreError> {
        range.to_bytes(self.0)
    }
}
pub(crate) fn point(text: &str, byte: usize) -> Point {
    let prefix = &text[..byte];
    Point::new(
        prefix.bytes().filter(|b| *b == b'\n').count(),
        prefix.rsplit('\n').next().unwrap_or("").len(),
    )
}
