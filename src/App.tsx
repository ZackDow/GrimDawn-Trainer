import { useEffect, useState } from "react";
import { FolderOpen, RefreshCw } from "lucide-react";
import { call, desktop, folder } from "./api";
import LiveSession from "./LiveSession";
import type { Home } from "./types";

export default function App() {
  const [home, setHome] = useState<Home | null>(null);
  const [gamePath, setGamePath] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const game = home?.game || "";
  async function run(fn: () => Promise<Home>) {
    setBusy(true);
    setError("");
    try {
      const next = await fn();
      setHome(next);
      setGamePath(next.game || "");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  useEffect(() => {
    void run(() => call<Home>("home"));
  }, []);
  return (
    <main className="app">
      <header className="app-header">
        <div className="brand">
          <span className="brand-rune">G</span>
          <div>
            <h1>Grim Dawn Trainer</h1>
            <p>Live controls. Keep your progress.</p>
          </div>
        </div>
        <span className="version">
          v0.3.1{!desktop && " · Interface preview"}
        </span>
      </header>
      {!desktop && (
        <div className="banner">
          Interface preview. Open the desktop app to connect to Grim Dawn.
        </div>
      )}
      {error && (
        <div className="banner error" role="alert">
          {error}
        </div>
      )}
      {notice && (
        <div className="banner success" role="status">
          {notice}
        </div>
      )}
      <details className="connection panel" open={!game && desktop}>
        <summary>
          Game connection{" "}
          <span>{game ? "Folder selected" : "Choose your installation"}</span>
        </summary>
        <div className="path-field">
          <label htmlFor="game-path">Grim Dawn folder</label>
          <div>
            <input
              id="game-path"
              value={gamePath}
              placeholder="Detected automatically when available"
              onChange={(e) => setGamePath(e.target.value)}
              disabled={!desktop || busy}
            />
            <button
              className="button"
              disabled={!desktop || busy}
              onClick={() =>
                void (async () => {
                  const path = await folder(
                    "Select the Grim Dawn installation",
                  );
                  if (path)
                    await run(() => call<Home>("configure", { game: path }));
                })()
              }
            >
              <FolderOpen size={16} />
              Browse
            </button>
            <button
              className="button"
              disabled={!desktop || busy || !gamePath || gamePath === game}
              onClick={() =>
                void run(() => call<Home>("configure", { game: gamePath }))
              }
            >
              Use folder
            </button>
          </div>
        </div>
        <button
          className="button subtle"
          disabled={!desktop || busy}
          onClick={() => void run(() => call<Home>("home"))}
        >
          <RefreshCw size={15} />
          Scan again
        </button>
      </details>
      <LiveSession game={game} onNotice={setNotice} />
      <footer>
        Closing the trainer resets live modifiers. Points and portal unlocks are
        saved character progress.
      </footer>
    </main>
  );
}
