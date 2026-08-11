#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    if let Some(supervision) = basalt_launcher_lib::supervisor_args(&arguments) {
        basalt_launcher_lib::supervise(supervision)
    }

    #[cfg(target_os = "linux")]
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    basalt_launcher_lib::run()
}
