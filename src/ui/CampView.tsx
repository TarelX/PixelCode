import { useCallback, useEffect, useRef, useState } from "react";
import { Application, Container, Graphics, TextureStyle } from "pixi.js";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { MissingAssetError } from "../camp/asset-error";
import { useAppStore } from "../app/store";
import { drawCamp, loadCampLayout } from "../camp/tilemap";
import { loadGenerals, placeGenerals, type GeneralSprite } from "../camp/sprites";
import { useRoster, type RoleConfig } from "../camp/roster";
import { PixelDialog } from "./PixelDialog";
import { RoleCard } from "./RoleCard";
import { RoleRoster } from "./RoleRoster";
import { TaskPanel } from "./TaskPanel";
import { CouncilPanel } from "./CouncilPanel";
import { TaskBoard } from "./TaskBoard";
import { BattleReportButton } from "./BattleReportButton";
import { PixelButton } from "./components/PixelButton";
import { RunBubble } from "./RunBubble";
import { listenToRuns, useRuns } from "../camp/run";
import type { BubbleAnchor } from "../camp/bubble";
import { useI18n, roleName } from "../i18n";

function drawMarker(color: string) {
  return new Graphics()
    .rect(-7, 1, 14, 1).rect(-7, -1, 1, 2).rect(6, -1, 1, 2)
    .rect(-1, -31, 3, 1).rect(0, -30, 1, 1)
    .fill(color);
}

