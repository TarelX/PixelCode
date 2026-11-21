import { useRoster, type RoleConfig } from "../camp/roster";
import { PixelButton } from "./components/PixelButton";
import { PixelIcon } from "./components/PixelIcon";
import { RoleAvatar } from "./components/RoleAvatar";
import { useI18n, roleName, roleTitle } from "../i18n";

export function RoleRoster({ onOpen }: { onOpen: (role: RoleConfig | null) => void }) {
  const { locale, theme } = useI18n();
  const roles = useRoster((s) => s.roles);
  const selectedId = useRoster((s) => s.selectedId);
  return (
    <aside className="role-roster" aria-label={theme.rosterTitle}>
      <div className="role-roster__head">
        <div><span className="section-label">{theme.rosterLabel}</span><h2>{theme.rosterTitle}</h2></div>
        <span>{roles.length}</span>
      </div>
      <p className="role-roster__hint">{theme.rosterHint}</p>
      {roles.length === 0 && <p className="role-roster__hint" role="status">{theme.rolesLoading}</p>}
      <ul>
        {roles.map((role) => (
          <li key={role.id}>
            <button type="button" className={`role-roster__item ${role.id === selectedId ? "is-selected" : ""}`} aria-pressed={role.id === selectedId} onClick={() => onOpen(role)}>
              <RoleAvatar avatar={role.avatar} />
              <span className="role-roster__name"><strong>{roleName(role, locale)}</strong><small>{roleTitle(role, locale)}</small></span>
              {role.source !== "preset" && <em className={`role-tag role-tag--${role.source}`}>{theme.roleSources[role.source]}</em>}
            </button>
          </li>
        ))}
      </ul>
      <PixelButton className="role-roster__new" onClick={() => onOpen(null)}><PixelIcon name="plus" />{theme.newGeneral}</PixelButton>
    </aside>
  );
}
