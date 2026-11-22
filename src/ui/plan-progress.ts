import type { StoredPlan } from "./plan-layout";

export function planProgress(plan: StoredPlan) {
  const total = plan.plan.tasks.length;
  const done = plan.plan.tasks.filter((task) => task.status === "done").length;
  const phase = plan.status === "draft" ? "draft"
    : plan.status === "aborted" ? "aborted"
    : total > 0 && done === total ? "done"
    : plan.status === "executing" ? "running" : "notStarted";
  return { total, done, phase } as const;
}
