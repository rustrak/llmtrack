//! llmtrack's benchmark kit.
//!
//! - [`mock`]: an OpenAI-compatible provider that answers instantly (or after
//!   a set delay), so what is measured is the gateway, not a model.
//! - [`load`]: a closed-loop load generator with one histogram per worker.
//! - [`resources`]: CPU and memory of the process or container under test.
//! - [`report`]: results as JSON, and the comparison as Markdown.

pub mod load;
pub mod mock;
pub mod report;
pub mod resources;
pub mod stats;
