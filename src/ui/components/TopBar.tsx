import type { ReactNode } from "react";

interface TopBarProps {
  title: string;
  children?: ReactNode;
}

export function TopBar({ title, children }: TopBarProps) {
  return (
    <header className="pixel-top-bar">
      <span className="pixel-top-bar__title">{title}</span>
      {children}
    </header>
  );
}
