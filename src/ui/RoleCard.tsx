import { useEffect, useId, useState, type FormEvent } from "react";
import { useAppStore } from "../app/store";
import { useRoster, type CliId, type RoleConfig, type RoleType } from "../camp/roster";
import { avatarFile, avatarFiles } from "../camp/sprites";
import { PixelButton } from "./components/PixelButton";
import { PixelIcon } from "./components/PixelIcon";
import { RoleAvatar } from "./components/RoleAvatar";
import { useI18n, roleName, roleTitle } from "../i18n";

const clis: { id: CliId; label: string }[] = [{ id: "claude-code", label: "Claude Code" }, { id: "codex", label: "Codex" }];

type RoleError = "skillsError" | "roleSaveError" | "roleDeleteError" | "roleIdInvalid" | "roleIdTaken" | "roleNameRequired" | "rolePersonalityRequired";

function blankRole(): RoleConfig {
  return {
    id: `custom-${Date.now().toString(36)}`,
    role_type: "frontend",
    name: "",
    name_en: null,
    title: null,
    avatar: avatarFiles[0] ?? null,
    personality: "",
    skills: [],
    loadouts: [{ cli: "claude-code", model: "sonnet" }],
    permissions: { allow: ["Read", "Grep", "Glob"], deny: ["Bash(git push *)", "Bash(rm -rf *)"] },
    task_types: [],
    source: "custom",
  };
}

const lines = (text: string) => text.split("\n").map((line) => line.trim()).filter(Boolean);
const optional = (text: string | null) => text?.trim() || null;

