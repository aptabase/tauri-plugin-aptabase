<script>
  import { invoke } from "@tauri-apps/api/core"
  import { trackEvent, trackError } from '@aptabase/tauri'

  let name = "";
  let greetMsg = ""

  async function greet(){
    trackEvent("greet", { name })
    // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
    greetMsg = await invoke("greet", { name })
  }

  function reportError() {
    try {
      JSON.parse("{ not json")
    } catch (error) {
      trackError(error)
    }
  }

  function panicInRust() {
    // A sync command runs on the main thread: with crash reporting on, the panic is reported as a crash.
    invoke("this_will_panic")
  }
</script>

<div>
  <div class="row">
    <input id="greet-input" placeholder="Enter a name..." bind:value={name} />
    <button on:click={greet}>
      Greet
    </button>
  </div>
  <p>{greetMsg}</p>
  <div class="row">
    <button on:click={reportError}>Report an error</button>
    <button on:click={panicInRust}>Panic in Rust</button>
  </div>
</div>

