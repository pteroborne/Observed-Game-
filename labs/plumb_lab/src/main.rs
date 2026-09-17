fn main() {
    if std::env::args().any(|arg| arg == "--report") {
        if !plumb_lab::report::run() {
            std::process::exit(1);
        }
        return;
    }
    plumb_lab::run();
}
