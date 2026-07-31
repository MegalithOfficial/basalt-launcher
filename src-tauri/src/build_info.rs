pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const CHANNEL: &str = env!("BASALT_BUILD_CHANNEL");
pub const DEV_BUILD: &str = env!("BASALT_DEV_BUILD");

pub fn display_version() -> String {
    if CHANNEL == "dev" {
        format!("{VERSION}-dev.{DEV_BUILD}")
    } else {
        VERSION.to_string()
    }
}
