use std::{
    io::{self, BufRead, BufReader, Write},
    net::UdpSocket,
};

fn main() -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("localhost:42069")?;

    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin);

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut line = String::new();
        reader.read_line(&mut line)?;

        socket.send(line.as_bytes())?;
    }
}
