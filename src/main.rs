//! # Mango Browser
//!
//! Entry point for the Mango browser application.
//! Initializes logging, creates the event loop, and launches the browser.

use mango_browser::app::MangoApp;
use mango_browser::cdp::CdpServer;
use winit::event_loop::EventLoop;

fn print_help() {
    println!("🥭 Mango Browser v{}", env!("CARGO_PKG_VERSION"));
    println!("\nUsage: mango [options] [URL]");
    println!("\nOptions:");
    println!(
        "  --remote-debugging-port=<port>  Start embedded Chrome DevTools Protocol server on <port>"
    );
    println!("  --help, -h                      Display this help message");
    println!("  --version, -v                   Display browser version");
}

fn main() {
    // Normalize locale to UTF-8 to prevent libxkbcommon fallback to legacy ISO-8859 compose files
    unsafe {
        if let Ok(lang) = std::env::var("LANG") {
            if !lang.to_lowercase().contains("utf-8") && !lang.to_lowercase().contains("utf8") {
                let normalized = format!("{}.UTF-8", lang.split('.').next().unwrap_or("en_US"));
                std::env::set_var("LANG", &normalized);
                std::env::set_var("LC_ALL", &normalized);
            }
        } else {
            std::env::set_var("LANG", "C.UTF-8");
            std::env::set_var("LC_ALL", "C.UTF-8");
        }
        if std::env::var("LC_CTYPE").is_err() {
            std::env::set_var("LC_CTYPE", "C.UTF-8");
        }
    }

    // Ignore SIGPIPE on Unix so broken pipes in Wayland, devtools websocket, or child processes
    // return an EPIPE error rather than abruptly terminating the entire browser process.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }

    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .filter_module("sctk_adwaita", log::LevelFilter::Off)
        .format_timestamp(None)
        .init();

    let args: Vec<String> = std::env::args().collect();
    let mut remote_debugging_port = None;
    let mut initial_url = None;

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--help" || arg == "-h" {
            print_help();
            return;
        } else if arg == "--version" || arg == "-v" {
            println!("Mango Browser {}", env!("CARGO_PKG_VERSION"));
            return;
        } else if let Some(port_str) = arg.strip_prefix("--remote-debugging-port=") {
            if let Ok(port) = port_str.parse::<u16>() {
                remote_debugging_port = Some(port);
            }
        } else if arg == "--remote-debugging-port" {
            if i + 1 < args.len()
                && let Ok(port) = args[i + 1].parse::<u16>()
            {
                remote_debugging_port = Some(port);
                i += 1;
            }
        } else if !arg.starts_with("--") && initial_url.is_none() {
            initial_url = Some(arg.clone());
        }
        i += 1;
    }

    log::info!("🥭 Mango Browser v{}", env!("CARGO_PKG_VERSION"));

    let _cdp_server = if let Some(port) = remote_debugging_port {
        match CdpServer::start(port) {
            Ok(server) => {
                log::info!(
                    "DevTools listening on ws://127.0.0.1:{}/devtools/browser",
                    server.port()
                );
                Some(server)
            }
            Err(e) => {
                log::error!("Failed to start CDP server on port {port}: {e}");
                None
            }
        }
    } else {
        None
    };

    log::info!("Starting up...");

    // Create the winit event loop
    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    // Create and run the application
    let mut app = MangoApp::new(initial_url);
    let run_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        event_loop.run_app(&mut app)
    }));
    match run_res {
        Ok(Ok(())) => {}
        Ok(Err(err)) => log::error!("Event loop terminated with error: {err:?}"),
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                *s
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.as_str()
            } else {
                "unknown panic"
            };
            log::warn!("Event loop cleanly caught shutdown panic: {msg}");
        }
    }
}
