use std::{
    io::{self, BufReader, BufWriter, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use crate::{headers::Headers, request::request_from_reader, response::write_headers};

pub struct Server {
    port: String,
    listener: Option<Arc<TcpListener>>,
    pub is_closed: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>, // server state, background thread
}

impl Server {
    pub fn handle(conn: TcpStream) {
        let mut buf = BufWriter::new(conn);
        let mut headers = Headers::new();
        headers.insert("Content-Type".to_string(), "text/plain".to_string());
        headers.insert("Content-Length".to_string(), "0".to_string());
        headers.insert("Connection".to_string(), "close".to_string());
        let _ = write_headers(&mut buf, headers);
        buf.flush().unwrap();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Set is_closed to true
        self.is_closed.store(true, Ordering::SeqCst);

        // create a throwaway TCP connection to unblock incoming()
        let _ = std::net::TcpStream::connect(self.port.clone());

        // Drop the server's Arc reference to the listener. The listener won't
        // be freed yet since the thread still holds listener_clone, but this
        // cleans up the server's own reference.
        drop(self.listener.take());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn serve(port: u16) -> io::Result<Server> {
    let port = format!("127.0.0.1:{port}");
    let listener = Arc::new(TcpListener::bind(&port)?);
    let is_closed = Arc::new(AtomicBool::new(false));

    let listener_clone = Arc::clone(&listener);
    let is_closed_clone = Arc::clone(&is_closed);

    let handle = thread::spawn(move || {
        for stream in listener_clone.incoming() {
            println!("Connection accepted.");

            let server_stream = match stream {
                Ok(v) => {
                    // The throwaway connection opened by drop() will appear as
                    // a successful connection here. Check is_closed and exit
                    // the loop instead of trying to parse it as a real request.
                    if is_closed_clone.load(Ordering::SeqCst) {
                        break;
                    }
                    v
                }
                Err(_) => match is_closed_clone.load(Ordering::SeqCst) {
                    true => break,
                    false => continue,
                },
            };

            thread::spawn(move || {
                let reader = BufReader::new(&server_stream);
                let _request = request_from_reader(reader).unwrap();
                Server::handle(server_stream)
            });
        }
    });

    Ok(Server {
        port,
        listener: Some(listener),
        is_closed,
        handle: Some(handle),
    })
}

#[cfg(test)]
mod test {
    use std::{
        io::{BufReader, Read, Write},
        net::{TcpListener, TcpStream},
        sync::atomic::Ordering,
    };

    use crate::{
        request::request_from_reader,
        server::{Server, serve},
    };

    #[test]
    fn test_serve_returns_server_open_connection() {
        // Given
        let port = 8888_u16;

        // When
        let result = serve(port).unwrap();

        // Then
        assert!(!result.is_closed.load(Ordering::SeqCst));
    }

    #[test]
    fn test_server_handle_associated_function_hardcoded_response() {
        // Given
        let addr = "127.0.0.1:1942".to_string();
        let listener = TcpListener::bind(&addr).unwrap();

        let mut client_stream = TcpStream::connect(&addr).unwrap();
        let (server_stream, _addr) = listener.accept().unwrap();

        // When
        Server::handle(server_stream);

        // Then
        let mut response = String::new();
        client_stream.read_to_string(&mut response).unwrap();
        assert_eq!(
            response,
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 0\r\nConnection: close\r\n"
        );
    }

    // The following integration tests are important because, differently from
    // the tests in request.rs, we are now using a live TcpStream. A TcpStream
    // behaves differently from a buffer/byte slice. For instance, when a byte
    // slice is exhausted, read() returns 0 (EOF) immediately. A TcpStream
    // never returns 0 until the connection closes, which is a fundamentally
    // different behavior.

    #[test]
    fn test_integration_between_request_from_reader_to_server_get_request() {
        // Given
        let addr = "127.0.0.1:1112".to_string();
        let listener = TcpListener::bind(&addr).unwrap();

        let mut client_stream = TcpStream::connect(&addr).unwrap();
        let (server_stream, _addr) = listener.accept().unwrap();

        let reader = BufReader::new(&server_stream);
        client_stream.write_all(b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n").unwrap();

        // When
        let _request = request_from_reader(reader).unwrap();
        Server::handle(server_stream);

        // Then
        let mut response = String::new();
        client_stream.read_to_string(&mut response).unwrap();
        assert_eq!(
            response,
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 0\r\nConnection: close\r\n"
        );
    }

    #[test]
    fn test_integration_between_request_from_reader_to_server_post_request() {
        // Given
        let addr = "127.0.0.1:1113".to_string();
        let listener = TcpListener::bind(&addr).unwrap();

        let mut client_stream = TcpStream::connect(&addr).unwrap();
        let (server_stream, _addr) = listener.accept().unwrap();

        // ❯ curl -X POST http://localhost:42069/coffee \
        // -H 'Content-Type: application/json' \
        // -d '{"type": "dark mode", "size": "medium"}'
        let reader = BufReader::new(&server_stream);
        client_stream.write_all(b"POST /coffee HTTP/1.1\r\nHost: localhost:42069\r\nContent-Type: application/json\r\nContent-Length: 39\r\n\r\n{\"type\": \"dark mode\", \"size\": \"medium\"}").unwrap();

        // When
        let _request = request_from_reader(reader).unwrap();
        Server::handle(server_stream);

        // Then
        let mut response = String::new();
        client_stream.read_to_string(&mut response).unwrap();
        assert_eq!(
            response,
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 0\r\nConnection: close\r\n"
        );
    }
}
