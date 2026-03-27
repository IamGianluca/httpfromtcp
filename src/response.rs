use std::io::{self, Write};

use crate::headers::Headers;

pub struct Writer<W: Write> {
    stream: W,
}

impl<W: Write> Writer<W> {
    pub fn new(stream: W) -> Self {
        Writer { stream }
    }

    pub fn write_status_line(&mut self, status_code: StatusCode) -> io::Result<()> {
        let _ = write_status_line(&mut self.stream, status_code);
        Ok(())
    }

    pub fn export_headers(&mut self, headers: Headers) -> io::Result<()> {
        let _ = write_headers(&mut self.stream, headers);
        Ok(())
    }

    pub fn write_body(&mut self, p: &[u8]) -> io::Result<usize> {
        let _ = self.stream.write_all(p);
        Ok(p.len())
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
    let keys = ["Content-Type", "Content-Length", "Connection"];
    for key in keys.iter() {
        if let Some(value) = headers.get(key) {
            write!(w, "{}: {}\r\n", key, value)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {

    use test_case::test_case;

    use crate::response::{StatusCode, Writer, get_default_headers};

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

        let headers = get_default_headers(13_usize);

        // When
        w.export_headers(headers).unwrap();

        // Then
        assert_eq!(
            buf,
            b"Content-Type: text/plain\r\nContent-Length: 13\r\nConnection: close\r\n"
        );
    }

    #[test]
    fn test_write_body() {
        // Given
        let buf = Vec::new();
        let mut w = Writer::new(buf);

        // When
        let result = w.write_body(b"hello").unwrap();

        // Then
        assert_eq!(result, 5);
    }
}
