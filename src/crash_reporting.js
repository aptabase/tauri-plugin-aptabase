// Added to every webview by `Builder::with_crash_reporting`: reports uncaught errors and unhandled promise rejections
// to Aptabase through the plugin's `track_error` command (the `aptabase:allow-track-error` permission).
(function () {
  var internals = window.__TAURI_INTERNALS__;
  if (!internals || typeof internals.invoke !== "function") return;

  function report(error, kind) {
    try {
      var name = "Error";
      var message;
      var stack;
      if (error instanceof Error) {
        name = error.name || "Error";
        message = error.message;
        stack = error.stack;
      } else {
        message = String(error);
      }
      // Reporting never throws or rejects: a failure here must not become an error of its own.
      Promise.resolve(
        internals.invoke("plugin:aptabase|track_error", {
          errorType: name,
          message: name + ": " + message,
          stackTrace: stack || null,
          kind: kind,
        })
      ).catch(function () {});
    } catch (_) {}
  }

  window.addEventListener("error", function (event) {
    if (!event.error) {
      // A script from another origin reports only "Script error.", without the error: nothing to act on. A
      // ResizeObserver that couldn't deliver every notification in one frame is harmless, and browsers report it often.
      var text = String(event.message || "");
      if (text === "Script error." || text.indexOf("ResizeObserver loop") !== -1) return;
    }
    report(event.error || event.message, "unhandled");
  });

  window.addEventListener("unhandledrejection", function (event) {
    report(event.reason, "taskException");
  });
})();
