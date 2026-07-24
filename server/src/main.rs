fn main() {
    let config = match observed_server::ServerConfig::from_args(std::env::args()) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(if message.starts_with("observed_server") {
                0
            } else {
                2
            });
        }
    };
    let server = match observed_server::AuthoritativeServer::bind(config) {
        Ok(server) => server,
        Err(error) => {
            eprintln!("server startup failed: {error}");
            std::process::exit(1);
        }
    };
    eprintln!(
        "Observed 2 dedicated server listening on {} (Ctrl-C to stop)",
        server
            .local_addr()
            .map_or_else(|_| "unknown".to_string(), |address| address.to_string())
    );
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    if let Err(error) = server.run(stop) {
        eprintln!("server stopped with error: {error}");
        std::process::exit(1);
    }
}
