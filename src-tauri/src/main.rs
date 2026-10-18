// Hide the console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Elevated sensor helper, started by the "Systana Sensors" scheduled task.
    #[cfg(windows)]
    {
        let args: Vec<String> = std::env::args().collect();
        if args.iter().any(|a| a == "--sensors") {
            std::process::exit(systana_lib::run_sensor_helper(args.iter().any(|a| a == "--once")));
        }
    }
    systana_lib::run()
}
