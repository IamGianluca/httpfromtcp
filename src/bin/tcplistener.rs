use std::{
    io::{self, BufReader, Read},
    net::{TcpListener, TcpStream},
    sync::mpsc::{Receiver, channel},
    thread,
};

fn get_lines_channel(mut reader: BufReader<TcpStream>) -> Receiver<String> {
    let (sender, receiver) = channel();
    thread::spawn(move || {
        let mut buffer = [0u8; 8];
        let mut current_line = String::new();
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buffer[..n]).to_string();

                    if let Some((before_newline, after_newline)) = chunk.split_once("\n") {
                        current_line.push_str(before_newline);
                        sender.send(current_line.clone()).unwrap();
                        current_line = after_newline.to_string();
                    } else {
                        current_line.push_str(&chunk);
                    }
                }
                Err(_) => break,
            }
        }
    });
    receiver
}

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:42069")?;

    loop {
        let (stream, _addr) = listener.accept()?;
        println!("Connection accepted.");

        let reader = BufReader::new(stream);
        let receiver = get_lines_channel(reader);

        for line in receiver {
            println!("{}", line);
        }

        println!("Connection closed.");
    }
}
