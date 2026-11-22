import { Container, Graphics } from "pixi.js";
import type { Point } from "./collaboration";
import type { GeneralSprite } from "./sprites";
import type { SceneMotion } from "./scene-motion";
import { workPose, type WorkPose } from "./work-actions";

export interface Workstation { seat: Point; screen: Point; keyboard: Point; hands: string }
export interface WorkstationLayout {
  palette: { shell: string; screen: string; key: string; code: string; muted: string; checking: string; review: string; waiting: string; error: string };
  stations: Record<string, Workstation>;
}

export class Workstations extends Container {
  private displays = new Map<string, Graphics>();
  private elapsed = 0;
  private frame = -1;

  constructor(private config: WorkstationLayout) {
    super();
    this.eventMode = "none";
    for (const [id, station] of Object.entries(config.stations)) {
      const { screen: s, keyboard: k } = station;
      const p = config.palette;
      const base = new Graphics()
        .rect(s.x - 1, s.y - 1, 16, 11).fill(p.shell)
        .rect(s.x + 5, s.y + 10, 4, 2).rect(s.x + 3, s.y + 12, 8, 1).fill(p.shell)
        .rect(k.x - 1, k.y - 1, 14, 6).fill(p.shell)
        .rect(k.x, k.y, 12, 4).fill(p.muted);
      for (let row = 0; row < 2; row++) for (let key = 0; key < 4; key++) base.rect(k.x + 1 + key * 3, k.y + row * 2, 2, 1).fill(p.key);
      this.addChild(base);
      const display = new Graphics();
      this.displays.set(id, display);
      this.addChild(display);
    }
  }

  tick(seconds: number, motion: SceneMotion, generals: GeneralSprite[], reduced: boolean) {
    this.elapsed += Math.min(seconds, .05);
    const frame = Math.floor(this.elapsed * 12);
    if (frame === this.frame) return;
    this.frame = frame;
    generals.forEach((general, index) => {
      const state = motion.actions.get(general.role.id);
      if (!state || !this.displays.has(general.role.id)) return;
      this.draw(general.role.id, workPose(state.kind, state.seconds, index, reduced), !state.walking);
    });
  }

  private draw(id: string, pose: WorkPose, atDesk: boolean) {
    const g = this.displays.get(id)!.clear();
    const { screen: s, keyboard: k, seat, hands } = this.config.stations[id];
    const p = this.config.palette;
    const { action, frame } = pose;
    g.rect(s.x, s.y, 14, 9).fill(p.screen);
    const line = (x: number, y: number, width: number, color = p.code) => g.rect(s.x + x, s.y + y, width, 1).fill(color);
    if (action === "typing") {
      const scroll = frame % 4;
      for (let row = 0; row < 3; row++) {
        line(1, 1 + row * 2, 2, p.review);
        line(4, 1 + row * 2, [7, 4, 6, 3][(row + scroll) % 4]);
      }
      line(4 + frame % 6, 7, 1, p.key);
    } else if (action === "checking") {
      for (let row = 0; row < 3; row++) {
        line(1, 1 + row * 3, 1, p.muted);
        line(3, 1 + row * 3, 7, p.muted);
      }
      line(1, (frame % 3) * 3 + 1, 11, p.checking);
    } else if (action === "reviewing") {
      for (let row = 0; row < 3; row++) line(1, 1 + row * 2, 7, p.muted);
      line(1, (frame % 3) * 2 + 1, 7, p.review);
      const x = s.x + 9, y = s.y + 3;
      g.rect(x, y, 3, 1).rect(x, y + 3, 3, 1).rect(x - 1, y + 1, 1, 2).rect(x + 3, y + 1, 1, 2).rect(x + 3, y + 4, 1, 1).fill(p.review);
    } else if (action === "done") {
      g.rect(s.x + 3, s.y + 4, 2, 2).rect(s.x + 5, s.y + 6, 2, 1).rect(s.x + 7, s.y + 4, 2, 2).rect(s.x + 9, s.y + 2, 2, 2).fill(p.code);
    } else if (action === "error") {
      for (let i = 0; i < 5; i++) g.rect(s.x + 4 + i, s.y + 2 + i, 1, 1).rect(s.x + 8 - i, s.y + 2 + i, 1, 1).fill(p.error);
    } else if (action === "waiting") {
      for (let i = 0; i < 3; i++) line(3 + i * 3, 4, 1, p.waiting);
    } else {
      line(2, 2, 6, p.muted); line(2, 4, 9, p.muted);
    }
    if (atDesk && ["typing", "checking", "reviewing"].includes(action)) {
      // Attached forearms reach from the visible torso to the keys, not floating dots below a monitor.
      for (const [shoulder, wrist, tap, bend] of [[seat.x - 4 + pose.lean, k.x + 1, pose.left, -1], [seat.x + 3 + pose.lean, k.x + 8, pose.right, 1]]) {
        const top = seat.y - 5 + pose.nod, bottom = k.y - 1 + tap;
        const joints = [{ x: shoulder, y: top }, { x: shoulder + bend, y: top + 2 }, { x: wrist, y: bottom }];
        const pixels: Point[] = [];
        for (let i = 1; i < joints.length; i++) {
          const a = joints[i - 1], b = joints[i], steps = Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y), 1);
          for (let step = 0; step <= steps; step++) pixels.push({ x: Math.round(a.x + (b.x - a.x) * step / steps), y: Math.round(a.y + (b.y - a.y) * step / steps) });
        }
        for (const point of pixels) g.rect(point.x - 1, point.y - 1, 4, 4).fill(p.shell);
        for (const point of pixels) g.rect(point.x, point.y, 2, 2).fill(hands);
        g.rect(wrist, bottom, 3, 2).fill(hands);
      }
      if (pose.left) g.rect(k.x + 2, k.y + 3, 2, 1).fill(p.code);
      if (pose.right) g.rect(k.x + 9, k.y + 3, 2, 1).fill(p.code);
    }
    // A desk light, not a progress bar: it only reports the scheduler's current state.
    const light = action === "error" ? p.error : action === "checking" ? p.checking : action === "reviewing" ? p.review
      : action === "waiting" ? p.waiting : action === "idle" || action === "stopped" ? p.muted : p.code;
    g.rect(s.x + 12, s.y + 10, 1, 1).fill(light);
  }
}
