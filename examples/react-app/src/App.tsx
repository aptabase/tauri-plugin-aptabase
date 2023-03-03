import reactLogo from "./assets/react.svg";
import { trackEvent } from 'tauri-plugin-aptabase-api'
import "./App.css";

function App() {
  function clickVite() {
    trackEvent("logo_click")
  }

  function clickTauri() {
    trackEvent("logo_click")
  }

  function clickReact() {
    trackEvent("logo_click")
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
    </div>
  );
}

export default App;
