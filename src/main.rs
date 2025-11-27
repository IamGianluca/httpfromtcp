use std::{
    fs::File,
    io::{self, Read},
};

fn main() -> io::Result<()> {
    let mut file = File::open("messages.txt")?;
    let mut buffer = [0u8; 8];

    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        // buffer[..n] is necessary for scenario when buffer is less than 8 bytes. this
        // could happen when reading the last chunk of the file.
        let text = std::str::from_utf8(&buffer[..n]).unwrap_or("<invalid utf8>");
        println!("read: {}", text);
    }
    Ok(())
}
