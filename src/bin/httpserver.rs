use httpfromtcp::server;
use std::process;
use std::sync::mpsc;

const PORT: u16 = 42069;

fn main() {
    // When Ctrl+C is pressed → tx.send(()) fires → rx.recv() unblocks → the
    // function returns → _server goes out of scope → Drop runs → server shuts
    // down gracefully.
    //
    // The channel is essentially being used as a signal to block until shutdown.
    // The () message carries no data — it's just a wake-up call.

    // Start server in a background thread
    let _server = match server::serve(PORT) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error starting server: {e}");
            process::exit(1);
        }
    };
    println!("Server started on port {PORT}");

    // Create a message-passing channel. tx is the sender, rx is the receiver
    let (tx, rx) = mpsc::channel();

    // Register a callback that fires when the user presses Ctrl+C. When
    // that happens, it sends an empty message () through tx.
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .expect("Error setting signal handler");

    // Block the main thread here, waiting for a message. The program just sits
    // and waits.
    rx.recv().expect("Could not receive signal");
    println!("Server gracefully stopped");
}
