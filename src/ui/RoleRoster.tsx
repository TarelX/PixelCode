import { useRoster, type RoleConfig } from "../camp/roster";
import { PixelButton } from "./components/PixelButton";
import { PixelIcon } from "./components/PixelIcon";
import { RoleAvatar } from "./components/RoleAvatar";
import { useI18n, roleName, roleTitle } from "../i18n";
import { useRuns } from "../camp/run";

export function RoleRoster({ onOpen }: { onOpen: (role: RoleConfig | null) => void }) {
  const { locale, theme } = useI18n();
  const roles = useRoster((s) => s.roles);
  const selectedId = useRoster((s) => s.selectedId);
  const select = useRoster((s) => s.select);
  const runs = useRuns((s) => s.runs);
  return (
    <aside className="role-roster" aria-label={theme.rosterTitle}>
      <div className="role-roster__head">
        <div><h2>{theme.studio.agents}</h2></div>
        <span>{roles.length}</span>
      </div>
      {roles.length === 0 && <p className="role-roster__hint" role="status">{theme.rolesLoading}</p>}
      <ul>
        {roles.map((role) => (
          <li key={role.id}>
            <button type="button" className={`role-roster__item ${role.id === selectedId ? "is-selected" : ""}`} aria-pressed={role.id === selectedId} onClick={() => select(role.id)}>
              <RoleAvatar avatar={role.avatar} scale={1} />
              <span className="role-roster__name"><strong>{roleName(role, locale)}</strong><small>{roleTitle(role, locale)}</small></span>
              {role.source !== "preset" && <em className={`role-tag role-tag--${role.source}`}>{theme.roleSources[role.source]}</em>}
              <i className={`status-dot run-dot--${runs[role.id]?.status ?? "idle"}`} title={runs[role.id] ? theme.runStates[runs[role.id].status] : theme.studio.teamReady} />
            </button>
            <button className="role-edit" aria-label={`${theme.studio.editAgent} · ${roleName(role, locale)}`} onClick={() => onOpen(role)}><PixelIcon name="edit" /></button>
          </li>
        ))}
      </ul>
      <PixelButton className="role-roster__new" onClick={() => onOpen(null)}><PixelIcon name="plus" />{theme.newGeneral}</PixelButton>
    </aside>
  );
}
