#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod core;
mod web;
mod models;
mod providers;
mod store;
mod custom_models;
mod workspace;
mod web_pages;
mod opex;
mod diagnostics;
mod training;
mod window;

fn main() {
    let result = run();
    if let Err(error) = result {
        eprintln!("{error}");
        window::error(&error);
        std::process::exit(1);
    }
}
fn run() -> core::Result<()> {
    let root = core::root()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("window");
    match command {
        "window" | "menu" => web::run(root, false),
        "manage" => window::run(root),
        "start" | "stop" | "status" | "monitor" => {
            let cfg = core::config(&root)?;
            let keys = if args.len() > 1 {
                args[1..].to_vec()
            } else {
                cfg["services"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .cloned()
                    .collect()
            };
            loop {
                for key in &keys {
                    if command == "status" {
                        println!(
                            "{key}: {}",
                            if core::online(&cfg, key) {
                                "bereikbaar"
                            } else {
                                "offline/loading"
                            }
                        );
                    } else {
                        println!("{}", core::service(&root, key, command != "stop")?);
                    }
                }
                if command != "monitor" {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
            Ok(())
        }
        "self-test" => web::run(root, true),
        "manager-test" => window::smoke_test(root),
        _ => Err("Gebruik: window | start [service] | stop [service] | status | monitor".into()),
    }
}
