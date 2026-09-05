use anyhow::{Context, Result};
use code_atlas::{ProjectAnalyzer, api, mcp};
use std::{net::SocketAddr, path::PathBuf};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".into());
    if command == "mcp" {
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .init();
    } else {
        tracing_subscriber::fmt::init();
    }
    match command.as_str() {
        "serve" => {
            let bind: SocketAddr = option(&mut args, "--bind")
                .unwrap_or_else(|| "127.0.0.1:3000".into())
                .parse()
                .context("invalid --bind address")?;
            let database = PathBuf::from(
                option(&mut args, "--db").unwrap_or_else(|| ".code-atlas/code-atlas.sqlite".into()),
            );
            api::serve(bind, &database).await
        }
        "analyze" => analyze(
            args.next()
                .context("usage: code-atlas analyze <path> [--json]")?,
            args.any(|value| value == "--json"),
        ),
        "mcp" => {
            let database = PathBuf::from(
                option(&mut args, "--db").unwrap_or_else(|| ".code-atlas/code-atlas.sqlite".into()),
            );
            mcp::serve_stdio(&database).await
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        // Backward-compatible shorthand retained from the initial prototype.
        path => analyze(path.to_string(), args.any(|value| value == "--json")),
    }
}
fn analyze(path: String, json: bool) -> Result<()> {
    let result = ProjectAnalyzer.analyze(path)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "Code Atlas analysis complete\nRoot: {}\nFiles: {} (parsed {}, reused {})\nNodes: {}\nEdges: {}\nUnresolved calls: {}",
            result.scan.root,
            result.scan.total_files,
            result.analyzed_files,
            result.reused_files,
            result.graph.nodes.len(),
            result.graph.edges.len(),
            result.graph.unresolved_calls.len()
        );
    }
    Ok(())
}
fn option(args: &mut impl Iterator<Item = String>, name: &str) -> Option<String> {
    while let Some(value) = args.next() {
        if value == name {
            return args.next();
        }
    }
    None
}
fn print_help() {
    println!(
        "Code Atlas\n\n  code-atlas analyze <path> [--json]\n  code-atlas serve [--bind 127.0.0.1:3000] [--db code-atlas.sqlite]\n  code-atlas mcp [--db code-atlas.sqlite]\n\nThe server also serves frontend/dist when it exists."
    )
}
