#![doc = "CardSpec fallback schema and action model crate."]

pub mod actions;
pub mod fallback;
pub mod schema;

pub use actions::{CardAction, CardActionKind};
pub use fallback::{fallback_from_result, FallbackReason};
pub use schema::{CardItem, CardSection, CardSpec, CardStatus};
