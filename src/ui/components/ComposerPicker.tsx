import { useEffect, useId, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { placePicker } from "./popover-placement";
import { PixelIcon } from "./PixelIcon";

export function ComposerPicker({ label, icon, className = "", children }: { label: string; icon: "team" | "terminal" | "grid"; className?: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<CSSProperties>({ visibility: "hidden" });
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const id = useId();
  useLayoutEffect(() => {
    if (!open) return;
    const place = () => {
      if (trigger.current && menu.current) setPosition(placePicker(trigger.current.getBoundingClientRect(), menu.current.scrollHeight, { width: window.innerWidth, height: window.innerHeight }));
    };
    place();
    const observer = new ResizeObserver(place);
    if (menu.current) observer.observe(menu.current);
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => { observer.disconnect(); window.removeEventListener("resize", place); window.removeEventListener("scroll", place, true); };
  }, [open]);
  useEffect(() => {
    if (!open) return;
    const frame = requestAnimationFrame(() => {
      const selected = menu.current?.querySelector<HTMLButtonElement>('button[aria-pressed="true"]:not(:disabled)');
      (selected ?? menu.current?.querySelector<HTMLButtonElement>("button:not(:disabled)"))?.focus();
    });
    const outside = (event: PointerEvent) => { if (!menu.current?.contains(event.target as Node) && !trigger.current?.contains(event.target as Node)) setOpen(false); };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); setOpen(false); trigger.current?.focus(); }
    };
    window.addEventListener("pointerdown", outside);
    window.addEventListener("keydown", key, true);
    return () => { cancelAnimationFrame(frame); window.removeEventListener("pointerdown", outside); window.removeEventListener("keydown", key, true); };
  }, [open]);
  return <div className={`composer-picker ${className}`}><button className="composer-picker__trigger" ref={trigger} aria-expanded={open} aria-controls={open ? id : undefined} aria-haspopup="dialog" onClick={() => setOpen(!open)} onKeyDown={(event) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); setOpen(true); }
  }}><PixelIcon name={icon} /><span>{label}</span><PixelIcon name="chevron" /></button>
    {open && createPortal(<div ref={menu} id={id} role="dialog" aria-label={label} className="composer-menu" style={position} onClick={(event) => {
      if ((event.target as HTMLElement).closest("button:not(:disabled)")) { setOpen(false); trigger.current?.focus(); }
    }} onKeyDown={(event) => {
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
      const buttons = Array.from(menu.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []);
      const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
      event.preventDefault();
      buttons[(index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length]?.focus();
    }}>{children}</div>, document.body)}
  </div>;
}
