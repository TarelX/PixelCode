import type { Camp } from "../app/store";
import type { RoleConfig } from "../camp/roster";
import type { RoleRun } from "../camp/run";
import type { StoredPlan } from "./plan-layout";
import { roleName, type Locale, type ThemeCopy } from "../i18n";

export function reportDay(now = new Date()) {
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const end = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
  const date = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
  return { date, start: start.getTime() / 1000, end: end.getTime() / 1000 };
}

export function reportCounts(statuses: Record<string, number>) {
  const sum = (...keys: string[]) => keys.reduce((total, key) => total + (statuses[key] ?? 0), 0);
  return { done: sum("done"), running: sum("running", "checking", "reviewing"), failed: sum("failed", "blocked"), pending: sum("pending") };
}

export async function renderBattleReport(data: {
  camp: Camp; date: string; roles: RoleConfig[]; runs: Record<string, RoleRun>;
  plan: StoredPlan | null; counts: Record<string, number>; snapshot?: HTMLCanvasElement;
}, { locale, theme }: { locale: Locale; theme: ThemeCopy }): Promise<Blob> {
  await document.fonts.ready;
  const canvas = document.createElement("canvas");
  canvas.lang = locale;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error(theme.report.canvasError);
  const style = getComputedStyle(document.documentElement);
  const color = (key: string) => style.getPropertyValue(key).trim();
  const font = getComputedStyle(document.body).fontFamily;
  const width = 640;
  ctx.font = `12px ${font}`;
  const wrap = (text: string, maxWidth = 576) => {
    const lines: string[] = [];
    let line = "";
    for (const char of text) {
      if (char === "\n" || ctx.measureText(line + char).width > maxWidth) {
        lines.push(line); line = char === "\n" ? "" : char;
      } else line += char;
    }
    lines.push(line);
    return lines;
  };
  const statuses: Record<string, string> = theme.wall.statuses;
  const runStates: Record<string, string> = theme.runStates;
  const names = new Map(data.roles.map((role) => [role.id, roleName(role, locale)]));
  const rows = [
    ...wrap(data.camp.name || data.camp.root_path), ...wrap(data.camp.root_path),
    "", theme.report.today,
    ...wrap(Object.entries(reportCounts(data.counts)).map(([key, count]) => `${theme.report[key as "done" | "running" | "failed" | "pending"]}  ${count}`).join("     ")),
    ...wrap(theme.report.scope), "", theme.report.roles,
    ...(data.roles.length ? data.roles.flatMap((role) => wrap(`${names.get(role.id)}  ·  ${data.runs[role.id] ? runStates[data.runs[role.id].status] : theme.report.idle}`)) : [theme.report.emptyRoles]),
    "", theme.report.tasks,
    ...(data.plan ? data.plan.plan.tasks.flatMap((task, index) => wrap(`${index + 1}. ${task.title}  ·  ${statuses[task.status ?? "pending"] ?? task.status}  ·  ${names.get(task.role_id) ?? task.role_id}`)) : wrap(theme.wall.empty)),
  ];
  const imageHeight = data.snapshot ? 196 : 0;
  const height = 120 + imageHeight + rows.length * 22;
  canvas.width = width * 2;
  canvas.height = height * 2;
  ctx.scale(2, 2);
  ctx.imageSmoothingEnabled = false;
  ctx.fillStyle = color("--ui-bg"); ctx.fillRect(0, 0, width, height);
  ctx.fillStyle = color("--ui-accent"); ctx.fillRect(16, 16, width - 32, 4);
  ctx.fillStyle = color("--text-light"); ctx.font = `22px ${font}`;
  ctx.fillText(theme.report.title, 32, 56);
  ctx.fillStyle = color("--ui-muted"); ctx.font = `12px ${font}`;
  ctx.fillText(`PIXELCODE  /  ${data.date}`, 32, 80);
  if (data.snapshot) ctx.drawImage(data.snapshot, 160, 96, 320, 180);
  rows.forEach((line, index) => {
    ctx.fillStyle = [theme.report.today, theme.report.roles, theme.report.tasks].includes(line) ? color("--ui-accent") : color("--text-light");
    ctx.fillText(line, 32, 110 + imageHeight + index * 22);
  });
  ctx.fillStyle = color("--ui-border"); ctx.fillRect(16, height - 16, width - 32, 2);
  return new Promise((resolve, reject) => canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error(theme.report.pngError)), "image/png"));
}
