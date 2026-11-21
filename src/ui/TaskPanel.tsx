import { useState } from "react";
import { useRoster } from "../camp/roster";
import { useRuns, type Clarification } from "../camp/run";
import { PixelDialog } from "./PixelDialog";
import theme from "../../themes/three-kingdoms/ui.json";

interface Assignment { roleId: string; index: number; request: string }

export function TaskPanel({ campId, ready }: { campId: string; ready: boolean }) {
  const role = useRoster((s) => s.roles.find((r) => r.id === s.selectedId));
  const { busy, error, start, stop, dismiss } = useRuns();
  const [prompt, setPrompt] = useState("");
  const [choice, setChoice] = useState({ roleId: "", index: 0 });
  const [followup, setFollowup] = useState<{ assignment: Assignment; questions: string[] } | null>(null);
  const [answer, setAnswer] = useState("");
  const index = choice.roleId === role?.id ? choice.index : 0;
  const captain = role?.role_type === "captain";
  async function assign(assignment: Assignment, clarification?: Clarification) {
    setFollowup(null);
    setAnswer("");
    const questions = await start(campId, assignment.roleId, assignment.index, assignment.request, clarification);
    if (questions?.length) setFollowup({ assignment, questions });
  }
  function clarify(response: string | null) {
    if (followup) void assign(followup.assignment, { questions: followup.questions, answer: response });
  }
  function cancelClarification() {
    if (followup) dismiss(followup.assignment.roleId);
    setFollowup(null);
  }
  return <section className="task-panel">
    <h2>{captain ? theme.planTitle : theme.taskTitle} · {role?.name ?? theme.taskSelect}</h2>
    {role && role.loadouts.length > 1 && <div className="task-loadouts" role="group" aria-label={theme.taskLoadout}>
      {role.loadouts.map((loadout, i) => <button key={i} type="button" aria-pressed={index === i} disabled={busy} onClick={() => setChoice({ roleId: role.id, index: i })}>{loadout.cli} · {loadout.model}</button>)}
    </div>}
    <label htmlFor="task-prompt">{captain ? theme.planPrompt : theme.taskPrompt}</label>
    <textarea id="task-prompt" rows={2} value={prompt} disabled={busy} onChange={(e) => setPrompt(e.target.value)} placeholder={captain ? theme.planPlaceholder : theme.taskPlaceholder} />
    <div className="task-actions"><button disabled={!ready || !role || busy || !prompt.trim()} onClick={() => role && void assign({ roleId: role.id, index, request: prompt })}>{captain ? theme.planStart : theme.taskStart}</button>
      {busy && <button onClick={() => void stop()}>{theme.taskStop}</button>}</div>
    {error && <p role="alert">{error}</p>}
    <PixelDialog open={followup !== null} title={theme.planQuestions} closeLabel={theme.planCancel} onClose={cancelClarification}>
      {followup && <form className="plan-clarification" onSubmit={(event) => {
        event.preventDefault();
        if (answer.trim()) clarify(answer.trim());
      }}>
        <ol>{followup.questions.map((question, i) => <li key={i}>{question}</li>)}</ol>
        <label htmlFor="plan-answer">{theme.planAnswer}</label>
        <textarea id="plan-answer" rows={3} value={answer} onChange={(event) => setAnswer(event.target.value)} />
        <div className="task-actions">
          <button type="submit" disabled={busy || !answer.trim()}>{theme.planContinue}</button>
          <button type="button" disabled={busy} onClick={() => clarify(null)}>{theme.planDecide}</button>
        </div>
      </form>}
    </PixelDialog>
  </section>;
}
