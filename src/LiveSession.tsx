import { useEffect, useRef, useState } from "react";
import {
  Activity,
  ArrowRight,
  Clock3,
  FolderOpen,
  Gem,
  LoaderCircle,
  Plug,
  Power,
  Sparkles,
  Swords,
  Users,
  MapPin,
} from "lucide-react";
import { call, desktop, folder } from "./api";
import type { Points } from "./types";
type Modifiers = {
  xp: number;
  drop: number;
  density: number;
  cooldown: number;
  zeroCooldown: boolean;
  enabled: boolean;
  preventBroken: boolean;
};
type Status = {
  modifiers?: Modifiers;
  sessionId?: string;
  state: string;
  error?: string;
  frames?: number;
  playerCookie?: string;
  characterName?: string;
  playerId?: string;
  difficulty?: number;
  portalTotal?: number;
  portalKnown?: number[];
  portalsAdded?: number;
  portalSaveRequested?: boolean;
  points?: Points;
  totalDevotion?: number;
  xpHits?: number;
  lootHits?: number;
  equipmentHits?: number;
  equipmentCreated?: number;
  equipmentBroken?: number;
  equipmentUsable?: number;
  protectedRolls?: number;
  lastEquipmentBase?: number;
  lastEquipmentResult?: number;
  densityHits?: number;
  cooldownHits?: number;
  acknowledged?: number;
  commandResult?: number;
  restored?: boolean;
  legacyProfileInstalled?: boolean;
  lastXpBase?: number;
  lastXpResult?: number;
  lastDensityBase?: number;
  lastDensityResult?: number;
  lastLootBase?: number;
  lastLootResult?: number;
  lastCooldownBaseMs?: number;
  lastCooldownResultMs?: number;
};
const original: Modifiers = {
  xp: 1,
  drop: 1,
  density: 1,
  cooldown: 0,
  zeroCooldown: false,
  enabled: false,
  preventBroken: true,
};
export default function LiveSession({
  game,
  onNotice,
}: {
  game: string;
  onNotice: (s: string) => void;
}) {
  const [status, setStatus] = useState<Status>({ state: "disconnected" });
  const [mod, setMod] = useState<Modifiers>(original);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState("");
  const [runner, setRunner] = useState({ wine: "", prefix: "" });
  const [grant, setGrant] = useState({
    skill: "0",
    attribute: "0",
    devotion: "0",
  });
  const [pendingAction, setPendingAction] = useState<{
    sequence: number;
    kind: "points" | "portals";
    characterName: string;
    difficulty: number;
  } | null>(null);
  const ready = status.state === "ready";
  const connected = ready || status.state === "connecting";
  const stopping = useRef(false);
  const operation = useRef(0);
  const syncedSession = useRef("");
  const latest = useRef(mod);
  latest.current = mod;
  useEffect(() => {
    if (!desktop) return;
    let dead = false,
      inFlight = false;
    const poll = async () => {
      if (inFlight) return;
      inFlight = true;
      try {
        const s = await call<Status>("live_poll");
        if (!dead) {
          setStatus(s);
          if (s.state === "disconnected" || s.state === "error") {
            setMod(original);
            setPendingAction(null);
            syncedSession.current = "";
            if (s.error) setError(s.error);
          } else if (
            s.state === "ready" &&
            s.sessionId &&
            syncedSession.current !== s.sessionId
          ) {
            syncedSession.current = s.sessionId;
            setMod(s.modifiers || original);
          }
        }
      } catch (e) {
        if (!dead) setError(String(e));
      } finally {
        inFlight = false;
      }
    };
    void poll();
    const timer = setInterval(() => void poll(), 500);
    return () => {
      dead = true;
      clearInterval(timer);
    };
  }, []);
  useEffect(() => {
    if (!ready || !mod.enabled || stopping.current) return;
    const generation = operation.current;
    const timer = setTimeout(() => {
      if (generation !== operation.current) return;
      void call("live_set", { modifiers: mod }).catch((e) => {
        if (generation === operation.current) {
          setError(String(e));
          setMod((m) => ({ ...m, enabled: false }));
        }
      });
    }, 180);
    return () => clearTimeout(timer);
  }, [mod, ready]);
  useEffect(() => {
    if (!pendingAction || status.acknowledged !== pendingAction.sequence)
      return;
    setPendingAction(null);
    if (status.commandResult === 0) {
      if (pendingAction.kind === "points") {
        setGrant({ skill: "0", attribute: "0", devotion: "0" });
        onNotice(`Points added to ${pendingAction.characterName}.`);
      } else {
        onNotice(
          status.portalsAdded
            ? `Unlocked ${status.portalsAdded} riftgates for ${pendingAction.characterName} on ${difficultyName(pendingAction.difficulty)}. ${status.portalSaveRequested ? "Game save requested." : "Save normally to keep this change."}`
            : "All riftgates were already unlocked on this difficulty.",
        );
      }
    } else {
      const errors: Record<number, string> = {
        1: "The loaded character changed. Try again with the current character.",
        2: "The point grant exceeds the character limit.",
        3: "The runtime did not recognize this action. Reconnect to the game.",
        4: "The difficulty changed. Check the current difficulty and try again.",
        5: "Riftgate data is not ready. Enter the campaign world and try again.",
        6: "The portal changes could not all be verified. Check the portal map before retrying.",
      };
      setError(
        errors[status.commandResult || 0] || "The character action failed.",
      );
    }
  }, [
    pendingAction,
    status.acknowledged,
    status.commandResult,
    status.portalsAdded,
    status.portalSaveRequested,
    status.difficulty,
    onNotice,
  ]);
  async function act(label: string, fn: () => Promise<void>) {
    if (busy) return;
    setBusy(label);
    setError("");
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy("");
    }
  }
  async function connect() {
    await act("Connecting", async () => {
      operation.current++;
      setMod(original);
      setStatus(
        await call<Status>("live_connect", {
          game,
          runner: { wine: runner.wine || null, prefix: runner.prefix || null },
        }),
      );
    });
  }
  async function disconnect() {
    operation.current++;
    stopping.current = true;
    setMod(original);
    await act("Restoring original settings", async () => {
      const s = await call<Status>("live_disconnect");
      setStatus(s);
      onNotice(
        s.restored
          ? "Disconnected. Original game functions restored; earned progress is kept."
          : "Disconnected. The game-side watchdog resets modifiers within two seconds.",
      );
    });
    stopping.current = false;
  }
  async function reset() {
    operation.current++;
    stopping.current = true;
    setMod(original);
    await act("Resetting modifiers", async () => {
      await call("live_set", { modifiers: original });
      onNotice("All live multipliers reset to their original values.");
    });
    stopping.current = false;
  }
  async function apply() {
    await act("Applying live settings", async () => {
      const next = { ...latest.current, enabled: true };
      await call("live_set", { modifiers: next });
      setMod(next);
    });
  }
  const values = {
    skill: Number(grant.skill),
    attribute: Number(grant.attribute),
    devotion: Number(grant.devotion),
  };
  const validGrant =
    Object.values(grant).every((v) => /^\d+$/.test(v) && Number(v) <= 1000) &&
    Object.values(values).some((v) => v > 0);
  return (
    <div className="live-session">
      <div className="profilebar">
        <div className="profile-title">
          <Activity size={19} />
          <strong>
            {mod.enabled && ready
              ? "Live modifiers active"
              : ready
                ? "Connected · Original settings"
                : status.state === "connecting"
                  ? "Connecting to game…"
                  : "Session disconnected"}
          </strong>
        </div>
        <div className="profile-actions">
          {connected ? (
            <button
              className="button"
              disabled={!!busy}
              onClick={() => void disconnect()}
            >
              <Power size={16} />
              Disconnect & reset
            </button>
          ) : (
            <button
              className="button primary"
              disabled={!desktop || !game || !!busy}
              onClick={() => void connect()}
            >
              <Plug size={16} />
              Connect to running game
            </button>
          )}
        </div>
      </div>
      {ready && (
        <p className="session-character">
          {status.playerCookie && status.playerCookie !== "0"
            ? `${status.characterName || "Character loaded"} · ${difficultyName(status.difficulty)}`
            : "Enter the playable world to use character actions."}
        </p>
      )}
      {error && (
        <div className="banner error" role="alert">
          {error}
        </div>
      )}
      {busy && (
        <div className="banner" role="status">
          <LoaderCircle size={16} className="spin" />
          {busy}…
        </div>
      )}
      {!game && desktop && (
        <div className="banner">
          Select your installation in Game files first.
        </div>
      )}
      <fieldset
        className="work-content"
        disabled={!!busy || pendingAction !== null}
      >
        <div className="controls-grid">
          <LiveControl
            title="Experience"
            label="Monster XP multiplier"
            icon={Sparkles}
            value={mod.xp}
            min={1}
            max={10}
            step={0.5}
            unit="×"
            onChange={(xp) => setMod((m) => ({ ...m, xp }))}
            note="Changes XP awards in the running game. XP you earn stays saved."
          />
          <LiveControl
            title="Loot probability"
            label="Monster loot chance"
            icon={Gem}
            value={mod.drop}
            min={1}
            max={10}
            step={0.5}
            unit="×"
            onChange={(drop) => setMod((m) => ({ ...m, drop }))}
            note="Boosts equipment chances for newly spawned monsters and miscellaneous drops on death. Chances cap at 100%; higher rarity is not guaranteed."
          >
            <label className="check-row">
              <input
                type="checkbox"
                checked={mod.preventBroken}
                disabled={mod.drop === 1}
                onChange={(e) =>
                  setMod((m) => ({ ...m, preventBroken: e.target.checked }))
                }
              />
              <span>Prevent broken equipment · applies above 1×</span>
            </label>
          </LiveControl>
          <LiveControl
            title="Monster density"
            label="Ordinary spawn multiplier"
            icon={Swords}
            value={mod.density}
            min={1}
            max={5}
            step={0.5}
            unit="×"
            onChange={(density) => setMod((m) => ({ ...m, density }))}
            note="Affects new ordinary spawn calculations. Already spawned monsters remain when settings reset."
          />
          <LiveControl
            title="Skill cooldown"
            label="Cooldown reduction"
            icon={Clock3}
            value={mod.cooldown}
            min={0}
            max={90}
            step={5}
            unit="%"
            disabled={mod.zeroCooldown}
            onChange={(cooldown) => setMod((m) => ({ ...m, cooldown }))}
            note="Changes cooldowns for the loaded player's learned skills on their next use."
          >
            <label className="check-row">
              <input
                type="checkbox"
                checked={mod.zeroCooldown}
                onChange={(e) =>
                  setMod((m) => ({ ...m, zeroCooldown: e.target.checked }))
                }
              />
              <span>Zero cooldown</span>
            </label>
          </LiveControl>
        </div>
        <div className="actionbar">
          <button
            className="button subtle"
            disabled={!ready}
            onClick={() => void reset()}
          >
            Reset all to original
          </button>
          <button
            className="button primary"
            disabled={!ready || !!status.legacyProfileInstalled}
            onClick={() => void apply()}
          >
            {mod.enabled ? "Live updates enabled" : "Apply live settings"}
            <ArrowRight size={16} />
          </button>
        </div>
        {ready && (
          <details className="diagnostics">
            <summary>Calculation details</summary>
            <div className="live-evidence">
              <span>
                {status.playerCookie && status.playerCookie !== "0"
                  ? "Character loaded"
                  : "Enter the world: press Start, then move and attack"}
              </span>
              <span>
                Observed game calculations: XP {status.xpHits || 0} · Misc loot{" "}
                {status.lootHits || 0} · Equipment {status.equipmentHits || 0} ·
                Spawns {status.densityHits || 0} · Cooldowns{" "}
                {status.cooldownHits || 0}
              </span>
              {!!status.xpHits && (
                <span>
                  Last XP award: {status.lastXpBase} → {status.lastXpResult}
                </span>
              )}
              {!!status.lootHits && (
                <span>
                  Last drop chance: {status.lastLootBase}% →{" "}
                  {status.lastLootResult}%
                </span>
              )}
              {!!status.equipmentCreated && (
                <span>
                  {!!status.equipmentHits && (
                    <>
                      Last equipment chance: {status.lastEquipmentBase}% →{" "}
                      {status.lastEquipmentResult}%{" · "}
                    </>
                  )}
                  {status.equipmentUsable || 0} non-broken /{" "}
                  {status.equipmentBroken || 0} broken items generated
                </span>
              )}
              {!!status.cooldownHits && (
                <span>
                  Last cooldown: {(status.lastCooldownBaseMs || 0) / 1000}s →{" "}
                  {(status.lastCooldownResultMs || 0) / 1000}s
                </span>
              )}
            </div>
          </details>
        )}
        <section className="panel settings-panel live-points">
          <div className="section-heading">
            <Users size={19} />
            <h2>One-time character actions</h2>
          </div>
          <p className="muted">
            {status.characterName || "Load a character"} ·{" "}
            {difficultyName(status.difficulty)}
            {" · "}These changes stay with your saved progress.
          </p>
          <div className="point-grid">
            {(["skill", "attribute", "devotion"] as const).map((k) => (
              <div className="path-field" key={k}>
                <label htmlFor={`live-grant-${k}`}>
                  {k[0].toUpperCase() + k.slice(1)} points{" "}
                  <span className="muted">
                    · Current {status.points?.[k] ?? "—"}
                  </span>
                </label>
                <input
                  id={`live-grant-${k}`}
                  type="number"
                  min={0}
                  max={1000}
                  step={1}
                  value={grant[k]}
                  onChange={(e) =>
                    setGrant((g) => ({ ...g, [k]: e.target.value }))
                  }
                />
              </div>
            ))}
          </div>
          <div className="actionbar">
            <span className="muted">
              Devotion cap remains the game's normal limit.
            </span>
            <button
              className="button"
              disabled={
                !ready ||
                !status.playerCookie ||
                status.playerCookie === "0" ||
                !validGrant ||
                pendingAction !== null
              }
              onClick={() =>
                void act("Sending point grant", async () => {
                  const n = await call<number>("live_grant", {
                    player: status.playerCookie,
                    points: values,
                  });
                  setPendingAction({
                    sequence: n,
                    kind: "points",
                    characterName: status.characterName || "the character",
                    difficulty: status.difficulty ?? -1,
                  });
                })
              }
            >
              Grant points
              <ArrowRight size={15} />
            </button>
          </div>
          <div className="portal-action">
            <div>
              <strong>
                <MapPin size={17} />
                Riftgates
              </strong>
              <p className="muted">
                {status.portalTotal
                  ? `${status.portalKnown?.[status.difficulty ?? 0] || 0} of ${status.portalTotal} unlocked`
                  : "Enter the campaign world to load riftgates"}
                {" · "}Current character and difficulty only.
              </p>
            </div>
            <button
              className="button"
              disabled={
                !ready ||
                !status.playerId ||
                !status.portalTotal ||
                status.difficulty === undefined ||
                status.difficulty < 0 ||
                !status.playerCookie ||
                status.playerCookie === "0" ||
                pendingAction !== null ||
                (status.portalKnown?.[status.difficulty] || 0) >=
                  status.portalTotal
              }
              onClick={() =>
                void act("Unlocking riftgates", async () => {
                  const sequence = await call<number>("live_unlock_portals", {
                    target: {
                      player: status.playerCookie,
                      playerId: status.playerId,
                      characterName: status.characterName,
                      sessionId: status.sessionId,
                      difficulty: status.difficulty,
                    },
                  });
                  setPendingAction({
                    sequence,
                    kind: "portals",
                    characterName: status.characterName || "the character",
                    difficulty: status.difficulty ?? -1,
                  });
                })
              }
            >
              Unlock all riftgates
              <ArrowRight size={15} />
            </button>
          </div>
        </section>
        <details className="advanced-connection">
          <summary>Advanced connection · Wine / CrossOver</summary>
          <p className="muted">
            Usually detected from the running game. On macOS, select CrossOver's
            wine command and the game's bottle folder.
          </p>
          {(["wine", "prefix"] as const).map((k) => (
            <div className="path-field" key={k}>
              <label htmlFor={`runtime-${k}`}>
                {k === "wine"
                  ? "Wine / CrossOver executable"
                  : "Wine prefix / CrossOver bottle"}
              </label>
              <div>
                <input
                  id={`runtime-${k}`}
                  value={runner[k]}
                  placeholder="Automatic"
                  onChange={(e) =>
                    setRunner((r) => ({ ...r, [k]: e.target.value }))
                  }
                />
                {k === "prefix" && (
                  <button
                    className="button"
                    disabled={!desktop || connected}
                    onClick={() =>
                      void act("Choosing prefix", async () => {
                        const p = await folder(
                          "Select the game’s Wine prefix or CrossOver bottle",
                        );
                        if (p) setRunner((r) => ({ ...r, prefix: p }));
                      })
                    }
                  >
                    <FolderOpen size={15} />
                    Browse
                  </button>
                )}
              </div>
            </div>
          ))}
        </details>
      </fieldset>
    </div>
  );
}
function LiveControl({
  title,
  label,
  icon: Icon,
  value,
  min,
  max,
  step,
  unit,
  onChange,
  note,
  disabled = false,
  children,
}: {
  title: string;
  label: string;
  icon: typeof Sparkles;
  value: number;
  min: number;
  max: number;
  step: number;
  unit: string;
  onChange: (n: number) => void;
  note: string;
  disabled?: boolean;
  children?: React.ReactNode;
}) {
  const id = "live-" + label.toLowerCase().replaceAll(/[^a-z0-9]+/g, "-");
  return (
    <section className="panel control-panel">
      <div className="control-heading">
        <span className="control-icon">
          <Icon size={21} />
        </span>
        <h2>{title}</h2>
      </div>
      <div className={"range-control " + (disabled ? "disabled" : "")}>
        <div className="range-label">
          <label htmlFor={id}>{label}</label>
          <output htmlFor={id}>
            {value}
            <span>{unit}</span>
          </output>
        </div>
        <input
          id={id}
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          disabled={disabled}
          style={
            {
              "--fill": `${((value - min) / (max - min)) * 100}%`,
            } as React.CSSProperties
          }
          onChange={(e) => onChange(Number(e.target.value))}
        />
        <div className="range-bounds">
          <span>
            {min}
            {unit} · Original
          </span>
          <span>
            {max}
            {unit}
          </span>
        </div>
      </div>
      {children}
      <p className="control-footnote">{note}</p>
    </section>
  );
}

function difficultyName(value?: number) {
  return value === 0
    ? "Normal"
    : value === 1
      ? "Elite"
      : value === 2
        ? "Ultimate"
        : "No difficulty loaded";
}
