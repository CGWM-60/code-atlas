//! Code Atlas core library.
//!
//! The binary, HTTP API and tests all use this same deterministic pipeline. Keeping
//! the analysis in a library prevents the UI from becoming a second implementation.

pub mod ai;
pub mod analyzer;
pub mod api;
pub mod api_explorer;
pub mod context_engine;
pub mod documentation;
pub mod engine;
pub mod features;
pub mod findings;
pub mod flow;
pub mod framework;
pub mod graph;
pub mod language;
pub mod library;
pub mod mcp;
pub mod model;
pub mod my_code;
pub mod porting;
pub mod scanner;
pub mod storage;
pub mod watcher;

pub use engine::{AnalysisResult, ProjectAnalyzer};

pub mod assistant;
pub mod assistant_tools;
pub mod dependencies;
pub mod embeddings;
pub mod intelligence;
pub mod retrieval;
pub mod taint;
