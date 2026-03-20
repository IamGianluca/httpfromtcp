use std::io::{self, Write};

use crate::headers::Headers;

#[derive(Debug, Clone)]
pub enum StatusCode {
    Okay = 200,
    ClientError = 400,
    ServerError = 500,
}

pub fn write_status_line(w: &mut impl Write, status_code: StatusCode) -> io::Result<()> {
    // In most cases, w will be a TcpStream ― which implements the Write trait

    // Generate a status line based on the StatusCode.
    // Note: Storing the status line to a variable is not very efficient. We
    // could simply write to the TcpStream and save the allocation
    let out = match status_code {
        StatusCode::Okay => "HTTP/1.1 200 OK\r\n",
        StatusCode::ClientError => "HTTP/1.1 400 Bad Request\r\n",
        StatusCode::ServerError => "HTTP/1.1 500 Internal Server Error\r\n",
    };

    // Write status code to TcpStream
    w.write_all(out.as_bytes())?;
    Ok(())
}

pub fn get_default_headers(content_len: u8) -> Headers {
    let mut headers = Headers::new();
    headers.insert("Content-Length".to_string(), content_len.to_string());
    headers.insert("Connection".to_string(), "close".to_string());
    headers.insert("Content-Type".to_string(), "text/plain".to_string());
    headers
}

#[cfg(test)]
mod test {
    use test_case::test_case;

    use crate::response::{StatusCode, get_default_headers, write_status_line};

    #[test_case(StatusCode::Okay, b"HTTP/1.1 200 OK\r\n")]
    #[test_case(StatusCode::ClientError, b"HTTP/1.1 400 Bad Request\r\n")]
    #[test_case(StatusCode::ServerError, b"HTTP/1.1 500 Internal Server Error\r\n")]
    fn test_write_status_line(status_code: StatusCode, expected: &[u8]) {
        // Given
        let mut buf = Vec::new();

        // When
        write_status_line(&mut buf, status_code).unwrap();

        // Then
        assert_eq!(buf, expected);
    }

    #[test]
    fn test_get_default_headers() {
        // Given
        let content_len = 13_u8;

        // When
        let headers = get_default_headers(content_len);

        // Then
        assert_eq!(headers.get("Content-Length"), Some(&"13".to_string()));
        assert_eq!(headers.get("Connection"), Some(&"close".to_string()));
        assert_eq!(headers.get("Content-Type"), Some(&"text/plain".to_string()));
    }
}