export function CampView() {
  const { locale, theme } = useI18n();
  const camp = useAppStore((s) => s.camp);
  const loadRoles = useRoster((s) => s.load);
  const roles = useRoster((s) => s.roles);
  const selectedId = useRoster((s) => s.selectedId);
  const select = useRoster((s) => s.select);
  const roleError = useRoster((s) => s.error);
  const hostRef = useRef<HTMLDivElement>(null);
  const stageRef = useRef<Container | null>(null);
  const appRef = useRef<Application | null>(null);
  const generalsRef = useRef<{ generals: GeneralSprite[]; marker: Graphics } | null>(null);
  const [stageVersion, setStageVersion] = useState(0);
  const [generalsVersion, setGeneralsVersion] = useState(0);
  const [error, setError] = useState<["campError" | "generalsError", unknown] | null>(null);
  const [card, setCard] = useState<{ role: RoleConfig | null } | null>(null);
  const [wallOpen, setWallOpen] = useState(false);
  const runs = useRuns((s) => s.runs);
  const [listening, setListening] = useState(false);
  const [anchors, setAnchors] = useState<Record<string, BubbleAnchor>>({});

  useEffect(() => {
    if (!camp) return;
    let cancelled = false;
    const subscription = listenToRuns(camp.id);
    subscription.then(() => { if (!cancelled) setListening(true); }).catch((reason) => { if (!cancelled) setError(["campError", reason]); });
    return () => { cancelled = true; setListening(false); void subscription.then((unlisten) => unlisten(), () => {}); };
  }, [camp?.id]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const position = () => {
      const canvas = host.querySelector("canvas");
      const stage = stageRef.current;
      if (!canvas || !stage) return;
      const bounds = canvas.getBoundingClientRect();
      const parent = host.getBoundingClientRect();
      setAnchors(Object.fromEntries((generalsRef.current?.generals ?? []).map((general) => [general.role.id, {
        x: Math.round(general.x * stage.scale.x),
        y: Math.round((general.y - 26) * stage.scale.y),
        footY: Math.round(general.y * stage.scale.y),
        canvas: { left: bounds.left - parent.left, top: bounds.top - parent.top, width: bounds.width, height: bounds.height },
      }])));
    };
    const observer = new ResizeObserver(position);
    observer.observe(host);
    host.addEventListener("scroll", position);
    position();
    return () => { observer.disconnect(); host.removeEventListener("scroll", position); };
  }, [generalsVersion]);

  useEffect(() => {
    for (const general of generalsRef.current?.generals ?? []) {
      const status = runs[general.role.id]?.status;
      const state = status === "completed" ? "done" : status === "failed" ? "error" : status === "stopped" ? "cancelled" : status === "waiting" ? "waiting_user" : status ?? "idle";
      if (general.state !== state) general.setState(state);
    }
  }, [runs, generalsVersion]);

  const openCard = useCallback((role: RoleConfig | null) => {
    if (role) select(role.id);
    setCard({ role });
  }, [select]);
  const openCardRef = useRef(openCard);
  openCardRef.current = openCard;

  useEffect(() => {
    loadRoles(camp?.id ?? null).catch(() => undefined);
  }, [camp?.id, loadRoles]);

  useEffect(() => {
    setError(null);
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
        const scenery = await drawCamp(layout);
        if (cancelled) {
          scenery.destroy({ children: true });
          app.destroy(true, { children: true });
          return;
        }
        app.stage.addChild(scenery);
        host.appendChild(app.canvas);
        ready = true;
        resize();
        stageRef.current = app.stage;
        appRef.current = app;
        setStageVersion((version) => version + 1);
      } catch (reason) {
        if (!cancelled) {
          if (app.renderer) app.destroy(true, { children: true });
          else app.stage.destroy({ children: true });
          ready = false;
          setError(["campError", reason]);
        }
      }
    }
    void mount();
    return () => {
      cancelled = true;
      observer.disconnect();
      stageRef.current = null;
      appRef.current = null;
      if (ready) app.destroy(true, { children: true });
      ready = false;
    };
  }, []);

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage || roles.length === 0) return;
    const layout = loadCampLayout();
    const layer = new Container();
    let cancelled = false;
    loadGenerals(roles).then((generals) => {
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
      for (const general of generals) {
        general.eventMode = "static";
        general.cursor = "pointer";
        general.on("pointertap", () => openCardRef.current(general.role));
      }
      const accent = getComputedStyle(document.documentElement).getPropertyValue("--ui-accent").trim() || "#da795c";
      const marker = drawMarker(accent);
      marker.visible = false;
      layer.addChild(marker);
      placeGenerals(layer, generals, stations);
      stage.addChild(layer);
      generalsRef.current = { generals, marker };
      setGeneralsVersion((version) => version + 1);
    }).catch((reason) => {
      if (!cancelled) setError(["generalsError", reason]);
    });
    return () => {
      cancelled = true;
      generalsRef.current = null;
      if (!layer.destroyed) layer.destroy({ children: true });
    };
  }, [roles, stageVersion]);

  useEffect(() => {
    const current = generalsRef.current;
    if (!current) return;
    const general = current.generals.find((g) => g.role.id === selectedId);
    current.marker.visible = Boolean(general);
    if (general) current.marker.position.copyFrom(general.position);
  }, [selectedId, generalsVersion]);

  return (
    <section className="screen camp-screen">
      <div className="camp-heading"><div><span className="section-label">{theme.campEyebrow}</span><h1>{camp?.name || theme.campTitle}</h1><p title={camp?.root_path}>{camp?.root_path}</p></div><div className="camp-heading-actions">
        <span className="camp-ready"><i className="status-dot" />{theme.campLegend}</span>
        {camp && <BattleReportButton key={camp.id} camp={camp} snapshot={() => appRef.current?.renderer.extract.canvas({ target: appRef.current.stage }) as HTMLCanvasElement | undefined} />}
      </div></div>
      {(roleError || error) && <p role="alert">{roleError ? theme.rosterError + roleError
        : error && theme[error[0]] + (error[1] instanceof MissingAssetError ? theme.missingAsset + error[1].message : String(error[1]))}</p>}
      <div className="camp-body">
        <div className="camp-map"><div className="camp-map__bar"><span><i className="status-dot" />{theme.campMapLabel}</span><span>320 × 180</span></div><div className="camp-canvas" ref={hostRef}>
          <div className="run-overlay">{Object.entries(runs).map(([id, run]) => anchors[id] && <RunBubble key={id} id={id} run={run} anchor={anchors[id]} />)}</div>
        </div></div>
        <RoleRoster onOpen={openCard} />
      </div>
      {camp && <TaskPanel key={`task-${camp.id}`} campId={camp.id} ready={listening} />}
      {camp && <CouncilPanel key={`council-${camp.id}`} campId={camp.id} ready={listening} onPlan={() => setWallOpen(true)} />}
      {camp && <div><PixelButton onClick={() => setWallOpen(true)}>{theme.wall.title}</PixelButton></div>}
      <PixelDialog open={wallOpen} title={theme.wall.title} closeLabel={theme.wall.close} onClose={() => setWallOpen(false)}>
        {wallOpen && camp && <TaskBoard key={camp.id} campId={camp.id} />}
      </PixelDialog>
      <p className="camp-caption">{theme.campMapHint}</p>
      <PixelDialog
        open={card !== null}
        title={card?.role ? `${theme.roleCardTitle} · ${roleName(card.role, locale)}` : theme.newRoleCardTitle}
        closeLabel={theme.closeRoleCard}
        onClose={() => setCard(null)}
      >
        {card && <RoleCard key={card.role?.id ?? "new"} role={card.role} onDone={() => setCard(null)} />}
      </PixelDialog>
    </section>
  );
}
