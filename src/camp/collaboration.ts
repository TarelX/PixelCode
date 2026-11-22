import type { RoleRun } from "./run";
import type { PlanTask, StoredPlan } from "../ui/plan-layout";

export interface Handoff {
  id: string;
  from: PlanTask;
  to: PlanTask;
  artifacts: string[];
  ready: boolean;
}

export function planHandoffs(plan: StoredPlan | null): Handoff[] {
  if (!plan || ["draft", "aborted"].includes(plan.status)) return [];
  const tasks = plan.plan.tasks;
  return tasks.flatMap((to) => to.depends_on.flatMap((id) => {
    const from = tasks.find((task) => task.id === id);
    if (!from || from.status !== "done" || from.role_id === to.role_id) return [];
    return [{ id: `${plan.id}:${id}:${to.id}`, from, to,
      artifacts: from.output_artifacts.filter((path) => to.input_artifacts.includes(path)),
      ready: to.depends_on.every((parent) => tasks.find((task) => task.id === parent)?.status === "done"),
    }];
  }));
}

export function roleActivity(id: string, plan: StoredPlan | null, run?: RoleRun, reviewer = false) {
  const tasks = plan?.plan.tasks.filter((task) => task.role_id === id) ?? [];
  const task = tasks.find((task) => ["running", "checking", "reviewing", "blocked", "failed"].includes(task.status ?? ""))
    ?? tasks.find((task) => !task.status || task.status === "pending") ?? tasks[tasks.length - 1];
  // A direct run started after a plan must not be hidden behind its old task.
  if (run && (run.status === "running" || run.status === "waiting" || !task)) {
    return { title: run.prompt || run.tool, status: run.status, detail: run.error || run.tool };
  }
  const review = reviewer && plan?.plan.tasks.find((task) => task.status === "reviewing");
  if (review) return { title: review.title, status: "reviewing", detail: "" };
  return { title: task?.title ?? "", status: plan?.status === "draft" ? "draft" : task?.status ?? "idle", detail: "" };
}

export interface Point { x: number; y: number }

export function handoffRoute(from: Point, to: Point, fromExit: Point, toExit: Point, corridorY: number): Point[] {
  return [fromExit, { x: fromExit.x, y: corridorY }, { x: toExit.x, y: corridorY }, toExit, to,
    toExit, { x: toExit.x, y: corridorY }, { x: fromExit.x, y: corridorY }, fromExit, from];
}

export function advanceRoute(position: Point, route: Point[], distance: number): Point {
  let { x, y } = position;
  while (route.length) {
    const next = route[0];
    const length = Math.hypot(next.x - x, next.y - y);
    if (length > distance) {
      x += (next.x - x) * distance / length;
      y += (next.y - y) * distance / length;
      break;
    }
    x = next.x; y = next.y; distance -= length; route.shift();
  }
  return { x, y };
}
