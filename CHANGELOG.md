## 0.4.1

* Automatic flush of events on app exit
* Fix User-Agent header

## 0.4.0

* Events are now sent in batches to reduce network overhead
* While offline, events will be enqueue and sent when the app is back online
* Tauri 1.4 required

## 0.3.2

* (macOS) Fixed an issue where sessions could span multiple days if the app was left open overnight

## 0.3.1

* Wait for event to be flushed on panic

## 0.3.0

* Add ability for panic hook to log panics to aptabase

## 0.2.1

* Added support for automatic segregation of Debug/Release data source

## 0.2.0

* BREAKING CHANGE: replaced the `init` function with a `Builder` struct, see README for example usage
* Ability to set custom hosts for self hosted servers