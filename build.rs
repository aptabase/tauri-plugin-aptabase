const COMMANDS: &[&str] = &["track_event", "track_error"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
