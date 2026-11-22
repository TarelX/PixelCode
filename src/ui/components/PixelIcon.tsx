import type { CSSProperties } from "react";

const paths = {
  camp: "M2 13h12M3 11V7l5-4 5 4v4M6 13V9h4v4M8 3V1h4v3H8",
  home: "M2 7l6-5 6 5M4 6v8h8V6M7 14v-4h2v4",
  folder: "M2 4h5l2 2h5v7H2V4Z",
  plus: "M8 3v10M3 8h10",
  arrow: "M3 8h10M9 4l4 4-4 4",
  settings: "M5 2v3H2v6h3v3h6v-3h3V5h-3V2H5ZM6 6h4v4H6V6Z",
  terminal: "M2 3h12v10H2V3ZM4 6l2 2-2 2M8 10h3",
  chat: "M2 3h12v8H7l-3 3v-3H2V3ZM5 6h6M5 8h4",
  chevron: "m5 6 3 3 3-3",
  info: "M8 7v4M8 4v1M3 2h10l2 3v6l-2 3H3l-2-3V5l2-3Z",
  edit: "m10 2 4 4-8 8H2v-4l8-8ZM8 4l4 4",
  send: "M8 13V3M3 8l5-5 5 5",
  team: "M6 2h4v5H6V2ZM3 14v-3l3-2h4l3 2v3M1 4h2v4H1M15 4h-2v4h2",
  grid: "M2 2h4v4H2V2ZM10 2h4v4h-4V2ZM2 10h4v4H2v-4ZM10 10h4v4h-4v-4Z",
};

export function PixelIcon({ name, style }: { name: keyof typeof paths; style?: CSSProperties }) {
  return <svg className="pixel-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true" style={style}>
    <path d={paths[name]} />
  </svg>;
}
