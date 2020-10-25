use serde::{Deserialize, Serialize};
use url::Url;

/// Source of a package (where it is fetched from)
#[derive(
    Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub enum Source {
    Git { url: Url, rev: String },
    Registry { name: String, url: Url },
    Path(Url),
}
