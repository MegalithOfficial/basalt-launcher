fn main() {
    println!("cargo:rerun-if-env-changed=BASALT_BUILD_CHANNEL");
    println!("cargo:rerun-if-env-changed=BASALT_DEV_BUILD");
    println!("cargo:rerun-if-env-changed=GITHUB_RUN_NUMBER");
    println!("cargo:rerun-if-env-changed=GITHUB_RUN_ATTEMPT");
    let channel = std::env::var("BASALT_BUILD_CHANNEL").unwrap_or_else(|_| "dev".to_string());
    match channel.as_str() {
        "dev" | "release" => println!("cargo:rustc-env=BASALT_BUILD_CHANNEL={channel}"),
        _ => panic!("BASALT_BUILD_CHANNEL must be dev or release"),
    }
    let dev_build = std::env::var("BASALT_DEV_BUILD").unwrap_or_else(|_| {
        let run = std::env::var("GITHUB_RUN_NUMBER").unwrap_or_else(|_| "0".to_string());
        let attempt = std::env::var("GITHUB_RUN_ATTEMPT").unwrap_or_else(|_| "0".to_string());
        format!("{run}.{attempt}")
    });
    println!("cargo:rustc-env=BASALT_DEV_BUILD={dev_build}");
    tauri_build::build()
}
