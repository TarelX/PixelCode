import { useEffect, useRef, useState } from "react";
import { Application, TextureStyle } from "pixi.js";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAppStore } from "../app/store";
import { zh } from "../i18n/zh";
import { drawCamp, loadCampLayout } from "../camp/tilemap";
import { loadGenerals, placeGenerals } from "../camp/sprites";

export function CampView() {
  const camp = useAppStore((s) => s.camp);
  const hostRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const host = hostRef.current!;
    const layout = loadCampLayout();
    const app = new Application();
    let cancelled = false;
    let ready = false;
    let resolution = window.devicePixelRatio || 1;
    TextureStyle.defaultOptions.scaleMode = "nearest";

    function resize() {
      if (!ready) return;
      const scale = Math.max(1, Math.floor(Math.min(
        host.clientWidth / layout.width, host.clientHeight / layout.height,
      )));
      app.stage.scale.set(scale);
      app.renderer.resize(layout.width * scale, layout.height * scale, resolution);
      app.canvas.style.width = `${layout.width * scale}px`;
      app.canvas.style.height = `${layout.height * scale}px`;
    }

    const observer = new ResizeObserver(resize);
    observer.observe(host);
    async function mount() {
      try {
        const size = await getCurrentWindow().innerSize();
        resolution = Math.max(resolution, size.width / window.innerWidth);
        if (cancelled) {
          app.stage.destroy({ children: true });
          return;
        }
        await app.init({
          width: layout.width, height: layout.height,
          antialias: false, resolution, roundPixels: true,
        });
        if (cancelled) {
          app.destroy(true, { children: true });
          return;
        }
        app.stage.addChild(drawCamp(layout));
        host.appendChild(app.canvas);
        ready = true;
        resize();
        const generals = await loadGenerals();
        if (cancelled) {
          for (const general of generals) general.destroy({ children: true });
          return;
        }
        const stations = Object.fromEntries(layout.zones
          .filter((zone) => zone.role)
          .map((zone) => [zone.role!, {
            x: (zone.x + zone.w / 2) * layout.tileSize,
            y: (zone.y + zone.h) * layout.tileSize,
          }]));
        placeGenerals(app.stage, generals, stations);
      } catch (reason) {
        if (!cancelled) {
          if (app.renderer) app.destroy(true, { children: true });
          else app.stage.destroy({ children: true });
          ready = false;
          setError(`Unable to render camp: ${String(reason)}`);
        }
      }
    }
    void mount();
    return () => {
      cancelled = true;
      observer.disconnect();
      if (ready) app.destroy(true, { children: true });
      ready = false;
    };
  }, []);

  return (
    <section className="screen camp-screen">
      <h1>{zh.campTitle}</h1>
      <h2>{camp?.name}</h2>
      <p>{camp?.root_path}</p>
      {error && <p role="alert">{error}</p>}
      <div className="camp-canvas" ref={hostRef} />
    </section>
  );
}
