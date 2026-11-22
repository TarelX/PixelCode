import { useI18n } from "../../i18n";
import type { ReactNode } from "react";
import { BrandMark } from "./BrandMark";

interface TopBarProps {
  title: string;
  children?: ReactNode;
}

export function TopBar({ title, children }: TopBarProps) {
  const { theme } = useI18n();
  return (
    <header className="pixel-top-bar">
      <div className="pixel-top-bar__title"><BrandMark /><strong>{theme.appName}</strong><span className="top-bar-divider" aria-hidden="true">/</span><span className="pixel-top-bar__eyebrow">{theme.workspace.title}</span><span aria-hidden="true">/</span><span className="top-bar-project">{title}</span></div>
      <div className="pixel-top-bar__actions">{children}</div>
    </header>
  );
}
