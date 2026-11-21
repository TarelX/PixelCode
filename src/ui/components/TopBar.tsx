import { useI18n } from "../../i18n";
import type { ReactNode } from "react";

interface TopBarProps {
  title: string;
  children?: ReactNode;
}

export function TopBar({ title, children }: TopBarProps) {
  const { theme } = useI18n();
  return (
    <header className="pixel-top-bar">
      <span className="pixel-top-bar__title"><span className="pixel-top-bar__eyebrow">{theme.workspace.title}</span><span aria-hidden="true">/</span>{title}</span>
      <div className="pixel-top-bar__actions">{children}</div>
    </header>
  );
}
