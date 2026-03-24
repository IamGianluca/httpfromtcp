# HTTP from TCP

A Rust implementation of an HTTP/1.1 request parser built from scratch using TCP sockets.

This is a solution to the course ["Learn the HTTP Protocol in Go"](https://www.boot.dev/courses/learn-http-protocol-golang), implemented in Rust instead of Go.

## Features

- HTTP/1.1 request line parsing
- HTTP header parsing (case-insensitive, validates formatting)
- HTTP body parsing
- Streaming parser (handles chunked reads)
- Validates HTTP methods (GET, POST, PUT, DELETE, HEAD, OPTIONS, PATCH)
- HTTP response writing (status line, headers, body)
- Pluggable handler function for custom request handling
- TCP server on `127.0.0.1:42069` with graceful shutdown
- Comprehensive test suite

## Usage

### Run the HTTP server:
```bash
cargo run --bin httpserver
```

### Send a request:
```bash
curl http://127.0.0.1:42069/
curl http://127.0.0.1:42069/yourproblem
curl http://127.0.0.1:42069/myproblem
```

### Run the TCP listener (debugging tool):
```bash
cargo run --bin tcplistener
```

### Run tests:
```bash
cargo test
```

## Project Structure

```
src/
├── lib.rs              # Module exports
├── request.rs          # HTTP request parser (request line, headers, body)
├── headers.rs          # HTTP header parser
├── response.rs         # HTTP response writer (status line, headers)
├── server.rs           # TCP server with pluggable handler
└── bin/
    ├── httpserver.rs   # Main HTTP server binary
    ├── tcplistener.rs  # Debugging tool: prints parsed requests
    └── udpsender.rs    # UDP utilities
```

## Implementation Notes

This project demonstrates:
- **Streaming parsing**: Handles partial/chunked data from network reads
- **State machine**: Parser tracks completion state
- **Pluggable handlers**: `serve()` accepts a `Handler` function pointer for custom logic
- **Rust idioms**: Result types, BufRead trait, ownership patterns
- **Test-driven development**: Extensive test coverage with property-based testing

## Resources

- [Boot.dev course](https://www.boot.dev/courses/learn-http-protocol-golang)
- [YouTube lecture](https://www.youtube.com/watch?v=FknTw9bJsXM)
- [HTTP/1.1 RFC 9112](https://datatracker.ietf.org/doc/html/rfc9112)

## Credits

This project is based on the excellent ["Learn the HTTP Protocol in Go"](https://www.boot.dev/courses/learn-http-protocol-golang) course.

- **Course Author**: [ThePrimeagen](https://www.youtube.com/c/theprimeagen) - For creating this course and years of quality content
- **Platform**: [Boot.dev](https://www.boot.dev/) - For hosting and making quality programming education accessible
