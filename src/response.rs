use crate::headers::Headers;
use std::io::{self, Error, ErrorKind, Write};

/// Provides methods to write an HTTP message:
///     HTTP-message   = start-line CRLF
///                      *( field-line CRLF )
///                      CRLF
///                      [ message-body ]
pub struct Writer<W: Write> {
    stream: W,
    state: WriterState,
}

// Batch:     Empty → StatusLineCompleted → HeadersCompleted → Done
// Streaming: Empty → StatusLineCompleted → HeadersCompleted → ChunkedBodyWritten → Done
enum WriterState {
    Empty,
    StatusLineCompleted,
    HeadersCompleted,
    ChunkedBodyWritten,
    Done,
}

impl<W: Write> Writer<W> {
    pub fn new(stream: W) -> Self {
        Writer {
            stream,
            state: WriterState::Empty,
        }
    }

    pub fn write_status_line(&mut self, status_code: StatusCode) -> io::Result<()> {
        match self.state {
            WriterState::Empty => {
                let r = write_status_line(&mut self.stream, status_code);
                self.state = WriterState::StatusLineCompleted;
                r
            }
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                "status line already written",
            )),
        }
    }

    pub fn write_headers(&mut self, headers: Headers) -> io::Result<()> {
        match self.state {
            WriterState::StatusLineCompleted => {
                let r = write_headers(&mut self.stream, headers);
                self.state = WriterState::HeadersCompleted;
                r
            }
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                "must call write_status_line() first",
            )),
        }
    }

    pub fn write_body(&mut self, p: &[u8]) -> io::Result<usize> {
        match self.state {
            WriterState::HeadersCompleted => {
                self.stream.write_all(p)?;
                self.state = WriterState::Done;
                Ok(p.len())
            }
            WriterState::Done => Err(Error::new(ErrorKind::InvalidInput, "body already written")),
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                "must call write_headers() first",
            )),
        }
    }

    // Useful for streaming responses where the total length is unknown upfront, e.g. when proxying
    // to an upstream server, streaming large generated responses, or sending server-sent events.
    pub(crate) fn write_chunked_body(&mut self, p: &[u8]) -> io::Result<usize> {
        match self.state {
            WriterState::HeadersCompleted | WriterState::ChunkedBodyWritten => {
                let n = p.len();
                write!(self.stream, "{:X}\r\n", n)?;
                self.stream.write_all(p)?;
                self.stream.write_all(b"\r\n")?;
                self.state = WriterState::ChunkedBodyWritten;
                Ok(n)
            }
            _ => Err(Error::new(ErrorKind::InvalidInput, "must call write_headers() first")),
        }
    }

    pub(crate) fn write_chunked_body_done(&mut self) -> io::Result<usize> {
        match self.state {
            WriterState::ChunkedBodyWritten => {
                self.stream.write_all(b"0\r\n")?;
                self.stream.write_all(b"\r\n")?;
                self.state = WriterState::Done;
                Ok(0)
            }
            _ => Err(Error::new(ErrorKind::InvalidInput, "must call write_chunked_body() first")),
        }
    }

    pub fn write_trailers(&mut self, headers: Headers) -> io::Result<()> {
        match self.state {
            WriterState::ChunkedBodyWritten => {
                let trailer_names = headers.get("trailer").cloned().ok_or_else(|| {
                    Error::new(ErrorKind::NotFound, "no trailer found in headers")
                })?;
                self.stream.write_all(b"0\r\n")?;
                for trailer in trailer_names.split(",") {
                    let trailer = trailer.trim();
                    let value = headers.get(trailer).ok_or_else(|| {
                        Error::new(ErrorKind::InvalidInput, "declared trailer missing from headers")
                    })?;
                    write!(self.stream, "{}: {}\r\n", trailer, value)?;
                }
                self.stream.write_all(b"\r\n")?;
                self.state = WriterState::Done;
                Ok(())
            }
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                "must call write_chunked_body() first",
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub enum StatusCode {
    Ok,          // 200
    ClientError, // 400
    ServerError, // 500
}

pub fn write_status_line(w: &mut impl Write, status_code: StatusCode) -> io::Result<()> {
    // Note: In most cases, w will be a TcpStream ― which implements the Write trait
    match status_code {
        StatusCode::Ok => w.write_all(b"HTTP/1.1 200 OK\r\n"),
        StatusCode::ClientError => w.write_all(b"HTTP/1.1 400 Bad Request\r\n"),
        StatusCode::ServerError => w.write_all(b"HTTP/1.1 500 Internal Server Error\r\n"),
    }
}

pub fn get_default_headers(content_len: usize) -> Headers {
    let mut headers = Headers::new();
    headers.insert("Content-Length".to_string(), content_len.to_string());
    headers.insert("Connection".to_string(), "close".to_string());
    headers.insert("Content-Type".to_string(), "text/plain".to_string());
    headers
}

pub fn write_headers(w: &mut impl Write, headers: Headers) -> io::Result<()> {
    let keys = [
        "Content-Type",
        "Content-Length",
        "Connection",
        "Transfer-Encoding",
    ];
    for key in keys.iter() {
        if let Some(value) = headers.get(key) {
            write!(w, "{}: {}\r\n", key, value)?;
        }
    }
    write!(w, "\r\n")
}

#[cfg(test)]
mod test {

    use test_case::test_case;

    use crate::headers::Headers;
    use crate::response::{StatusCode, Writer, WriterState, get_default_headers};

    #[test_case(StatusCode::Ok, b"HTTP/1.1 200 OK\r\n")]
    #[test_case(StatusCode::ClientError, b"HTTP/1.1 400 Bad Request\r\n")]
    #[test_case(StatusCode::ServerError, b"HTTP/1.1 500 Internal Server Error\r\n")]
    fn test_write_status_line(status_code: StatusCode, expected: &[u8]) {
        // Given
        let mut buf = Vec::new();
        let mut w = Writer::new(&mut buf);

        // When
        w.write_status_line(status_code).unwrap();

        // Then
        assert_eq!(buf, expected);
    }

    #[test]
    fn test_get_default_headers_helper_function() {
        // Given
        let content_len = 13_usize;

        // When
        let headers = get_default_headers(content_len);

        // Then
        assert_eq!(headers.get("Content-Length"), Some(&"13".to_string()));
        assert_eq!(headers.get("Connection"), Some(&"close".to_string()));
        assert_eq!(headers.get("Content-Type"), Some(&"text/plain".to_string()));
    }

    #[test]
    fn test_write_headers() {
        // Given
        let mut buf = Vec::new();
        let mut w = Writer::new(&mut buf);
        w.state = crate::response::WriterState::StatusLineCompleted;

        let headers = get_default_headers(13_usize);

        // When
        w.write_headers(headers).unwrap();

        // Then
        assert_eq!(
            buf,
            b"Content-Type: text/plain\r\nContent-Length: 13\r\nConnection: close\r\n\r\n"
        );
    }

    #[test]
    fn test_write_body() {
        // Given
        let mut buf = Vec::new();
        let mut w = Writer::new(&mut buf);
        w.state = crate::response::WriterState::HeadersCompleted;

        // When
        let bytes_written = w.write_body(b"hello").unwrap();

        // Then
        assert_eq!(buf, b"hello");
        assert_eq!(bytes_written, 5);
    }

    #[test]
    fn test_chunked_encoding() {
        // Given
        let mut buf = Vec::new();
        let mut w = Writer::new(&mut buf);
        w.state = WriterState::HeadersCompleted;

        // When
        let bytes_written = w.write_chunked_body(b"hello").unwrap();

        // Then
        assert_eq!(buf, b"5\r\nhello\r\n");
        assert_eq!(bytes_written, 5);
    }

    #[test]
    fn test_chunked_encoding_done() {
        // Given
        let mut buf = Vec::new();
        let mut w = Writer::new(&mut buf);
        w.state = WriterState::ChunkedBodyWritten;

        // When
        let bytes_written = w.write_chunked_body_done().unwrap();

        // Then
        assert_eq!(buf, b"0\r\n\r\n");
        assert_eq!(bytes_written, 0);
    }

    #[test]
    fn test_write_trailers() {
        // Given
        let mut buf = Vec::new();
        let mut w = Writer::new(&mut buf);
        w.state = crate::response::WriterState::ChunkedBodyWritten;

        let mut headers = get_default_headers(13_usize);
        headers.inner.remove("Content-Length");
        headers.insert("Trailer".to_string(), "X-Content-Length".to_string());
        headers.insert("X-Content-Length".to_string(), "15".to_string());

        // When
        w.write_trailers(headers).unwrap();

        // Then
        assert_eq!(buf, b"0\r\nX-Content-Length: 15\r\n\r\n");
    }
    #[test]
    fn test_write_trailers_raises_error_if_trailers_header_is_missing() {
        // Given
        let mut buf = Vec::new();
        let mut w = Writer::new(&mut buf);
        w.state = crate::response::WriterState::ChunkedBodyWritten;

        let mut headers = get_default_headers(13_usize);
        headers.inner.remove("Content-Length");
        headers.insert("X-Content-Length".to_string(), "15".to_string());

        // When
        let result = w.write_trailers(headers);

        // Then
        assert!(result.is_err());
    }

    #[test_case(WriterState::Empty, false)]
    #[test_case(WriterState::StatusLineCompleted, false)]
    #[test_case(WriterState::HeadersCompleted, false)]
    #[test_case(WriterState::ChunkedBodyWritten, true)]
    #[test_case(WriterState::Done, false)]
    fn test_write_trailers_state_machine(state: WriterState, should_succeed: bool) {
        // Given
        let mut buf = Vec::new();
        let mut w = Writer::new(&mut buf);
        w.state = state;

        let mut headers = Headers::new();
        headers.insert("Trailer".to_string(), "X-Content-Length".to_string());
        headers.insert("X-Content-Length".to_string(), "15".to_string());

        // When
        let result = w.write_trailers(headers);

        // Then
        assert_eq!(result.is_ok(), should_succeed);
    }
}
