use std::io::{self, BufRead};

#[derive(Debug)]
pub struct Request {
    // A parser
    pub request_line: RequestLine,
    pub status: RequestState,
    cache: String,
}

impl Request {
    pub fn new() -> Self {
        Request {
            request_line: RequestLine {
                http_version: String::new(),
                request_target: String::new(),
                method: String::new(),
            },
            status: RequestState::Initialized,
            cache: String::new(),
        }
    }

    pub fn parse(&mut self, data: &[u8]) -> Result<usize, io::Error> {
        // It accepts the next slice of bytes that needs to be parsed into the Request struct
        // It updates the "state" of the parser (the Request itself), and the parsed RequestLine field.
        // It returns the number of bytes it consumed (meaning successfully parsed) and an error if it encountered one.
        self.cache.push_str(&String::from_utf8_lossy(data));

        // If cache contains \r\n, parse it and update RequestLine
        if self.cache.contains("\r\n") {
            // Make a copy of cache for internal usage, and reset self.cache to empty String
            let x = self.cache.clone();
            if let Some((before, after)) = x.split_once("\r\n") {
                self.cache = after.to_string();

                // Parse request line
                let (request_line, bytes_parsed) = parse_request_line(before.to_string())?;

                // Update request_line and status attributes
                self.request_line = request_line;
                self.status = RequestState::Done;

                // Return number of bytes successfully parsed
                return Ok(bytes_parsed);
            };
        }
        // Return placeholder to signal we still have cache to parse
        Ok(0)
    }
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
    Done,
}

pub fn request_from_reader<R: BufRead>(mut reader: R) -> Result<Request, io::Error> {
    let mut req = Request::new();
    let mut buf = [0_u8; 8];

    while let Ok(n) = reader.read(&mut buf) {
        if n == 0 {
            break; // EOF reached
        }
        match req.parse(&buf[..n]) {
            Ok(_) => {
                if matches!(req.status, RequestState::Done) {
                    break; // Done parsing
                }
            }
            Err(e) => return Err(e),
        }
    }

    match req.status {
        RequestState::Done => Ok(req),
        RequestState::Initialized => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "could not finish parsing request".to_string(),
        )),
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
        line_string.len(),
    ))
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
        let input = "GET / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = BufReader::new(input.as_bytes());

        let r = request_from_reader(reader);
        assert!(r.is_ok());

        let r = r.unwrap();
        assert_eq!("GET", r.request_line.method);
        assert_eq!("/", r.request_line.request_target);
        assert_eq!("1.1", r.request_line.http_version);
    }

    #[test]
    fn test_good_get_request_line_with_path() {
        let input = "GET /coffee HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = BufReader::new(input.as_bytes());

        let r = request_from_reader(reader);
        assert!(r.is_ok());

        let r = r.unwrap();
        assert_eq!("GET", r.request_line.method);
        assert_eq!("/coffee", r.request_line.request_target);
        assert_eq!("1.1", r.request_line.http_version);
    }

    #[test]
    fn test_invalid_number_of_parts_in_request_line() {
        let input = "/coffee HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = BufReader::new(input.as_bytes());

        let r = request_from_reader(reader);
        assert!(r.is_err());
    }

    #[test]
    fn test_method_non_capitalized() {
        let input = "get / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = BufReader::new(input.as_bytes());

        let r = request_from_reader(reader);
        assert!(r.is_err());
    }

    #[test]
    fn test_invalid_method() {
        let input = "XXX / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = BufReader::new(input.as_bytes());

        let r = request_from_reader(reader);
        assert!(r.is_err());
    }

    #[test]
    fn test_only_http_1_1_supported() {
        let input = "GET / HTTP/1.0\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = BufReader::new(input.as_bytes());

        let r = request_from_reader(reader);
        assert!(r.is_err());
    }
    #[test]
    fn test_good_post_request_with_path() {
        let input = "POST /coffee HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let reader = BufReader::new(input.as_bytes());

        let r = request_from_reader(reader);
        assert!(r.is_ok());

        let r = r.unwrap();
        assert_eq!("POST", r.request_line.method);
        assert_eq!("/coffee", r.request_line.request_target);
        assert_eq!("1.1", r.request_line.http_version);
    }

    #[test]
    fn test_empty_request() {
        let input = "";
        let reader = BufReader::new(input.as_bytes());
        let r = request_from_reader(reader);
        assert!(r.is_err());
    }

    #[test]
    fn test_missing_http_version() {
        let input = "GET /\r\n";
        let reader = BufReader::new(input.as_bytes());
        let r = request_from_reader(reader);
        assert!(r.is_err());
    }

    #[test]
    fn test_request_with_query_params() {
        let input = "GET /coffee?flavor=dark HTTP/1.1\r\n\r\n";
        let reader = BufReader::new(input.as_bytes());

        let r = request_from_reader(reader);
        assert!(r.is_ok());

        let r = r.unwrap();
        assert_eq!("/coffee?flavor=dark", r.request_line.request_target);
    }

    #[test_case(2, "GE")]
    #[test_case(7, "GET /co")]
    #[test_case(10, "GET /coffe"; "max buffer length of 10-bytes is reached")]
    fn test_chunk_reader(chunk_length: usize, expected: &str) {
        let input = "GET /coffee HTTP/1.1\r\n".to_string();
        let mut reader = ChunkReader::new(input, chunk_length); // 2 bytes per read

        let mut buf = [0_u8; 10];
        let n = reader.read(&mut buf).unwrap();
        assert_eq!(n, chunk_length);
        assert_eq!(&buf[..chunk_length], expected.as_bytes());
    }

    #[test_case(1; "one byte chunks")]
    #[test_case(3; "three byte chunks")]
    #[test_case(22; "chunk as big as entire request")]
    fn test_chunk_reader_integration_in_request_from_reader(chunk_size: usize) {
        let input = "GET /coffee HTTP/1.1\r\n".to_string();
        // Simulate network reading small chunks
        let chunk_reader = ChunkReader::new(input, chunk_size);
        let reader = BufReader::new(chunk_reader);

        let r = request_from_reader(reader);
        assert!(r.is_ok());

        let r = r.unwrap();
        assert_eq!(r.request_line.method, "GET");
        assert_eq!(r.request_line.request_target, "/coffee");
        assert_eq!(r.request_line.http_version, "1.1");
    }

    #[test_case("GET /coffee HTTP/1.1")]
    #[test_case("GET /coffee HTTP/1.1/\r")]
    #[test_case("GET /coffee HTTP/1.1/\n")]
    fn test_incomplete_request_hangs(input: &str) {
        // All these tests should fail because request does not include \r\n
        let reader = BufReader::new(input.as_bytes());
        let r = request_from_reader(reader);
        assert!(r.is_err());
    }
}
