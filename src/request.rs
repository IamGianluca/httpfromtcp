use std::io::{self, BufRead};

use crate::headers::Headers;

pub fn request_from_reader<R: BufRead>(mut reader: R) -> Result<Request, io::Error> {
    let mut request = Request::new();
    let mut bytes_buffered = 0;
    let chunk_size = 8_usize;
    let mut buffer = vec![0_u8; chunk_size];

    loop {
        // Streaming parser pattern: This loop alternates between two operations:
        // 1. Growing the buffer when full (bytes_buffered == buffer.len())
        // 2. Draining parsed data to free space (see buffer.drain below)
        // This ensures we always have space to read more data while keeping
        // memory usage bounded by only buffering unparsed data.

        // Grow buffer if needed
        if bytes_buffered >= buffer.len() {
            buffer.resize(buffer.len() + chunk_size, 0);
        }

        // Read from reader. Note that reader.read() will fill UP TO the remaining
        // buffer space (never exceeding buffer.len()).
        let bytes_read = reader.read(&mut buffer[bytes_buffered..])?;
        bytes_buffered += bytes_read;

        // Parse data in the buffer. If the parser was able to parse some data,
        // pop first bytes_parsed elements from the buffer. In this way, we can
        // reduce overall buffer size and memory consumption.
        let bytes_parsed = request.parse(&buffer[..bytes_buffered])?;
        if bytes_parsed > 0 {
            buffer.drain(..bytes_parsed); // Remove parsed bytes
            bytes_buffered -= bytes_parsed;
        }

        match request.status {
            RequestState::Initialized
            | RequestState::ParsingHeaders
            | RequestState::ParsingBody => {
                // If no more data available, exit
                if bytes_read == 0 {
                    break;
                }
                continue;
            }
            RequestState::Done => break,
        }
    }

    match request.status {
        RequestState::Done => Ok(request),
        RequestState::Initialized | RequestState::ParsingHeaders | RequestState::ParsingBody => {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not finish parsing request".to_string(),
            ))
        }
    }
}

#[derive(Debug)]
pub struct Request {
    // A parser
    pub request_line: RequestLine,
    pub headers: Headers,
    pub body: Vec<u8>,
    pub status: RequestState,
}

impl Default for Request {
    fn default() -> Self {
        Self::new()
    }
}

impl Request {
    pub fn new() -> Self {
        Request {
            request_line: RequestLine {
                http_version: String::new(),
                request_target: String::new(),
                method: String::new(),
            },
            headers: Headers::new(),
            body: Vec::<u8>::new(),
            status: RequestState::Initialized,
        }
    }

    pub fn parse(&mut self, data: &[u8]) -> Result<usize, io::Error> {
        // It accepts the next slice of bytes that needs to be parsed into the
        // Request struct. It updates the "state" of the parser (the Request
        // itself), and the parsed RequestLine and Headers fields. It returns
        // the number of bytes it consumed (meaning, successfully parsed) or
        // an error if it encountered one.
        let data_str = String::from_utf8_lossy(data);

        match self.status {
            RequestState::Initialized => {
                // Check if we have a complete line.
                let Some((before, _after)) = data_str.split_once("\r\n") else {
                    return Ok(0); // No CRLF found, need more data
                };
                // Parse request line
                let (request_line, bytes_parsed) = parse_request_line(before.to_string())?;
                self.request_line = request_line;
                // Flag that request line has been parsed and we can now expect to parse the headers
                self.status = RequestState::ParsingHeaders;

                Ok(bytes_parsed)
            }
            RequestState::ParsingHeaders => {
                // Parse headers
                let (bytes_parsed, done) = self.headers.parse(data)?;

                if done {
                    let content_length = self
                        .headers
                        .get("content-length")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(0);

                    if content_length > 0 {
                        self.status = RequestState::ParsingBody;
                    } else {
                        self.status = RequestState::Done;
                    }
                }

                Ok(bytes_parsed)
            }
            RequestState::ParsingBody => {
                // Get the expected content length
                let content_length = self
                    .headers
                    .get("content-length")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(0);

                // Calculate how many bytes we still need
                let bytes_needed = content_length.saturating_sub(self.body.len());

                // Take only what we need from the available data
                let bytes_to_consume = std::cmp::min(bytes_needed, data.len());

                // Append to body
                self.body.extend_from_slice(&data[..bytes_to_consume]);

                // Check if we've read the entire body
                if self.body.len() == content_length {
                    self.status = RequestState::Done;

                    // Check if more data in the buffer
                    if data.len() > bytes_to_consume {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "body longer than provided content length",
                        ));
                    }
                }

                Ok(bytes_to_consume)
            }
            RequestState::Done => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "request state set to Done",
            )),
        }
    }
}

