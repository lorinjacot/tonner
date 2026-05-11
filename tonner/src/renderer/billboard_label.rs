#[non_exhaustive]
#[derive(Debug)]
pub struct BillboardLabel {
    pub text: String,
}

impl BillboardLabel {
    pub fn new(text: impl Into<String>) -> Self {
        BillboardLabel { text: text.into() }
    }
}