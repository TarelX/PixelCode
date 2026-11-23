import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useRoster } from "../camp/roster";
import { useRuns, type PendingQuestion } from "../camp/run";
import { PixelDialog } from "./PixelDialog";
import { useI18n, roleName } from "../i18n";
import { RoleAvatar } from "./components/RoleAvatar";
import { PixelIcon } from "./components/PixelIcon";
import { ComposerPicker } from "./components/ComposerPicker";

interface Assignment { roleId: string; index: number; request: string }
interface Followup {
  roleId: string;
  loadoutIndex: number;
  request: string;
  questions: string[];
  pendingId: string;
  answerKey: string;
}

function newAnswerKey() {
  return typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `answer-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function followupFromPending(item: PendingQuestion): Followup {
  return {
    roleId: item.role_id,
    loadoutIndex: item.loadout_index,
    request: item.request,
    questions: item.questions,
    pendingId: item.id,
    answerKey: newAnswerKey(),
  };
}

export function TaskPanel({ campId, ready, onConfigure }: { campId: string; ready: boolean; onConfigure: () => void }) {
  const { locale, theme } = useI18n();
  const role = useRoster((s) => s.roles.find((r) => r.id === s.selectedId));
  const roles = useRoster((s) => s.roles);
  const select = useRoster((s) => s.select);
  const { busy, error, start, stop, dismiss } = useRuns();
  const [prompt, setPrompt] = useState("");
  const [choice, setChoice] = useState({ roleId: "", index: 0 });
  const [followup, setFollowup] = useState<Followup | null>(null);
  const [answer, setAnswer] = useState("");
  const index = choice.roleId === role?.id ? choice.index : 0;
  const captain = role?.role_type === "captain";
  useEffect(() => {
    let cancelled = false;
    invoke<PendingQuestion[]>("pending_questions", { campId })
      .then((items) => {
        if (!cancelled && items[0]) setFollowup(followupFromPending(items[0]));
      })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [campId]);
  async function assign(assignment: Assignment) {
    setFollowup(null);
    setAnswer("");
    const result = await start(campId, assignment.roleId, assignment.index, assignment.request);
    if (result?.questions.length) {
      setFollowup({
        roleId: assignment.roleId,
        loadoutIndex: assignment.index,
        request: assignment.request,
        questions: result.questions,
        pendingId: result.pending_id,
        answerKey: newAnswerKey(),
      });
    }
  }
  async function answerFollowup(response: string | null) {
    if (!followup) return;
    const current = followup;
    setFollowup(null);
    setAnswer("");
    const result = await start(
      campId,
      current.roleId,
      current.loadoutIndex,
      current.request,
      { questions: current.questions, answer: response },
      current.pendingId,
      current.answerKey,
    );
    if (result?.questions.length) {
      setFollowup({ ...current, questions: result.questions, pendingId: result.pending_id, answerKey: newAnswerKey() });
    }
  }
  function clarify(response: string | null) {
    void answerFollowup(response);
  }
  function cancelClarification() {
    if (followup) dismiss(followup.roleId);
    setFollowup(null);
    setAnswer("");
  }
  function submit() {
    if (ready && role && !busy && prompt.trim()) void assign({ roleId: role.id, index, request: prompt.trim() });
  }
  const loadout = role?.loadouts[index];
  return <section className="task-panel task-composer">
    <label className="sr-only" htmlFor="task-prompt">{captain ? theme.planPrompt : theme.taskPrompt}</label>
    <div className="composer-input">
    <textarea id="task-prompt" rows={2} value={prompt} disabled={busy} onChange={(e) => setPrompt(e.target.value)} placeholder={theme.studio.prompt} onKeyDown={(event) => {
      if (!event.nativeEvent.isComposing && event.nativeEvent.keyCode !== 229 && event.key === "Enter" && (event.ctrlKey || event.metaKey)) { event.preventDefault(); submit(); }
    }} />
    <div className="composer-team">{roles.map((item) => <button key={item.id} aria-label={roleName(item, locale)} title={roleName(item, locale)} aria-pressed={item.id === role?.id} disabled={busy} onClick={() => select(item.id)}><RoleAvatar avatar={item.avatar} scale={1} /></button>)}</div>
    </div>
    <div className="composer-toolbar">
      <ComposerPicker icon="team" label={role ? roleName(role, locale) : theme.studio.role}>
        <strong>{theme.studio.role}</strong>{roles.map((item) => <button key={item.id} disabled={busy} aria-pressed={item.id === role?.id} onClick={() => select(item.id)}><RoleAvatar avatar={item.avatar} scale={1} /><span>{roleName(item, locale)}<small>{theme.roleTypes[item.role_type]}</small></span></button>)}
      </ComposerPicker>
      <ComposerPicker icon="terminal" className="composer-model" label={loadout ? `${loadout.cli} · ${loadout.model}` : theme.studio.loadout}><strong>{theme.studio.loadout}</strong>
        {role?.loadouts.map((item, i) => <button key={i} aria-pressed={index === i} disabled={busy} onClick={() => setChoice({ roleId: role.id, index: i })}><PixelIcon name="terminal" /><span>{item.model}<small>{item.cli}</small></span></button>)}
      </ComposerPicker>
      <ComposerPicker icon="grid" className="composer-skills" label={theme.studio.skills}><strong>{theme.studio.skills}</strong><small>{theme.studio.skillHint}</small>
        {role?.skills.length ? role.skills.map((skill) => <p key={skill}>{skill}</p>) : <p>{theme.studio.noSkills}</p>}<button onClick={onConfigure}><PixelIcon name="settings" />{theme.studio.configure}</button>
      </ComposerPicker>
      <button className="composer-permissions" title={theme.studio.permissions} aria-label={theme.studio.permissions} onClick={onConfigure}><PixelIcon name="settings" /></button>
      {busy ? <button className="composer-send" onClick={() => void stop()}>{theme.taskStop}</button> : <button className="composer-send" aria-label={captain ? theme.planStart : theme.taskStart} title={`${theme.studio.send} · Ctrl+Enter`} disabled={!ready || !role || !prompt.trim()} onClick={submit}><PixelIcon name="send" /></button>}
    </div>
    <div className="composer-caption"><span>{captain ? theme.studio.plan : theme.studio.direct} · {theme.studio.local}</span><span>Ctrl + Enter</span></div>
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
