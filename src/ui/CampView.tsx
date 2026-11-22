import { useCallback, useEffect, useRef, useState } from "react";
import { Application, Container, Graphics, TextureStyle } from "pixi.js";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { MissingAssetError } from "../camp/asset-error";
import { useAppStore } from "../app/store";
import { drawCamp, loadCampLayout } from "../camp/tilemap";
import { FRAME_HEIGHT, loadGenerals, placeGenerals, type GeneralSprite } from "../camp/sprites";
import { useRoster, type RoleConfig } from "../camp/roster";
import { PixelDialog } from "./PixelDialog";
import { RoleCard } from "./RoleCard";
import { AgentPanel } from "./AgentPanel";
import { WorkspaceHeading } from "./WorkspaceHeading";
import { RunFeed } from "./RunFeed";
import { TaskPanel } from "./TaskPanel";
import { CouncilPanel } from "./CouncilPanel";
import { TaskBoard } from "./TaskBoard";
import { BattleReportButton } from "./BattleReportButton";
import { PixelIcon } from "./components/PixelIcon";
import { listenToRuns, useRuns } from "../camp/run";
import type { BubbleAnchor } from "../camp/bubble";
import { useI18n, roleName } from "../i18n";
import { useScenePlan } from "../camp/use-scene-plan";
import { SceneMotion } from "../camp/scene-motion";
import type { Handoff } from "../camp/collaboration";
import { SceneTasks, SceneHandoffs } from "./SceneActivity";

function drawMarker(color: string) {
  return new Graphics()
    .rect(-7, 1, 14, 1).rect(-7, -1, 1, 2).rect(6, -1, 1, 2)
    .rect(-1, -31, 3, 1).rect(0, -30, 1, 1)
    .fill(color);
}

