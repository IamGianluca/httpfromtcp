use std::{
    fs::File,
    io::{self, BufRead, BufReader},
};

fn main() -> io::Result<()> {
    const CAPACITY: usize = 8; // 8 bytes
    let file = File::open("messages.txt").unwrap();
    let mut reader = BufReader::with_capacity(CAPACITY, file);

    loop {
        let buffer = reader.fill_buf()?;
        let length: usize = buffer.len();

        if length == 0_usize {
            break;
        }
        let text = std::str::from_utf8(buffer).unwrap_or("<invalid utf8>");
        println!("read: {}", text);
        reader.consume(length);
    }

    Ok(())
}
