import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useRoster } from "../camp/roster";
import { useRuns } from "../camp/run";
import theme from "../../themes/three-kingdoms/ui.json";
import "../styles/council.css";

interface ConversationEvent {
  camp_id: string;
  conversation_id: string;
  role_id?: string;
  name?: string;
  round?: number;
  text?: string;
  outcome: "plan_saved" | "failed" | "stopped" | null;
  message: string | null;
}
const copy = theme.council;

export function CouncilPanel({ campId, ready, onPlan }: { campId: string; ready: boolean; onPlan: () => void }) {
  const roles = useRoster((s) => s.roles);
  const otherBusy = useRuns((s) => s.busy);
  const [selected, setSelected] = useState<string[]>([]);
  const [request, setRequest] = useState("");
  const [running, setRunning] = useState(false);
  const [listening, setListening] = useState(false);
  const [turns, setTurns] = useState<ConversationEvent[]>([]);
  const [outcome, setOutcome] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    const subscription = listen<ConversationEvent>("conversation", ({ payload }) => {
      if (!active || payload.camp_id !== campId) return;
      if (payload.role_id) setTurns((previous) => [...previous, payload]);
      if (payload.outcome) {
        setOutcome(payload.outcome);
        if (payload.message) setError(payload.message);
      }
    });
    void subscription.then(() => { if (active) setListening(true); }).catch((reason) => { if (active) setError(String(reason)); });
    return () => { active = false; void subscription.then((unlisten) => unlisten(), () => {}); };
  }, [campId]);
  const chosen = selected.filter((id) => roles.some((role) => role.id === id));
  async function start() {
    setRunning(true); setError(""); setOutcome(""); setTurns([]);
    useRuns.setState({ busy: true });
    try { await invoke("start_conversation", { campId, roleIds: chosen, request }); }
    catch (reason) { setError(String(reason)); }
    finally { setRunning(false); useRuns.setState({ busy: false }); }
  }
  async function stop() {
    try { await invoke("stop_role_run"); }
    catch (reason) { setError(String(reason)); }
  }
  return <section className="task-panel council-panel">
    <h2>{copy.title}</h2>
    <div className="council-roles" role="group" aria-label={copy.participants}>
      {roles.map((role) => <button key={role.id} type="button" aria-pressed={chosen.includes(role.id)}
        disabled={running || otherBusy || (!chosen.includes(role.id) && chosen.length >= 4)}
        onClick={() => setSelected(chosen.includes(role.id) ? chosen.filter((id) => id !== role.id) : [...chosen, role.id])}>{role.name}</button>)}
    </div>
    <p className="council-hint">{copy.hint}</p>
    <label htmlFor="council-request">{copy.request}</label>
    <textarea id="council-request" rows={2} value={request} disabled={running} onChange={(event) => setRequest(event.target.value)} placeholder={copy.placeholder} />
    <div className="task-actions">
      <button disabled={!ready || !listening || running || otherBusy || chosen.length < 2 || chosen.length > 4 || !request.trim()} onClick={() => void start()}>{running ? copy.running : copy.start}</button>
      {running && <button onClick={() => void stop()}>{copy.stop}</button>}
    </div>
    <ol className="council-transcript" aria-label={copy.transcript} aria-live="polite">
      {turns.map((turn, index) => <li key={index}><strong>{turn.name} · {copy.round.replace("{round}", String(turn.round))}</strong><p>{turn.text}</p></li>)}
    </ol>
    {outcome === "plan_saved" && <p role="status">{copy.saved} <button onClick={onPlan}>{copy.openWall}</button></p>}
    {outcome === "stopped" && <p role="status">{copy.stopped}</p>}
    {error && <p role="alert">{error}</p>}
  </section>;
}
