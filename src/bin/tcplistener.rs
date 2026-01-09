use std::{
    io::{self, BufReader},
    net::TcpListener,
};

use httpfromtcp::request::{RequestState, request_from_reader};

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:42069")?;
    loop {
        let (stream, _addr) = listener.accept()?;
        println!("Connection accepted.");

        let reader = BufReader::new(stream);
        let request = request_from_reader(reader)?;

        match request.status {
            RequestState::Done => {
                println!(
                    "Request line:\n- Method: {}\n- Target: {}\n- Version: {}",
                    request.request_line.method,
                    request.request_line.request_target,
                    request.request_line.http_version,
                );
                println!("Headers:");
                for (key, value) in &request.headers.inner {
                    println!("- {}: {}", key, value);
                }
            }
            RequestState::Initialized
            | RequestState::ParsingHeaders
            | RequestState::ParsingBody => continue,
        }

        println!("Connection closed.");
    }
}
