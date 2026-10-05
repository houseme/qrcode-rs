// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredAppendInfo {
    pub index: i32, // -1;
    pub count: i32, // = -1;
    pub id: String,
}

impl Default for StructuredAppendInfo {
    fn default() -> Self {
        Self { index: -1, count: -1, id: Default::default() }
    }
}
