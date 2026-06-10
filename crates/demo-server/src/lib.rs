#![doc = "Demo merchant Agent server for the coffee Skill MVP."]

pub mod audit;
pub mod auth;
pub mod coffee;
pub mod routes;

pub use routes::{app, DemoState};
