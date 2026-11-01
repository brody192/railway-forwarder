//! In-memory account artwork, separate from saved user preferences.
#[derive(Debug)]
pub struct AccountPhoto {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}
