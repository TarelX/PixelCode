import { useEffect, useState, type RefObject } from "react";
import { useI18n } from "../i18n";

export function MapScrollHint({ viewport, active }: { viewport: RefObject<HTMLDivElement | null>; active: boolean }) {
  const { theme } = useI18n();
  const [below, setBelow] = useState(false);

  useEffect(() => {
    const map = viewport.current;
    if (!map || !active) return;
    const update = () => setBelow(map.clientHeight > 0 && map.scrollHeight - map.clientHeight - map.scrollTop > 1);
    const observer = new ResizeObserver(update);
    observer.observe(map);
    if (map.firstElementChild) observer.observe(map.firstElementChild);
    map.addEventListener("scroll", update, { passive: true });
    update();
    return () => { observer.disconnect(); map.removeEventListener("scroll", update); };
  }, [viewport, active]);

  return active && below ? <div className="camp-map__scroll-hint">
    <i className="camp-map__scroll-arrow" aria-hidden="true" />{theme.studio.scrollMap}
  </div> : null;
}
