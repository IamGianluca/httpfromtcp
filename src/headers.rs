use std::{collections::HashMap, io};

pub struct Headers {
    pub inner: HashMap<String, String>,
}

impl Default for Headers {
    fn default() -> Self {
        Self::new()
    }
}

impl Headers {
    pub fn new() -> Self {
        Headers {
            inner: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        // HTTP headers are case-insensitive
        self.inner.get(&key.to_lowercase())
    }

    pub fn insert(&mut self, key: String, value: String) {
        // HTTP headers are case-insensitive
        self.inner.insert(key.to_lowercase(), value);
    }

    pub fn parse(&mut self, data: &[u8]) -> Result<(usize, bool), io::Error> {
        // Note: This function will be called over and over until all the
        // headers are parsed, and it can only parse one key/value pair at a time.
        let data_str = String::from_utf8_lossy(data).to_string();

        // Check whether data include CRLF
        if let Some((before, _after)) = data_str.split_once("\r\n") {
            // If nothing before CRLF, we found the end of headers (\r\n at start)
            if before.is_empty() {
                return Ok((2, true)); // Consumed \r\n, done=true
            }

            // Parse header line
            if let Some((key, value)) = before.split_once(":") {
                // Validate: no spaces before the colon (field name must not have trailing spaces)
                if key.ends_with(" ") {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "spaces before colon not allowed",
                    ));
                }

                // Validate: field name contains only valid characters
                let key = key.trim().to_string();
                if !key
                    .chars()
                    .all(|c| c.is_ascii_alphabetic() || "!#$%&'*+-.^_`|~".contains(c))
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid character in header name",
                    ));
                }

                let value = value.trim().to_string();

                // Populate HashMap
                self.insert(key, value);

                return Ok((before.len() + 2, false)); // Parsed one header, not done yet
            }

            // No colon found
            Err(io::Error::new(io::ErrorKind::InvalidData, "missing colon"))
        } else {
            // No CRLF found, need more data
            Ok((0, false))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test]
    fn test_valid_single_header() {
        // Given
        let mut headers = Headers::new();
        let data = "Host: localhost:42069\r\n\r\n";

        // When
        let result = headers.parse(data.as_bytes());

        // Then
        assert!(result.is_ok());

        let (n, done) = result.unwrap();
        assert_eq!(headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(n, 23);
        assert!(!done);
    }

    #[test]
    fn test_valid_single_header_with_extra_whitespaces() {
        // Given
        let mut headers = Headers::new();
        let data = "     Host: localhost:42069       \r\n\r\n";

        // When
        let result = headers.parse(data.as_bytes());

        // Then
        assert!(result.is_ok());

        let (n, done) = result.unwrap();
        assert_eq!(headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(n, 35);
        assert!(!done);
    }

    #[test]
    fn test_two_headers_with_existing_headers() {
        // Given
        let mut headers = Headers::new();
        let data1 = "Host: localhost:42069\r\n";

        // When: First call to parse() parses "Host: localhost:42069"
        let (n1, done1) = headers.parse(data1.as_bytes()).unwrap();

        // Then
        assert_eq!(n1, 23);
        assert!(!done1);

        // Given
        let data2 = "User-Agent: curl/7.81.0\r\n";

        // When: Second call to parse() parses "User-Agent: curl/7.81.0"
        // The headers map already has "Host" in it (existing headers)
        let (n2, done2) = headers.parse(data2.as_bytes()).unwrap();

        // Then
        assert_eq!(n2, 25);
        assert!(!done2);

        // Verify BOTH headers exist in the map
        assert_eq!(headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(headers.get("User-Agent"), Some(&"curl/7.81.0".to_string()));
    }

    #[test]
    fn test_valid_done() {
        // Given
        let mut headers = Headers::new();
        let data1 = "Host: localhost:42069\r\n";

        // When: first call to parse
        let result1 = headers.parse(data1.as_bytes());

        // Then
        assert!(result1.is_ok());

        let (n, done) = result1.unwrap();
        assert_eq!(headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(n, 23);
        assert!(!done);

        // Given
        let data2 = "\r\n";

        // When: second call to parse
        let result2 = headers.parse(data2.as_bytes());

        // Then
        assert!(result2.is_ok());
        let (n, done) = result2.unwrap();
        assert_eq!(n, 2);
        assert!(done);
    }

    #[test_case("       Host : localhost:99999       \r\n\r\n")]
    #[test_case("Host : localhost:42069       \r\n\r\n")]
    fn test_invalid_spacing_header(data: &str) {
        // Given
        let mut headers = Headers::new();

        // When: parsing header with space before colon (invalid per HTTP spec)
        let result = headers.parse(data.as_bytes());

        // Then
        assert!(result.is_err());
    }

    #[test_case("Host: localhost:42069\r\n\r\n"; "uppercase header")]
    #[test_case("host: localhost:42069\r\n\r\n"; "lowercase header")]
    fn test_header_case_insensitive(data: &str) {
        // Given
        let mut headers = Headers::new();

        // When
        let result = headers.parse(data.as_bytes());

        // Then
        assert!(result.is_ok());
        assert_eq!(headers.get("host"), Some(&"localhost:42069".to_string()));
        assert_eq!(headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(headers.get("HOST"), Some(&"localhost:42069".to_string()));
    }

    #[test_case("Ho st: value\r\n"; "space in field name")]
    #[test_case("Host@Name: value\r\n"; "@ symbol")]
    #[test_case("Host(Name): value\r\n"; "parentheses")]
    #[test_case("Host[Name]: value\r\n"; "square brackets")]
    #[test_case("Host{Name}: value\r\n"; "curly braces")]
    #[test_case("Host/Name: value\r\n"; "forward slash")]
    #[test_case("Host\\Name: value\r\n"; "backslash")]
    #[test_case("Host;Name: value\r\n"; "semicolon")]
    #[test_case("Host=Name: value\r\n"; "equals sign")]
    #[test_case("Host,Name: value\r\n"; "comma")]
    fn test_header_with_invalid_character(data: &str) {
        // Given
        let mut headers = Headers::new();

        // When
        let result = headers.parse(data.as_bytes());

        // Then
        assert!(result.is_err());
    }
}
