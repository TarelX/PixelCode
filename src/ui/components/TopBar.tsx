import type { ReactNode } from "react";

interface TopBarProps {
  title: string;
  children?: ReactNode;
}

export function TopBar({ title, children }: TopBarProps) {
  return (
    <header className="pixel-top-bar">
      <span className="pixel-top-bar__title"><span className="pixel-top-bar__eyebrow">工作空间</span><span aria-hidden="true">/</span>{title}</span>
      <div className="pixel-top-bar__actions">{children}</div>
    </header>
  );
}
