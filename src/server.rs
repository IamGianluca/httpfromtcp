use std::{
    io::{self, BufReader, BufWriter, Read},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use crate::{
    headers::Headers,
    request::{Request, request_from_reader},
    response::{StatusCode, Writer},
};

use sha2::{Digest, Sha256};

pub struct Server {
    port: String,
    listener: Option<Arc<TcpListener>>,
    pub is_closed: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>, // server state, background thread
}

impl Server {
    pub fn handle(conn: TcpStream, handler: Handler) -> io::Result<()> {
        // Parse request
        let reader = BufReader::new(&conn);
        let request = match request_from_reader(reader) {
            Ok(r) => r,
            Err(e) => {
                let writer = BufWriter::new(conn);
                let headers = Headers::new();
                let mut w = Writer::new(writer);
                w.write_status_line(StatusCode::ClientError)?;
                w.write_headers(headers)?;
                let error_body = format!("{e}");
                w.write_body(error_body.as_bytes())?;
                return Ok(());
            }
        };

        // Write response
        let mut w = Writer::new(BufWriter::new(conn));
        handler(&mut w, &request)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Set is_closed to true
        self.is_closed.store(true, Ordering::SeqCst);

        // Create a throwaway TCP connection to unblock incoming()
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

pub fn handler(w: &mut Writer<BufWriter<TcpStream>>, req: &Request) -> io::Result<()> {
    // Proxy handler
    if let Some(path) = req.request_line.request_target.strip_prefix("/httpbin") {
        let url = format!("https://httpbin.org{path}");
        let mut resp = reqwest::blocking::get(url).unwrap();

        let mut headers = Headers::new();
        let content_type = resp
            .headers()
            .get("content-type") // Header names are lowercase in reqwest
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        headers.insert("Content-Type".to_string(), content_type);
        headers.insert("Transfer-Encoding".to_string(), "chunked".to_string());
        headers.insert("Connection".to_string(), "close".to_string());
        if path == "/html" {
            headers.insert(
                "Trailer".to_string(),
                "X-Content-SHA256, X-Content-Length".to_string(),
            );
        }

        let has_trailers = headers.get("trailer").is_some();
        w.write_status_line(StatusCode::Ok)?;
        w.write_headers(headers)?;

        // Stream body
        let mut buf = [0u8; 1024];
        let mut hasher = Sha256::new();
        let mut body_len = 0usize;
        loop {
            let n = resp.read(&mut buf)?;
            if n == 0 {
                break;
            }
            w.write_chunked_body(&buf[..n])?;
            if has_trailers {
                hasher.update(&buf[..n]);
                body_len += n;
            }
        }

        return match has_trailers {
            true => {
                let hash = hasher.finalize();
                let hex = hash
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>();
                let mut trailer_headers = Headers::new();
                trailer_headers.insert("X-Content-SHA256".to_string(), hex);
                trailer_headers.insert("X-Content-Length".to_string(), body_len.to_string());
                w.write_trailers(trailer_headers)?;
                Ok(())
            }
            false => {
                w.write_chunked_body_done()?;
                Ok(())
            }
        };
    } else {
        match req.request_line.request_target.as_str() {
            "/yourproblem" => {
                let body = b"<html>
  <head>
    <title>400 Bad Request</title>
  </head>
  <body>
    <h1>Bad Request</h1>
    <p>Your request honestly kinda sucked.</p>
  </body>
</html>";
                let mut headers = Headers::new();
                headers.insert("Content-Type".to_string(), "text/html".to_string());
                headers.insert("Content-Length".to_string(), body.len().to_string());
                headers.insert("Connection".to_string(), "close".to_string());
                w.write_status_line(StatusCode::ClientError)?;
                w.write_headers(headers)?;
                w.write_body(body)?;
            }
            "/myproblem" => {
                let body = b"<html>
  <head>
    <title>500 Internal Server Error</title>
  </head>
  <body>
    <h1>Internal Server Error</h1>
    <p>Okay, you know what? This one is on me.</p>
  </body>
</html>";
                let mut headers = Headers::new();
                headers.insert("Content-Type".to_string(), "text/html".to_string());
                headers.insert("Content-Length".to_string(), body.len().to_string());
                headers.insert("Connection".to_string(), "close".to_string());
                w.write_status_line(StatusCode::ServerError)?;
                w.write_headers(headers)?;
                w.write_body(body)?;
            }
            "/video" => {
                let body = std::fs::read("./assets/vim.mp4")?;
                let mut headers = Headers::new();
                headers.insert("Content-Type".to_string(), "video/mp4".to_string());
                headers.insert("Content-Length".to_string(), body.len().to_string());
                headers.insert("Connection".to_string(), "close".to_string());
                w.write_status_line(StatusCode::Ok)?;
                w.write_headers(headers)?;
                w.write_body(&body)?;
            }
            _ => {
                let body = b"<html>
  <head>
    <title>200 OK</title>
  </head>
  <body>
    <h1>Success!</h1>
    <p>Your request was an absolute banger.</p>
  </body>
</html>";
                let mut headers = Headers::new();
                headers.insert("Content-Type".to_string(), "text/html".to_string());
                headers.insert("Content-Length".to_string(), body.len().to_string());
                headers.insert("Connection".to_string(), "close".to_string());
                w.write_status_line(StatusCode::Ok)?;
                w.write_headers(headers)?;
                w.write_body(body)?;
            }
        }
    }
    Ok(())
}

type Handler = fn(&mut Writer<BufWriter<TcpStream>>, &Request) -> io::Result<()>;

pub fn serve(port: u16, handler: Handler) -> io::Result<Server> {
    let port = format!("127.0.0.1:{port}");
    let listener = Arc::new(TcpListener::bind(&port)?);
    let is_closed = Arc::new(AtomicBool::new(false));

    // Create shallow copies to use these objects both in the loop and later in the
    // Server struct. Arc allows shared ownership across threads via reference counting.
    let listener_clone = Arc::clone(&listener);
    let is_closed_clone = Arc::clone(&is_closed);

    // Process each request in a separate thread
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
                let _ = Server::handle(server_stream, handler);
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
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::atomic::Ordering,
    };

    use crate::server::{Server, handler, serve};

    #[test]
    fn test_serve_returns_server_open_connection() {
        // Given
        let port = 8888_u16;

        // When
        let result = serve(port, handler).unwrap();

        // Then
        assert!(!result.is_closed.load(Ordering::SeqCst));
    }

    // The following integration tests are important because, differently from
    // the tests in request.rs, we are now using a live TcpStream. A TcpStream
    // behaves differently from a buffer/byte slice. For instance, when a byte
    // slice is exhausted, read() returns 0 (EOF) immediately. A TcpStream
    // never returns 0 until the connection closes, which is a fundamentally
    // different behavior.

    #[test]
    fn test_get_request_returns_200() {
        // Given
        let addr = "127.0.0.1:1112".to_string();
        let listener = TcpListener::bind(&addr).unwrap();

        let mut client_stream = TcpStream::connect(&addr).unwrap();
        let (server_stream, _addr) = listener.accept().unwrap();

        client_stream.write_all(b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n").unwrap();

        // When
        Server::handle(server_stream, handler).unwrap();

        // Then
        let mut response = String::new();
        client_stream.read_to_string(&mut response).unwrap();
        assert_eq!(
            response,
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 148\r\nConnection: close\r\n\r\n<html>\n  <head>\n    <title>200 OK</title>\n  </head>\n  <body>\n    <h1>Success!</h1>\n    <p>Your request was an absolute banger.</p>\n  </body>\n</html>"
        );
    }

    #[test]
    fn test_post_request_returns_200() {
        // Given
        let addr = "127.0.0.1:1113".to_string();
        let listener = TcpListener::bind(&addr).unwrap();

        let mut client_stream = TcpStream::connect(&addr).unwrap();
        let (server_stream, _addr) = listener.accept().unwrap();

        client_stream.write_all(b"POST /coffee HTTP/1.1\r\nHost: localhost:42069\r\nContent-Type: application/json\r\nContent-Length: 39\r\n\r\n{\"type\": \"dark mode\", \"size\": \"medium\"}").unwrap();

        // When
        Server::handle(server_stream, handler).unwrap();

        // Then
        let mut response = String::new();
        client_stream.read_to_string(&mut response).unwrap();
        assert_eq!(
            response,
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 148\r\nConnection: close\r\n\r\n<html>\n  <head>\n    <title>200 OK</title>\n  </head>\n  <body>\n    <h1>Success!</h1>\n    <p>Your request was an absolute banger.</p>\n  </body>\n</html>"
        );
    }

    #[test]
    fn test_handler_error_returns_500() {
        // Given
        let addr = "127.0.0.1:8210".to_string();
        let listener = TcpListener::bind(&addr).unwrap();

        let mut client_stream = TcpStream::connect(&addr).unwrap();
        let (server_stream, _addr) = listener.accept().unwrap();

        client_stream.write_all(b"POST /myproblem HTTP/1.1\r\nHost: localhost:42069\r\nContent-Type: application/json\r\nContent-Length: 39\r\n\r\n{\"type\": \"dark mode\", \"size\": \"medium\"}").unwrap();

        // When
        Server::handle(server_stream, handler).unwrap();

        // Then
        let mut response = String::new();
        client_stream.read_to_string(&mut response).unwrap();
        assert_eq!(
            response,
            "HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/html\r\nContent-Length: 183\r\nConnection: close\r\n\r\n<html>\n  <head>\n    <title>500 Internal Server Error</title>\n  </head>\n  <body>\n    <h1>Internal Server Error</h1>\n    <p>Okay, you know what? This one is on me.</p>\n  </body>\n</html>"
        );
    }

    #[test]
    fn test_malformed_request_returns_400() {
        // Given
        let addr = "127.0.0.1:8211".to_string();
        let listener = TcpListener::bind(&addr).unwrap();

        let mut client_stream = TcpStream::connect(&addr).unwrap();
        let (server_stream, _addr) = listener.accept().unwrap();

        client_stream.write_all(b"BADREQUEST\r\n\r\n").unwrap();

        // When
        Server::handle(server_stream, handler).unwrap();

        // Then
        let mut response = String::new();
        client_stream.read_to_string(&mut response).unwrap();
        assert_eq!(
            response,
            "HTTP/1.1 400 Bad Request\r\n\r\nmore than 3 elements to parse"
        );
    }
}
