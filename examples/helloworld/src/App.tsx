import reactLogo from "./assets/react.svg";
import { trackEvent } from '@aptabase/tauri'
import "./App.css";
import { invoke } from "@tauri-apps/api";

function App() {
  function clickVite() {
    trackEvent("logo_click", { "logo": "vite", count: 1 })
  }

  function clickTauri() {
    trackEvent("logo_click", { "logo": "tauri", count: 1 })
  }

  function clickReact() {
    trackEvent("logo_click", { "logo": "react", count: 1 })
  }

  function panic() {
    invoke("this_will_panic")
  }

  return (
    <div className="container">
      <h1>Welcome to Tauri + Aptabase Example!</h1>

      <div className="row">
        <button onClick={clickVite}>
          <img src="/vite.svg" className="logo vite" alt="Vite logo" />
        </button>
        <button onClick={clickTauri}>
          <img src="/tauri.svg" className="logo tauri" alt="Tauri logo" />
        </button>
        <button onClick={clickReact}>
          <img src={reactLogo} className="logo react" alt="React logo" />
        </button>
      </div>

      <p>Click on the Tauri, Vite, and React logos to trigger an event.</p>

      <div>
        <button onClick={panic}>PANIC!</button>
      </div>
    </div>
  );
}

export default App;
