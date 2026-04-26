//! MCP server that proxies Kagi searches using a session token discovered
//! from the user's installed browsers.
//!
//! The crate is organised in hexagonal style. [`domain`] holds pure types
//! and the port traits. [`app`] contains the use cases (session discovery,
//! search orchestration) wired against those ports. [`adapters`] provides
//! the concrete I/O: a reqwest-backed Kagi client, a URL fetcher, browser
//! cookie readers, and the SERP HTML parser. [`mcp`] exposes the use cases
//! as JSON-RPC tools over stdio for an MCP host such as Claude Desktop.
//!
//! The binary entry point lives in `main.rs` and just composes these layers.

pub mod adapters;
pub mod app;
pub mod domain;
pub mod mcp;
