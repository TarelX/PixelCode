import type { GeneralSprite } from "./sprites";
import type { GeneralState } from "./animations";
import type { RoleRun } from "./run";
import type { StoredPlan } from "../ui/plan-layout";
import { advanceRoute, handoffRoute, planHandoffs, roleActivity, type Handoff, type Point } from "./collaboration";

export interface Navigation { corridorY: number; exits: Record<string, Point> }

export class SceneMotion {
  active: Handoff | null = null;
  private planId = "";
  private seen = new Set<string>();
  private queue: Handoff[] = [];
  private routes = new Map<string, Point[]>();
  private positions = new Map<string, Point>();
  private states = new Map<string, GeneralState>();
  private elapsed = 0;

  constructor(private generals: GeneralSprite[], private homes: Record<string, Point>, private navigation?: Navigation) {
    for (const general of generals) {
      this.positions.set(general.role.id, { x: general.x, y: general.y });
      this.homes[general.role.id] ??= { x: general.x, y: general.y };
    }
  }

  update(plan: StoredPlan | null, runs: Record<string, RoleRun>) {
    const transfers = planHandoffs(plan);
    if (plan?.id !== this.planId) {
      this.planId = plan?.id ?? "";
      this.seen = new Set(transfers.map((transfer) => transfer.id));
      this.queue = []; this.active = null; this.routes.clear();
      for (const general of this.generals) this.positions.set(general.role.id, { ...this.homes[general.role.id] });
    }
    for (const transfer of transfers) {
      if (!this.seen.has(transfer.id)) this.queue.push(transfer);
      this.seen.add(transfer.id);
    }
    if (plan?.status === "aborted") { this.queue = []; this.active = null; this.routes.clear(); }
    for (const general of this.generals) {
      const { status } = roleActivity(general.role.id, plan, runs[general.role.id], general.role.role_type === "reviewer");
      const state: GeneralState = ["running", "checking", "reviewing"].includes(status) ? "running"
        : ["done", "completed"].includes(status) ? "done" : ["failed", "blocked"].includes(status) ? "error"
          : status === "waiting" ? "waiting_user" : status === "stopped" ? "cancelled" : "idle";
      this.states.set(general.role.id, state);
    }
  }

  tick(seconds: number, reducedMotion: boolean) {
    this.elapsed += seconds;
    if (reducedMotion) { this.queue = []; this.active = null; this.routes.clear(); }
    const next = this.queue[0];
    if (!this.active && next && this.navigation) {
      const from = next.from.role_id, to = next.to.role_id;
      const exit = this.navigation.exits[from], targetExit = this.navigation.exits[to];
      if (this.homes[from] && this.homes[to] && exit && targetExit) {
        this.active = next;
        this.routes.set(from, handoffRoute(this.homes[from], this.homes[to], exit, targetExit, this.navigation.corridorY));
      }
      this.queue.shift();
    }
    for (const [index, general] of this.generals.entries()) {
      const id = general.role.id;
      const state = this.states.get(id) ?? "idle";
      let route = this.routes.get(id);
      const home = this.homes[id];
      // Working roles make short trips beside their own desk, never random map-wide wandering.
      if (!reducedMotion && !route?.length && state === "running" && this.active?.from.role_id !== id && this.navigation?.exits[id]) {
        route = [{ ...this.navigation.exits[id] }, home];
        this.routes.set(id, route);
      }
      if (state !== "running" && this.active?.from.role_id !== id) {
        route = [home]; this.routes.set(id, route);
      }
      const moving = !reducedMotion && Boolean(route?.length);
      const position = reducedMotion ? home : advanceRoute(this.positions.get(id)!, route ?? [], Math.min(seconds, .05) * 62);
      this.positions.set(id, position);
      general.position.set(Math.round(position.x), Math.round(position.y));
      const visual = moving ? "running" : state === "running" || state === "done" ? "idle" : state;
      if (general.state !== visual) general.setState(visual, index);
      general.alpha = 1;
      if (!moving && state === "running" && !reducedMotion) general.y += Math.round(Math.sin(this.elapsed * 3 + index));
    }
    if (this.active && !this.routes.get(this.active.from.role_id)?.length) this.active = null;
  }
}
