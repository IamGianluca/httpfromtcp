use httpfromtcp::server;
use std::process;
use std::sync::mpsc;

const PORT: u16 = 42069;

fn main() {
    let _server = match server::serve(PORT) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error starting server: {e}");
            process::exit(1);
        }
    };
    println!("Server started on port {PORT}");

    let (tx, rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .expect("Error setting signal handler");

    rx.recv().expect("Could not receive signal");
    println!("Server gracefully stopped");
}
