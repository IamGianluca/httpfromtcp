use std::io::{self, Write};

#[derive(Debug)]
pub enum StatusCode {
    Okay = 200,
    ClientError = 400,
    ServerError = 500,
}

pub fn write_status_line(w: &mut impl Write, status_code: StatusCode) -> io::Result<()> {
    let out = match status_code {
        StatusCode::Okay => "HTTP/1.1 200 OK\r\n",
        StatusCode::ClientError => "HTTP/1.1 400 Bad Request\r\n",
        StatusCode::ServerError => "HTTP/1.1 500 Internal Server Error\r\n",
    };
    w.write_all(out.as_bytes())?;
    Ok(())
}
