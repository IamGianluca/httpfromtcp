use std::{
    fs::File,
    io::{self, BufReader, Read},
    sync::mpsc::{Receiver, channel},
    thread,
};

fn get_lines_channel(mut reader: BufReader<File>) -> Receiver<String> {
    let (sender, receiver) = channel();
    thread::spawn(move || {
        let mut buffer = [0u8; 8];
        let mut current_line = String::new();
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buffer[..n]);
                    let chunk = chunk.to_string();

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

    receiver // Return immediately!
}

fn main() -> io::Result<()> {
    let file = File::open("messages.txt")?;
    let reader = BufReader::new(file);

    let receiver = get_lines_channel(reader);
    for line in receiver {
        println!("read: {}", line);
    }

    Ok(())
}
