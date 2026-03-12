use std::{
    io::{self, BufReader},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use crate::request::request_from_reader;

pub struct Server {
    port: String,
    listener: Option<Arc<TcpListener>>,
    pub is_closed: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>, // server state, background thread
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

            let s = match stream {
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
                let reader = BufReader::new(&s);
                let _request = request_from_reader(reader).unwrap();
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
    use std::sync::atomic::Ordering;

    use crate::server::serve;

    #[test]
    fn test_something() {
        // Given
        let port = 8888_u16;

        // When
        let result = serve(port).unwrap();

        // Then
        assert_eq!(result.is_closed.load(Ordering::SeqCst), false);
    }
}
