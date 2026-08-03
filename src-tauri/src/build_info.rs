pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const CHANNEL: &str = env!("BASALT_BUILD_CHANNEL");
pub const DEV_BUILD: &str = env!("BASALT_DEV_BUILD");
#[cfg(target_os = "linux")]
pub const DISTRIBUTION: &str = env!("BASALT_DISTRIBUTION");
const CURSEFORGE_API_KEY: &str = env!("BASALT_CURSEFORGE_API_KEY");

pub fn bundled_curseforge_key() -> Option<&'static str> {
    Some(CURSEFORGE_API_KEY.trim()).filter(|key| !key.is_empty())
}

pub fn display_version() -> String {
    if CHANNEL == "dev" {
        format!("{VERSION}-dev.{DEV_BUILD}")
    } else {
        VERSION.to_string()
    }
}
