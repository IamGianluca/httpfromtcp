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
        let data_str = String::from_utf8_lossy(data).to_string();

        if let Some((before, _after)) = data_str.split_once("\r\n") {
            // If before is empty, we found the end of headers (\r\n at start)
            if before.is_empty() {
                return Ok((2, true)); // Consumed \r\n, done=true
            }

            // Parse the header line
            if let Some((key, value)) = before.split_once(":") {
                // Validate: no spaces before the colon (key must not have trailing spaces)
                if key.ends_with(" ") {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "spaces before colon not allowed",
                    ));
                }

                // Populate HashMap
                let key = key.trim().to_string();
                let value = value.trim().to_string();
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

    #[test_case("Host: localhost:42069\r\n\r\n", "localhost:42069", 23)]
    #[test_case("     Host: localhost:42070       \r\n\r\n", "localhost:42070", 35)]
    fn test_valid_single_header(data: &str, expected: &str, bytes_processed: usize) {
        let mut headers = Headers::new();

        let result = headers.parse(data.as_bytes());
        assert!(result.is_ok());

        let (n, done) = result.unwrap();
        assert_eq!(headers.get("Host"), Some(&expected.to_string()));
        assert_eq!(n, bytes_processed);
        assert!(!done);
    }

    #[test_case("       Host : localhost:99999       \r\n\r\n")]
    #[test_case("Host : localhost:42069       \r\n\r\n")]
    fn test_invalid_spacing_header(data: &str) {
        let mut headers = Headers::new();

        let result = headers.parse(data.as_bytes());
        assert!(result.is_err());
    }
}
