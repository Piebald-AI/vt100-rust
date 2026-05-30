/// The internal identifier for OSC 8 hyperlink metadata stored on cells.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct HyperlinkId(pub(crate) u32);

/// Metadata for an OSC 8 hyperlink.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hyperlink {
    params: Vec<u8>,
    uri: Vec<u8>,
}

impl Hyperlink {
    pub(crate) fn new(params: Vec<u8>, uri: Vec<u8>) -> Self {
        Self { params, uri }
    }

    /// Returns the raw OSC 8 params payload.
    #[must_use]
    pub fn params(&self) -> &[u8] {
        &self.params
    }

    /// Returns the raw OSC 8 URI payload.
    #[must_use]
    pub fn uri(&self) -> &[u8] {
        &self.uri
    }
}