export function RoleCard({ role, onDone }: { role: RoleConfig | null; onDone: () => void }) {
  const { locale, theme } = useI18n();
  const campId = useAppStore((s) => s.camp?.id ?? null);
  const roles = useRoster((s) => s.roles);
  const skills = useRoster((s) => s.skills);
  const loadSkills = useRoster((s) => s.loadSkills);
  const save = useRoster((s) => s.save);
  const remove = useRoster((s) => s.remove);
  const [draft, setDraft] = useState<RoleConfig>(() => role ? structuredClone(role) : blankRole());
  const [allow, setAllow] = useState(draft.permissions.allow.join("\n"));
  const [deny, setDeny] = useState(draft.permissions.deny.join("\n"));
  const [taskTypes, setTaskTypes] = useState(draft.task_types.join(", "));
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<[RoleError, string?] | null>(null);
  const id = useId();
  const isNew = role === null;

  useEffect(() => {
    loadSkills().catch((reason) => setError(["skillsError", String(reason)]));
  }, [loadSkills]);

  const update = (patch: Partial<RoleConfig>) => setDraft((current) => ({ ...current, ...patch }));
  const updateLoadout = (index: number, patch: Partial<RoleConfig["loadouts"][number]>) =>
    update({ loadouts: draft.loadouts.map((loadout, i) => i === index ? { ...loadout, ...patch } : loadout) });

  async function run(action: () => Promise<void>, key: RoleError) {
    setBusy(true);
    setError(null);
    try {
      await action();
      onDone();
    } catch (reason) {
      setError([key, String(reason)]);
      setBusy(false);
    }
  }

  function submit(event: FormEvent) {
    event.preventDefault();
    const next: RoleConfig = {
      ...draft,
      id: draft.id.trim(),
      name: draft.name.trim(),
      name_en: optional(draft.name_en),
      title: optional(draft.title),
      personality: draft.personality.trim(),
      loadouts: draft.loadouts.map((loadout) => ({ ...loadout, model: loadout.model.trim() || "auto" })),
      permissions: { allow: lines(allow), deny: lines(deny) },
      task_types: taskTypes.split(/[,，]/).map((type) => type.trim()).filter(Boolean),
    };
    const invalid: [RoleError, string?] | null = isNew && !/^[a-z0-9]+(-[a-z0-9]+)*$/.test(next.id) ? ["roleIdInvalid"]
      : isNew && roles.some((existing) => existing.id === next.id) ? ["roleIdTaken", next.id]
      : !next.name ? ["roleNameRequired"]
      : !next.personality ? ["rolePersonalityRequired"]
      : null;
    if (invalid) {
      setError(invalid);
      return;
    }
    void run(() => save(next, campId), "roleSaveError");
  }

  return (
    <form className="role-card" noValidate onSubmit={submit} aria-busy={busy}>
      <header className="role-card__hero">
        <span className="role-card__portrait"><RoleAvatar avatar={draft.avatar} scale={2} /></span>
        <div className="role-card__identity">
          <span className="section-label">{isNew ? theme.newRoleCardTitle : theme.roleCardTitle}</span>
          <h3>{roleName(draft, locale) || theme.roleName}</h3>
          <p>{roleTitle(draft, locale)}</p>
          <div className="role-card__tags">
            <em className="role-tag">{theme.roleTypes[draft.role_type]}</em>
            <em className={`role-tag role-tag--${draft.source}`}>{theme.roleSources[draft.source]}</em>
            <code>{draft.id || "—"}</code>
          </div>
          {isNew && <p className="role-card__intro">{theme.newRoleIntro}</p>}
        </div>
      </header>

      <fieldset className="role-card__section role-card__grid">
        <legend className="section-label">PROFILE</legend>
        {isNew && <label className="role-field role-field--wide"><span>{theme.roleId}</span><input value={draft.id} onChange={(e) => update({ id: e.target.value })} aria-describedby={`${id}-id`} /><small id={`${id}-id`}>{theme.roleIdHint}</small></label>}
        <label className="role-field"><span>{theme.roleName}</span><input value={draft.name} onChange={(e) => update({ name: e.target.value })} /></label>
        <label className="role-field"><span>{theme.roleNameEn}</span><input value={draft.name_en ?? ""} onChange={(e) => update({ name_en: e.target.value })} /></label>
        <label className="role-field"><span>{theme.roleTitle}</span><input value={draft.title ?? ""} onChange={(e) => update({ title: e.target.value })} /></label>
        <div className="role-field role-field--wide"><span id={`${id}-type`}>{theme.roleType}</span>
          <div className="role-choice" role="radiogroup" aria-labelledby={`${id}-type`}>
            {(Object.entries(theme.roleTypes) as [RoleType, string][]).map(([type, label]) => (
              <button key={type} type="button" role="radio" aria-checked={draft.role_type === type} onClick={() => update({ role_type: type })}>{label}</button>
            ))}
          </div>
        </div>
        <div className="role-field role-field--wide"><span id={`${id}-avatar`}>{theme.roleAvatar}</span>
          <div className="role-card__avatars" role="radiogroup" aria-labelledby={`${id}-avatar`}>
            {avatarFiles.map((file) => (
              <button key={file} type="button" role="radio" aria-checked={avatarFile(draft.avatar) === file} aria-label={file} title={file} className="role-card__avatar-option" onClick={() => update({ avatar: file })}>
                <RoleAvatar avatar={file} />
              </button>
            ))}
          </div>
        </div>
      </fieldset>

      <fieldset className="role-card__section">
        <legend className="section-label">PERSONA</legend>
        <label className="role-field"><span>{theme.rolePersonality}</span><textarea rows={4} value={draft.personality} onChange={(e) => update({ personality: e.target.value })} aria-describedby={`${id}-persona`} /><small id={`${id}-persona`}>{theme.rolePersonalityHint}</small></label>
      </fieldset>

      <fieldset className="role-card__section">
        <legend className="section-label">SKILLS</legend>
        <span className="role-field__label">{theme.roleSkills}</span>
        <div className="role-card__skills">
          {skills.map((skill) => (
            <label key={skill.id} className="role-check">
              <input type="checkbox" checked={draft.skills.includes(skill.id)} onChange={(e) => update({ skills: e.target.checked ? [...draft.skills, skill.id] : draft.skills.filter((s) => s !== skill.id) })} />
              <span><code>{skill.id}</code><small>{skill.summary}</small></span>
            </label>
          ))}
        </div>
      </fieldset>

      <fieldset className="role-card__section">
        <legend className="section-label">LOADOUTS</legend>
        <div className="role-field__label">{theme.roleLoadouts}<small>{theme.roleLoadoutsHint}</small></div>
        <ol className="role-card__loadouts">
          {draft.loadouts.map((loadout, index) => (
            <li key={index}>
              <span className="role-card__slot">{String(index + 1).padStart(2, "0")}</span>
              <div className="role-choice" role="radiogroup" aria-label={theme.loadoutCli}>
                {clis.map((cli) => <button key={cli.id} type="button" role="radio" aria-checked={loadout.cli === cli.id} onClick={() => updateLoadout(index, { cli: cli.id })}>{cli.label}</button>)}
              </div>
              <input aria-label={theme.loadoutModel} placeholder="auto" value={loadout.model} onChange={(e) => updateLoadout(index, { model: e.target.value })} />
              <PixelButton disabled={draft.loadouts.length === 1} onClick={() => update({ loadouts: draft.loadouts.filter((_, i) => i !== index) })}>{theme.removeLoadout}</PixelButton>
            </li>
          ))}
        </ol>
        <PixelButton className="role-card__add" onClick={() => update({ loadouts: [...draft.loadouts, { cli: "codex", model: "auto" }] })}><PixelIcon name="plus" />{theme.addLoadout}</PixelButton>
      </fieldset>

      <fieldset className="role-card__section role-card__grid">
        <legend className="section-label">PERMISSIONS</legend>
        <label className="role-field"><span>{theme.permissionsAllow}</span><textarea rows={3} value={allow} onChange={(e) => setAllow(e.target.value)} spellCheck={false} /></label>
        <label className="role-field"><span>{theme.permissionsDeny}</span><textarea rows={3} value={deny} onChange={(e) => setDeny(e.target.value)} spellCheck={false} /></label>
        <label className="role-field role-field--wide"><span>{theme.roleTaskTypes}</span><input value={taskTypes} onChange={(e) => setTaskTypes(e.target.value)} /></label>
      </fieldset>

      <footer className="role-card__actions">
        {error && <p className="role-card__error" role="alert">{theme[error[0]]}{error[1]}</p>}
        {draft.source === "modified_preset" && <PixelButton disabled={busy} onClick={() => void run(() => remove(draft.id, campId), "roleDeleteError")}>{theme.restorePreset}</PixelButton>}
        {!isNew && draft.source === "custom" && <PixelButton className="role-card__danger" disabled={busy} onClick={() => confirming ? void run(() => remove(draft.id, campId), "roleDeleteError") : setConfirming(true)}>{confirming ? theme.confirmDelete : theme.deleteRole}</PixelButton>}
        <span />
        <PixelButton disabled={busy} onClick={onDone}>{theme.cancel}</PixelButton>
        <PixelButton type="submit" variant="primary" disabled={busy}>{busy ? theme.savingRole : theme.saveRole}</PixelButton>
      </footer>
    </form>
  );
}
