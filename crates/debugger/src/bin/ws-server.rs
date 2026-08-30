fn main() {
    if let Err(error) = debugger::ws::serve() {
        eprintln!("ws-server failed: {error}");
        std::process::exit(1);
    }
}
