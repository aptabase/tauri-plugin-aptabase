![Aptabase](https://aptabase.com/og.png)

# Tauri Plugin for Aptabase

This plugin allows you to instrument your app with events that can be analyzed in Aptabase, an Open Source, Privacy-First, and Simple Analytics for Mobile, Desktop, and Web Apps.

## Install

Install the Core plugin by adding the following to your `Cargo.toml` file:

`src-tauri/Cargo.toml`

```toml
[dependencies]
tauri-plugin-aptabase = "0.4"
```

You can install the JavaScript Guest bindings using your preferred JavaScript package manager

```bash
npm add @aptabase/tauri
```

## Usage

First, you need to get your `App Key` from Aptabase, you can find it in the `Instructions` menu on the left side menu.

Then you need to register the core plugin with Tauri:

`src-tauri/src/main.rs`

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_aptabase::Builder::new("<YOUR_APP_KEY>").build()) // 👈 this is where you enter your App Key
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

You can then start sending events from Rust by importing the `tauri_plugin_aptabase::EventTracker` trait and calling the `track_event` method on `App`, `AppHandle` or `Window`. 

As an example, you can add `app_started` and `app_exited` events like this:


```rust
use tauri_plugin_aptabase::EventTracker;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_aptabase::init("<YOUR_APP_KEY>".into()))
        .setup(|app| {
            app.track_event("app_started", None);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|handler, event| match event {
            tauri::RunEvent::Exit { .. } => {
                handler.track_event("app_exited", None);
                handler.flush_events_blocking();
            }
            _ => {}
        })
}
```

The `trackEvent` function is also available through the JavaScript guest bindings:

```js
import { trackEvent } from "@aptabase/tauri";

trackEvent("save_settings") // An event with no properties
trackEvent("screen_view", { name: "Settings" }) // An event with a custom property
```

A few important notes:

1. The plugin will automatically enhance the event with some useful information, like the OS, the app version, and other things.
2. You're in control of what gets sent to Aptabase. This plugin does not automatically track any events, you need to call `trackEvent` manually.
    - Because of this, it's generally recommended to at least track an event at startup
3. You do not need to await for the `trackEvent` function, it'll run in the background.
3. Only strings and numbers values are allowed on custom properties
