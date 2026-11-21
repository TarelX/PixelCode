import type { CSSProperties } from "react";

const paths = {
  camp: "M2 13h12M3 11V7l5-4 5 4v4M6 13V9h4v4M8 3V1h4v3H8",
  home: "M2 7l6-5 6 5M4 6v8h8V6M7 14v-4h2v4",
  folder: "M2 4h5l2 2h5v7H2V4Z",
  plus: "M8 3v10M3 8h10",
  arrow: "M3 8h10M9 4l4 4-4 4",
  settings: "M5 2v3H2v6h3v3h6v-3h3V5h-3V2H5ZM6 6h4v4H6V6Z",
  terminal: "M2 3h12v10H2V3ZM4 6l2 2-2 2M8 10h3",
};

export function PixelIcon({ name, style }: { name: keyof typeof paths; style?: CSSProperties }) {
  return <svg className="pixel-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true" style={style}>
    <path d={paths[name]} />
  </svg>;
}
