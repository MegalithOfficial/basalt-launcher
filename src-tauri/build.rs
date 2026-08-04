fn main() {
    println!("cargo:rerun-if-env-changed=BASALT_BUILD_CHANNEL");
    println!("cargo:rerun-if-env-changed=BASALT_DEV_BUILD");
    println!("cargo:rerun-if-env-changed=BASALT_DISTRIBUTION");
    println!("cargo:rerun-if-env-changed=BASALT_CURSEFORGE_API_KEY");
    println!("cargo:rerun-if-env-changed=BASALT_DISCORD_APP_ID");
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
    let distribution =
        std::env::var("BASALT_DISTRIBUTION").unwrap_or_else(|_| "source".to_string());
    println!("cargo:rustc-env=BASALT_DISTRIBUTION={distribution}");
    let curseforge_api_key = std::env::var("BASALT_CURSEFORGE_API_KEY").unwrap_or_default();
    println!("cargo:rustc-env=BASALT_CURSEFORGE_API_KEY={curseforge_api_key}");
    let discord_app_id = std::env::var("BASALT_DISCORD_APP_ID").unwrap_or_default();
    println!("cargo:rustc-env=BASALT_DISCORD_APP_ID={discord_app_id}");
    tauri_build::build()
}
