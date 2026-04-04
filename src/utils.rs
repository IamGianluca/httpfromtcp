use std::io::{self, Read};

/// A reader that simulates reading data in small chunks from a network connection.
/// Useful for testing streaming/partial reads.
#[derive(Debug)]
pub struct ChunkReader {
    data: String,
    num_bytes_per_read: usize,
    pos: usize,
}

impl ChunkReader {
    pub fn new(data: String, num_bytes_per_read: usize) -> Self {
        ChunkReader {
            data,
            num_bytes_per_read,
            pos: 0,
        }
    }
}

impl Read for ChunkReader {
    /// Reads up to len(buf) or num_bytes_per_read bytes from the string per call.
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

#[cfg(test)]
mod test {

    use std::io::Read;

    use test_case::test_case;

    use crate::utils::ChunkReader;

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
}
