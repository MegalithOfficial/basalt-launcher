fn main() {
    println!("cargo:rerun-if-env-changed=BASALT_BUILD_CHANNEL");
    let channel = std::env::var("BASALT_BUILD_CHANNEL").unwrap_or_else(|_| "dev".to_string());
    match channel.as_str() {
        "dev" | "release" => println!("cargo:rustc-env=BASALT_BUILD_CHANNEL={channel}"),
        _ => panic!("BASALT_BUILD_CHANNEL must be dev or release"),
    }
    tauri_build::build()
}
