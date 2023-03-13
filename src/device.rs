#[cfg(target_os = "macos")]
static OS_FAMILY: &str = "macOS";
#[cfg(target_os = "linux")]
static OS_FAMILY: &str = "Linux";
#[cfg(target_os = "windows")]
static OS_FAMILY: &str = "Windows";


pub struct Device {
    pub identifier: String,
    pub os_family: String,
    pub os_name: String,
    pub os_version: String,
    pub os_locale: String,
}

pub fn info() -> Device {
    let info = os_info::get();
    let os_locale = sys_locale::get_locale().unwrap_or_default();

    let os_name = match info.os_type() {
        os_info::Type::Macos => "macOS".to_string(),
        os_info::Type::Windows => "Windows".to_string(),
        _ => info.os_type().to_string(),
    };

    Device {
        identifier: machine_uid::get().unwrap(),
        os_family: OS_FAMILY.to_string(),
        os_name,
        os_version: info.version().to_string(),
        os_locale,
    }
}
