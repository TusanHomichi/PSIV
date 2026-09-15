//! ROM pointer to extracted shop portrait image, shared by every counter.
#[derive(Clone, Debug, serde::Deserialize)]
pub(super) struct Portrait {
    pub art: String,
    pub png: String,
}
