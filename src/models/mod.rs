mod attachment;
mod comment;
mod component;
mod enums;
mod milestone;
mod notification;
mod pagination;
mod password;
mod session;
mod ticket;
mod user;

pub use attachment::*;
pub use comment::*;
pub use component::*;
pub use enums::*;
pub use milestone::*;
pub use notification::*;
pub use pagination::*;
pub use password::*;
pub use session::*;
pub use ticket::*;
pub use user::*;

use serde::{Deserialize, Deserializer};

/// Deserializes a double-Option field for PATCH semantics.
///
/// - Field absent → `None` (don't touch)
/// - Field present as `null` → `Some(None)` (set to null)
/// - Field present with value → `Some(Some(v))` (set to value)
pub fn deserialize_optional_field<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}
