# HTTP from TCP

A Rust implementation of an HTTP/1.1 request parser built from scratch using TCP sockets.

This is a solution to the course ["Learn the HTTP Protocol in Go"](https://www.boot.dev/courses/learn-http-protocol-golang), implemented in Rust instead of Go.

## Features

- HTTP/1.1 request line parsing
- HTTP header parsing (case-insensitive, validates formatting)
- Streaming parser (handles chunked reads)
- Validates HTTP methods (GET, POST, PUT, DELETE, HEAD, OPTIONS, PATCH)
- TCP listener on `127.0.0.1:42069`
- Comprehensive test suite 

## Usage

### Run the TCP listener:
```bash
cargo run --bin tcplistener
```

### Send a request:
```bash
curl http://127.0.0.1:42069/
```

### Run tests:
```bash
cargo test
```

## Project Structure

```
src/
├── lib.rs              # Module exports
├── request.rs          # HTTP request line parser
├── headers.rs          # HTTP header parser
└── bin/
    ├── tcplistener.rs  # TCP server implementation
    └── udpsender.rs    # UDP utilities
```

## Implementation Notes

This project demonstrates:
- **Streaming parsing**: Handles partial/chunked data from network reads
- **State machine**: Parser tracks completion state
- **Rust idioms**: Result types, BufRead trait, ownership patterns
- **Test-driven development**: Extensive test coverage with property-based testing

## Limitations

Currently parses request line and headers. Body parsing is not yet implemented.

## Resources

- [Boot.dev course](https://www.boot.dev/courses/learn-http-protocol-golang)
- [YouTube lecture](https://www.youtube.com/watch?v=FknTw9bJsXM)
- [HTTP/1.1 RFC 9112](https://datatracker.ietf.org/doc/html/rfc9112)

## Credits

This project is based on the excellent ["Learn the HTTP Protocol in Go"](https://www.boot.dev/courses/learn-http-protocol-golang) course.

- **Course Author**: [ThePrimeagen](https://www.youtube.com/c/theprimeagen) - For creating this course and years of quality content
- **Platform**: [Boot.dev](https://www.boot.dev/) - For hosting and making quality programming education accessible
