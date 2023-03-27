use wry::webview;

#[cfg(target_os = "linux")]
static ENGINE_NAME: &str = "WebKitGTK";

#[cfg(target_os = "macos")]
static ENGINE_NAME: &str = "WebKit";

#[cfg(target_os = "windows")]
static ENGINE_NAME: &str = "WebView2";

pub struct SystemProperties {
    pub os_name: String,
    pub os_version: String,
    pub locale: String,
    pub framework_name: String,
    pub framework_version: String,
    pub engine_name: String,
    pub engine_version: String,
}

pub fn get_info() -> SystemProperties {
    let info = os_info::get();
    let locale = sys_locale::get_locale().unwrap_or_default();
    let engine_version = webview::webview_version().unwrap_or_default();

    let os_name = match info.os_type() {
        os_info::Type::Macos => "macOS".to_string(),
        os_info::Type::Windows => "Windows".to_string(),
        _ => info.os_type().to_string(),
    };

    SystemProperties {
        os_name,
        os_version: info.version().to_string(),
        locale,
        framework_name: "Tauri".to_string(),
        framework_version: "".to_string(), // TODO: depends on https://github.com/tauri-apps/tauri/pull/6546
        engine_name: ENGINE_NAME.to_string(),
        engine_version,
    }
}
