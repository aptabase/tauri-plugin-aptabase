<img src="https://aptabase.com/og.png" alt="Aptabase"/>

# Tauri Plugin for Aptabase

This plugin allows you to instrument your app with events that can be analyzed in Aptabase, a privacy-first analytics platform for Desktop and Mobile apps.

## Install

Install the Core plugin by adding the following to your `Cargo.toml` file:

`src-tauri/Cargo.toml`

```toml
[dependencies]
tauri-plugin-aptabase = "0.1"
```

You can install the JavaScript Guest bindings using your preferred JavaScript package manager

```bash
pnpm add @aptabase/tauri
# or
npm add @aptabase/tauri
# or
yarn add @aptabase/tauri
```

## Usage

First you need to get your `App Key` from Aptabase, you can find it in the `Instructions` menu on the left side menu.

Then you need to register the core plugin with Tauri:

`src-tauri/src/main.rs`

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_aptabase::init("<YOUR_APP_KEY>".into())) // 👈 this is where you enter your App Key
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Afterwards all the plugin's APIs are available through the JavaScript guest bindings:

```js
import { trackEvent } from "@aptabase/tauri";

trackEvent("connect_click") // An event with no properties
trackEvent("play_music", { name: "Here comes the sun" }) // An event with a custom property
```

A few important notes:

1. The plugin will automatically enhance the event with some useful information, like the OS, the app version, and other things.
2. You're in control of what gets sent to Aptabase. This plugin does not automatically track any events, you need to call `trackEvent` manually.
    - Because of this, it's generally recommended to at least track an event at startup
3. You do not need to await the `trackEvent` function, it'll run in the background.
3. Only strings and numbers values are allowed on custom properties