export function CampView() {
  const { locale, theme } = useI18n();
  const camp = useAppStore((s) => s.camp);
  const tab = useAppStore((s) => s.workspaceTab);
  const setTab = useAppStore((s) => s.setWorkspaceTab);
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
  const [sceneOpen, setSceneOpen] = useState(true);
  const runs = useRuns((s) => s.runs);
  const [listening, setListening] = useState(false);
  const [anchors, setAnchors] = useState<Record<string, BubbleAnchor>>({});
  const { plan, error: sceneError } = useScenePlan(camp?.id);
  const motionRef = useRef<SceneMotion | null>(null);
  const sceneStateRef = useRef({ plan, runs });
  sceneStateRef.current = { plan, runs };
  const [handoff, setHandoff] = useState<Handoff | null>(null);

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
        y: Math.round((general.y - FRAME_HEIGHT - 2) * stage.scale.y),
        footY: Math.round(general.y * stage.scale.y),
        canvas: { left: bounds.left - parent.left, top: bounds.top - parent.top, width: bounds.width, height: bounds.height },
      }])));
    };
    const observer = new ResizeObserver(position);
    observer.observe(host);
    host.addEventListener("scroll", position);
    let elapsed = 0;
    const tick = () => { elapsed++; if (elapsed % 3 === 0) position(); };
    const app = appRef.current;
    app?.ticker.add(tick);
    position();
    return () => { observer.disconnect(); host.removeEventListener("scroll", position); app?.ticker.remove(tick); };
  }, [generalsVersion]);

  useEffect(() => {
    motionRef.current?.update(plan, runs);
  }, [runs, plan, generalsVersion]);

  const openCard = useCallback((role: RoleConfig | null) => {
    if (role) select(role.id);
    setCard({ role });
  }, [select]);
  const openCardRef = useRef(openCard);
  openCardRef.current = openCard;

  useEffect(() => {
    let active = true;
    void loadRoles(camp?.id ?? null).then((loaded) => {
      if (active && !loaded.some((role) => role.id === useRoster.getState().selectedId)) select(loaded.find((role) => role.role_type === "captain")?.id ?? loaded[0]?.id ?? null);
    }).catch(() => undefined);
    return () => { active = false; };
  }, [camp?.id, loadRoles, select]);

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
        const foreground = layout.foreground ? await drawCamp({ ...layout, background: layout.foreground }) : new Container();
        if (cancelled) {
          scenery.destroy({ children: true });
          foreground.destroy({ children: true });
          app.destroy(true, { children: true });
          return;
        }
        app.stage.addChild(scenery, foreground);
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
      stage.addChildAt(layer, 1);
      generalsRef.current = { generals, marker };
      const motion = new SceneMotion(generals, stations, layout.navigation);
      motionRef.current = motion;
      motion.update(sceneStateRef.current.plan, sceneStateRef.current.runs);
      const envelope = new Graphics().rect(0, 0, 7, 5).fill("#e4c98b").rect(1, 1, 5, 1).fill("#927344");
      layer.addChild(envelope);
      const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
      const tick = () => {
        motion.tick((appRef.current?.ticker.deltaMS ?? 16) / 1000, reduced.matches);
        const selected = generals.find((general) => general.role.id === useRoster.getState().selectedId);
        if (selected) marker.position.copyFrom(selected.position);
        const sender = generals.find((general) => general.role.id === motion.active?.from.role_id);
        envelope.visible = Boolean(sender);
        if (sender) envelope.position.set(sender.x + 6, sender.y - 12);
        setHandoff((previous) => previous?.id === motion.active?.id ? previous : motion.active);
      };
      appRef.current?.ticker.add(tick);
      layer.on("destroyed", () => appRef.current?.ticker.remove(tick));
      setGeneralsVersion((version) => version + 1);
    }).catch((reason) => {
      if (!cancelled) setError(["generalsError", reason]);
    });
    return () => {
      cancelled = true;
      generalsRef.current = null;
      motionRef.current = null;
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
      <WorkspaceHeading />
      <nav className="workbench-tabs" aria-label={theme.workspace.navigation}>
        <button aria-current={tab === "conversation" ? "page" : undefined} onClick={() => setTab("conversation")}><PixelIcon name="chat" />{theme.studio.conversation}</button>
        <button aria-current={tab === "agents" ? "page" : undefined} onClick={() => setTab("agents")}><PixelIcon name="team" />{theme.studio.agents}<span className="tab-tag">{roles.length}</span></button>
        <button aria-current={tab === "tasks" ? "page" : undefined} onClick={() => setTab("tasks")}><PixelIcon name="grid" />{theme.studio.tasks}</button>
        <button aria-current={tab === "council" ? "page" : undefined} onClick={() => setTab("council")}>{theme.council.title}</button>
      </nav>
      {(roleError || error) && <p role="alert">{roleError ? theme.rosterError + roleError
        : error && theme[error[0]] + (error[1] instanceof MissingAssetError ? theme.missingAsset + error[1].message : String(error[1]))}</p>}
      <div className="workbench-grid"><div className="workbench-center">
        <section className="collaboration-scene" hidden={tab !== "agents"}>
          <header><strong>{theme.studio.collaboration}</strong><span className="live-label"><i className="status-dot" />{theme.studio.live}</span><button onClick={() => setSceneOpen(!sceneOpen)}>{sceneOpen ? theme.studio.hideScene : theme.studio.showScene}<PixelIcon name="chevron" /></button></header>
          <div className="camp-map" hidden={!sceneOpen}><div className="camp-canvas" ref={hostRef}>
            <SceneTasks roles={roles} runs={runs} plan={plan} anchors={anchors} selectedId={selectedId} handoff={handoff} onSelect={select} />
            <div className="station-overlay">{roles.map((role) => {
              const anchor = anchors[role.id];
              if (!anchor) return null;
              const top = anchor.canvas.top + Math.min(anchor.footY + 24 * anchor.canvas.width / loadCampLayout().width, anchor.canvas.height - 20);
              return <button key={role.id} className={`station-label ${role.id === selectedId ? "is-selected" : ""}`} style={{ left: anchor.canvas.left + anchor.x, top }} onClick={() => select(role.id)} title={roleName(role, locale)}>
                <span>{roleName(role, locale)}</span><i className={`status-dot run-dot--${runs[role.id]?.status ?? "idle"}`} />
              </button>;
            })}</div>
          </div></div>
          {sceneOpen && <SceneHandoffs plan={plan} roles={roles} handoff={handoff} error={sceneError} onPlan={() => setTab("tasks")} />}
        </section>
        <div className="workbench-feed" hidden={tab === "tasks" || tab === "council"}><RunFeed onPlan={() => setTab("tasks")} /></div>
        <div className="workbench-board" hidden={tab !== "tasks"}>{camp && <TaskBoard key={camp.id} campId={camp.id} />}</div>
        <div className="workbench-council" hidden={tab !== "council"}>{camp && <CouncilPanel key={`council-${camp.id}`} campId={camp.id} ready={listening} onPlan={() => setTab("tasks")} />}</div>
        <div className="workbench-composer" hidden={tab === "tasks" || tab === "council"}>{camp && <TaskPanel key={`task-${camp.id}`} campId={camp.id} ready={listening} onConfigure={() => openCard(roles.find((role) => role.id === selectedId) ?? null)} />}</div>
      </div>
      {camp && <AgentPanel campId={camp.id} onOpen={openCard} onPlan={() => setTab("tasks")}><BattleReportButton key={camp.id} camp={camp} snapshot={() => appRef.current?.renderer.extract.canvas({ target: appRef.current.stage }) as HTMLCanvasElement | undefined} /></AgentPanel>}
      </div>
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
