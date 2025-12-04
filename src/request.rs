use std::io::{self, BufRead};

pub struct Request {
    pub request_line: RequestLine,
}

pub struct RequestLine {
    pub http_version: String,
    pub request_target: String,
    pub method: String,
}

pub fn request_from_reader<R: BufRead>(mut reader: R) -> Result<Request, io::Error> {
    let mut line_string = String::new();
    reader.read_line(&mut line_string)?;

    let v: Vec<&str> = line_string.split_whitespace().collect();

    // Validate first-line length
    if v.len() != 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid request line format",
        ));
    }

    // Extract HTTP version
    let http_version = v[2]
        .strip_prefix("HTTP/")
        .ok_or(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing HTTP/ prefix",
        ))?
        .to_string();

    // Validate method
    if v[0] != v[0].to_uppercase() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "method must be uppercase",
        ));
    }

    Ok(Request {
        request_line: RequestLine {
            http_version,
            request_target: v[1].to_string(),
            method: v[0].to_string(),
        },
    })
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use self::request_from_reader;
    use super::*;

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
}
