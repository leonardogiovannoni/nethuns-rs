//! Buffer descriptors and references for zero-copy packet handling.

/// Opaque descriptor representing a buffer in a memory pool.
///
/// This is typically an offset or index into a UMEM region or similar.
#[derive(Clone, Copy, Debug)]
pub struct BufferDesc(pub(crate) usize);

impl From<usize> for BufferDesc {
    fn from(val: usize) -> Self {
        Self(val)
    }
}

impl From<BufferDesc> for usize {
    fn from(val: BufferDesc) -> usize {
        val.0
    }
}

/// Reference to a buffer, used in some backends (e.g., netmap).
#[derive(Clone, Copy, Debug)]
pub struct BufferRef(pub(crate) usize);

impl From<usize> for BufferRef {
    fn from(val: usize) -> Self {
        Self(val)
    }
}

impl From<BufferRef> for usize {
    fn from(val: BufferRef) -> usize {
        val.0
    }
}

impl From<BufferRef> for BufferDesc {
    fn from(val: BufferRef) -> Self {
        BufferDesc(val.0)
    }
}

impl From<BufferDesc> for BufferRef {
    fn from(val: BufferDesc) -> Self {
        BufferRef(val.0)
    }
}
