//! # ormify
//!
//! A type-safe, asynchronous ORM core with a pluggable backend boundary.
//!
//! `ormify` provides entity metadata, typed builders, transport-neutral
//! values, query construction, and the [`Ormify`] orchestration engine.
//! Database dialects and drivers (for example PostgreSQL) live in separate
//! crates such as [`ormify_postgres`](https://docs.rs/ormify_postgres).
//!
//! The core does not choose an async runtime or database client. A backend
//! implements [`Language`] to compile model operations into [`LanguageQuery`]
//! values and [`Driver`] to execute those values.
//!
//! ## Typical flow
//!
//! 1. Derive [`Entity`] for application structs.
//! 2. Submit `Entity::init()` executions to register schemas and create tables.
//! 3. Build typed queries and executions with the generated APIs.
//! 4. Let [`Ormify`] compile and dispatch them through the backend driver.
//!
//! Initialization commands are submitted as one transaction. The engine
//! resolves included foreign-key initialization dependencies before compiling
//! them; cyclic dependencies require a backend migration strategy.
//!
//! ## Feature flags
//!
//! - `orm_engine` enables [`Ormify`], [`Language`], [`Driver`], and execution orchestration.
//! - `proc_macro` enables the [`Entity`] and [`FieldType`] derives and [`c!`] macro.
//! - `serde_requests` enables serializable request/response envelopes.
//! - `full` enables `orm_engine`, `serde_requests`, and `proc_macro`.
//! - `bench` enables benchmark recording on the engine. *Feature for testing.*
//!
//! ## Architecture
//!
//! The crate is split into four layers:
//!
//! 1. **Model** — [`Entity`], [`Scheme`], [`Constraint`], and [`Value`].
//! 2. **Query** — [`Query`], [`Filters`], [`Condition`], [`Join`].
//! 3. **Language** — trait-based SQL dialect compilation.
//! 4. **Engine** — [`Ormify`] orchestrates schemes, queries, and a driver.
//!
//! ```text
//!   Entity derive
//!        │
//!        ▼
//!   Scheme / Constraint / Entry
//!        │
//!        ▼
//!   Query / Execution
//!        │
//!        ▼
//!   Language::parse_*  ──►  LanguageQuery
//!        │
//!        ▼
//!   Driver::query / execute  ──►  Entry / ExecutionResponse
//! ```
//!

#[cfg(feature = "bench")]
mod bench;
mod constraint;
#[cfg(feature = "orm_engine")]
mod engine;
mod entity;
mod err;
mod field;
mod filter;
mod query;
#[cfg(feature = "serde_requests")]
mod request;
mod scheme;
mod transform;
mod util;

#[cfg(feature = "bench")]
pub use bench::*;
pub use constraint::*;
#[cfg(feature = "orm_engine")]
pub use engine::*;
pub use entity::*;
pub use err::*;
pub use field::*;
pub use filter::*;
#[cfg(feature = "proc_macro")]
pub use ormify_macro::*;
pub use query::*;
#[cfg(feature = "serde_requests")]
pub use request::*;
pub use scheme::*;
pub use transform::*;
pub use util::*;