fn parse_request_line(line_string: String) -> Result<(RequestLine, usize), io::Error> {
    if line_string.contains("\r\n") {
        eprintln!("Processed {} bytes", line_string.len() + 2); // 2 refers to the \r\n characters
    }
    let v: Vec<&str> = line_string.split_whitespace().collect();

    // Validate first-line length
    if v.len() != 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "more than 3 elements to parse",
        ));
    }

    // Extract and validate HTTP version
    let http_version = v[2]
        .strip_prefix("HTTP/")
        .ok_or(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing HTTP/ prefix",
        ))?
        .to_string();

    if http_version != "1.1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "only HTTP 1.1 is currently supported",
        ));
    }

    // Validate method
    match v[0] {
        "GET" | "POST" | "PUT" | "DELETE" | "HEAD" | "OPTIONS" | "PATCH" => {}
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid HTTP method",
            ));
        }
    }

    Ok((
        RequestLine {
            http_version,
            request_target: v[1].to_string(),
            method: v[0].to_string(),
        },
        line_string.len() + 2, // +2 bytes to account for \r\n
    ))
}

#[derive(Debug)]
pub struct RequestLine {
    pub http_version: String,
    pub request_target: String,
    pub method: String,
}

// An alternative, and perhaps better approach, could be to make Request itself
// be an Enum, with possible states <Initiatized, Done(RequestLine)>. This would
// avoid inconsistent states where state=Initiatized but request_line is
// assigned to a valid RequestLine object, etc.
#[derive(Debug)]
pub enum RequestState {
    Initialized,
    ParsingHeaders,
    ParsingBody,
    Done,
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Read};

    use self::request_from_reader;
    use super::*;
    use test_case::test_case;

    /// A reader that simulates reading data in small chunks from a network connection.
    /// Useful for testing streaming/partial reads.
    struct ChunkReader {
        data: String,
        num_bytes_per_read: usize,
        pos: usize,
    }

    impl ChunkReader {
        fn new(data: String, num_bytes_per_read: usize) -> Self {
            ChunkReader {
                data,
                num_bytes_per_read,
                pos: 0,
            }
        }
    }

    impl Read for ChunkReader {
        /// Read reads up to len(buf) or num_bytes_per_read bytes from the string per call.
        /// Returns the number of bytes read, or 0 to indicate EOF.
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            // If we've read all the data, return EOF (0 bytes read)
            if self.pos >= self.data.len() {
                return Ok(0);
            }

            // Calculate how much to read (min of chunk size and remaining data)
            let end_index = std::cmp::min(self.pos + self.num_bytes_per_read, self.data.len());

            // Get the chunk to read
            let chunk = &self.data.as_bytes()[self.pos..end_index];
            let n = chunk.len();

            // Copy chunk into the provided buffer
            buf[..n].copy_from_slice(chunk);

            // Update position
            self.pos += n;

            Ok(n)
        }
    }

    #[test]
    fn test_good_get_request_line() {
        // Given
        let data = "GET / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_ok(), "Expected Ok, got Err: {:?}", r.err());

        let r = r.unwrap();
        assert_eq!("GET", r.request_line.method);
        assert_eq!("/", r.request_line.request_target);
        assert_eq!("1.1", r.request_line.http_version);
    }

    #[test]
    fn test_good_get_request_line_with_path() {
        // Given
        let data = "GET /coffee HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_ok(), "Expected Ok, got Err: {:?}", r.err());

        let r = r.unwrap();
        assert_eq!("GET", r.request_line.method);
        assert_eq!("/coffee", r.request_line.request_target);
        assert_eq!("1.1", r.request_line.http_version);
    }

    #[test]
    fn test_invalid_number_of_parts_in_request_line() {
        // Given
        let data = "/coffee HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_err());
    }

    #[test]
    fn test_method_non_capitalized() {
        // Given
        let data = "get / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_err());
    }

    #[test]
    fn test_invalid_method() {
        // Given
        let data = "XXX / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_err());
    }

    #[test]
    fn test_only_http_1_1_supported() {
        // Given
        let data = "GET / HTTP/1.0\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_err());
    }
    #[test]
    fn test_good_post_request_with_path() {
        // Given
        let data = "POST /coffee HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_ok(), "Expected Ok, got Err: {:?}", r.err());

        let r = r.unwrap();
        assert_eq!("POST", r.request_line.method);
        assert_eq!("/coffee", r.request_line.request_target);
        assert_eq!("1.1", r.request_line.http_version);
    }

    #[test]
    fn test_empty_request() {
        // Given
        let data = "";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_err());
    }

    #[test]
    fn test_missing_http_version() {
        // Given
        let data = "GET /\r\n";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_err());
    }

    #[test]
    fn test_request_with_query_params() {
        // Given
        let data = "GET /coffee?flavor=dark HTTP/1.1\r\n\r\n";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_ok(), "Expected Ok, got Err: {:?}", r.err());

        let r = r.unwrap();
        assert_eq!("/coffee?flavor=dark", r.request_line.request_target);
    }

    #[test_case(2, "GE")]
    #[test_case(7, "GET /co")]
    #[test_case(10, "GET /coffe"; "max buffer length of 10-bytes is reached")]
    fn test_chunk_reader(chunk_length: usize, expected: &str) {
        // Given
        let data = "GET /coffee HTTP/1.1\r\n".to_string();
        let mut reader = ChunkReader::new(data, chunk_length); // 2 bytes per read
        let mut buf = [0_u8; 10];

        // When
        let n = reader.read(&mut buf).unwrap();

        // Then
        assert_eq!(n, chunk_length);
        assert_eq!(&buf[..chunk_length], expected.as_bytes());
    }

    #[test_case(1; "one byte chunks")]
    #[test_case(3; "three byte chunks")]
    #[test_case(22; "chunk as big as entire request")]
    fn test_chunk_reader_integration_in_request_from_reader(chunk_size: usize) {
        // Given
        let data = "GET /coffee HTTP/1.1\r\n\r\n".to_string();

        // Simulate network reading small chunks
        let chunk_reader = ChunkReader::new(data, chunk_size);
        let reader = BufReader::new(chunk_reader);

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_ok(), "Expected Ok, got Err: {:?}", r.err());

        let r = r.unwrap();
        assert_eq!(r.request_line.method, "GET");
        assert_eq!(r.request_line.request_target, "/coffee");
        assert_eq!(r.request_line.http_version, "1.1");
    }

    #[test_case("GET /coffee HTTP/1.1")]
    #[test_case("GET /coffee HTTP/1.1/\r")]
    #[test_case("GET /coffee HTTP/1.1/\n")]
    fn test_incomplete_request_hangs(data: &str) {
        // All these tests should fail because request does not include \r\n
        // Given
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_err());
    }

    #[test_case("GET", 0; "No delimiter - returns 0")]
    #[test_case("GET / HTTP/1.1\r\n", 16; "Request line (14) + delimiter (2)")]
    #[test_case("POST /coffee HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n", 23; "Only parses request line (21 + 2), ignores headers")]
    fn test_request_parse_return_correct_num_bytes_parsed(data: &str, expected: usize) {
        // Given
        let mut request = Request::new();

        // When
        let bytes_parsed = request.parse(data.as_bytes()).unwrap();

        // Then
        assert_eq!(bytes_parsed, expected);
    }

    #[test]
    fn test_standard_headers() {
        // Given
        let data = "GET / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n".to_string();
        let chunk_reader = ChunkReader::new(data, 3);
        let reader = BufReader::new(chunk_reader);

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_ok(), "Expected Ok, got Err: {:?}", r.err());
        let r = r.unwrap();
        assert_eq!(r.headers.get("host"), Some(&"localhost:42069".to_string()));
        assert_eq!(
            r.headers.get("user-agent"),
            Some(&"curl/7.81.0".to_string())
        );
        assert_eq!(r.headers.get("accept"), Some(&"*/*".to_string()));
    }

    #[test]
    fn test_malformed_header_missing_colon() {
        // Given
        let data = "GET / HTTP/1.1\r\nHost localhost:42069\r\n\r\n".to_string();
        let chunk_reader = ChunkReader::new(data, 3);
        let reader = BufReader::new(chunk_reader);

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_err());
    }

    #[test]
    fn test_request_with_no_headers() {
        // Tests the "Empty Headers" scenario explicitly
        // Given
        let data = "GET / HTTP/1.1\r\n\r\n";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_ok(), "Expected Ok, got Err: {:?}", r.err());
        let r = r.unwrap();
        assert_eq!(r.request_line.method, "GET");
        assert_eq!(r.headers.inner.len(), 0); // No headers
    }

    #[test]
    fn test_request_missing_end_of_headers() {
        // Tests "Missing End of Headers" - request never sends final \r\n\r\n
        // Given
        let data = "GET / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl\r\n";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_err()); // Should fail because headers never terminate
    }

    #[test]
    fn test_request_with_very_long_header_value() {
        // Edge case: header with very long value
        // Given
        let long_value = "a".repeat(1000);
        let data = format!("GET / HTTP/1.1\r\nX-Long-Header: {}\r\n\r\n", long_value);
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_ok(), "Expected Ok, got Err: {:?}", r.err());
        let r = r.unwrap();
        assert_eq!(r.headers.get("x-long-header"), Some(&long_value));
    }

    #[test]
    fn test_request_with_duplicate_headers_integration() {
        // Tests "Duplicate Headers" at request level (already covered in headers.rs)
        // Given
        let data = "GET / HTTP/1.1\r\nSet-Cookie: session=abc\r\nSet-Cookie: token=xyz\r\n\r\n";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_ok(), "Expected Ok, got Err: {:?}", r.err());
        let r = r.unwrap();
        assert_eq!(
            r.headers.get("set-cookie"),
            Some(&"session=abc, token=xyz".to_string())
        );
    }

    #[test]
    fn test_request_with_empty_header_value() {
        // Edge case: header with empty value
        // Given
        let data = "GET / HTTP/1.1\r\nX-Empty:\r\n\r\n";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_ok(), "Expected Ok, got Err: {:?}", r.err());
        let r = r.unwrap();
        assert_eq!(r.headers.get("x-empty"), Some(&"".to_string()));
    }

    #[test]
    fn test_request_header_with_only_spaces_in_value() {
        // Edge case: header value is only whitespace
        // Given
        let data = "GET / HTTP/1.1\r\nX-Spaces:     \r\n\r\n";
        let reader = BufReader::new(data.as_bytes());

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_ok(), "Expected Ok, got Err: {:?}", r.err());
        let r = r.unwrap();
        // trim() is applied, so should be empty string
        assert_eq!(r.headers.get("x-spaces"), Some(&"".to_string()));
    }

    #[test]
    fn test_standard_body() {
        // Given
        let data = "POST /submit HTTP/1.1\r\n\
Host: localhost:42069\r\n\
Content-Length: 13\r\n\
\r\n\
hello world!\n"
            .to_string();
        let chunk_reader = ChunkReader::new(data, 3);
        let reader = BufReader::new(chunk_reader);

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_ok(), "Expected Ok, got Err: {:?}", r.err());
        let r = r.unwrap();
        assert_eq!("hello world!\n", String::from_utf8_lossy(&r.body));
    }

    #[test]
    fn test_body_shorter_than_reported_content_length() {
        // Given
        let data = "POST /submit HTTP/1.1\r\n\
Host: localhost:42069\r\n\
Content-Length: 20\r\n\
\r\n\
partial content"
            .to_string();
        let chunk_reader = ChunkReader::new(data, 3);
        let reader = BufReader::new(chunk_reader);

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_err()); // Should fail because body is shorter than Content-Length
    }

    #[test]
    fn test_body_longer_than_reported_content_length() {
        // Given
        let data = "POST /submit HTTP/1.1\r\n\
Host: localhost:42069\r\n\
Content-Length: 1\r\n\
\r\n\
content exceeding provided content length\n"
            .to_string();
        let chunk_reader = ChunkReader::new(data, 3);
        let reader = BufReader::new(chunk_reader);

        // When
        let r = request_from_reader(reader);

        // Then
        assert!(r.is_err()); // Should fail because body is longer than Content-Length
    }
}
