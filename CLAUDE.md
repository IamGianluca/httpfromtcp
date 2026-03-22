# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                    # Build the project
cargo test                     # Run all tests
cargo test --release           # Run tests with optimizations
cargo run --bin httpserver     # Run the HTTP server
cargo run --bin tcplistener    # Run the basic TCP listener
```

To run a single test:
```bash
cargo test test_name           # Run tests matching name
cargo test -- --nocapture      # Run tests with output
```

## Architecture

This is a Rust implementation of an HTTP/1.1 request parser built from TCP sockets, ported from the Boot.dev "Learn HTTP Protocol" course.

The TCP server listens on `127.0.0.1:42069`. The server spawns a thread per connection; graceful shutdown uses the `Drop` trait to signal closure and unblock the listener with a dummy connection.

### Core Modules (`src/`)

- **`request.rs`** — Streaming HTTP request parser. Uses a `RequestState` enum state machine (`Initialized → ParsingHeaders → ParsingBody → Done`). Handles partial/chunked reads by buffering and draining parsed bytes. Entry point: `request_from_reader()`.
- **`headers.rs`** — `Headers` struct backed by a `HashMap`. Keys stored lowercase (case-insensitive). Duplicate headers concatenated with `", "` per spec. Returns the number of bytes consumed on each `parse()` call.
- **`response.rs`** — `StatusCode` enum (Ok/400/500), `write_status_line()`, `write_headers()`, `get_default_headers()`.
- **`server.rs`** — `Server` struct. `serve()` accepts connections on a background thread; `handle()` writes the HTTP response.

### Binaries (`src/bin/`)

- `httpserver.rs` — Main server with Ctrl+C graceful shutdown via `ctrlc` crate.
- `tcplistener.rs` — Debugging tool; prints parsed request details.
- `udpsender.rs` — Interactive UDP message sender.

### Testing

Tests use a custom `ChunkReader` that simulates network reads in 1–22 byte chunks to validate the streaming parser. Parameterized tests use the `test-case` crate (`#[test_case]`).

## Dependencies

- `ctrlc` — Ctrl+C signal handling
- `test-case` — Parameterized test macros
