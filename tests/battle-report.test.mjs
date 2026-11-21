import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("../src/ui/battle-report.ts", import.meta.url), "utf8");
const theme = JSON.parse(await readFile(new URL("../themes/three-kingdoms/ui.json", import.meta.url), "utf8"));
const exports = {};
new Function("require", "exports", ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } }).outputText)(() => theme, exports);

test("report uses local calendar boundaries and groups all task states", () => {
  const day = exports.reportDay(new Date(2026, 10, 21, 23, 59));
  assert.equal(day.date, "2026-11-21");
  assert.equal(day.start, new Date(2026, 10, 21).getTime() / 1000);
  assert.equal(day.end, new Date(2026, 10, 22).getTime() / 1000);
  assert.deepEqual(exports.reportCounts({ done: 2, checking: 1, reviewing: 1, running: 1, blocked: 2, failed: 1, pending: 4 }), { done: 2, running: 3, failed: 3, pending: 4 });
  assert.deepEqual(exports.reportCounts({}), { done: 0, running: 0, failed: 0, pending: 0 });
});

test("renders populated and empty reports, propagates canvas and encoding failures", async () => {
  const text = [];
  let context = true, encode = true;
  const ctx = { measureText: (value) => ({ width: value.length * 12 }), scale() {}, fillRect() {}, drawImage() {}, fillText: (value) => text.push(value) };
  globalThis.document = { fonts: { ready: Promise.resolve() }, documentElement: {}, body: {}, createElement: () => ({ getContext: () => context ? ctx : null, toBlob: (callback) => callback(encode ? new Blob(["png"]) : null) }) };
  globalThis.getComputedStyle = () => ({ fontFamily: "monospace", getPropertyValue: () => "#123456" });
  const data = { camp: { name: "Test camp", root_path: "/camp" }, date: "2026-11-21", roles: [], runs: {}, plan: null, counts: {} };
  assert.ok(await exports.renderBattleReport(data) instanceof Blob);
  assert.ok(text.includes(theme.report.emptyRoles));
  await exports.renderBattleReport({ ...data, roles: [{ id: "a", name: "General" }], runs: { a: { status: "running" } }, plan: { plan: { tasks: [{ title: "Implement", role_id: "a", status: "done" }] } } });
  assert.ok(text.some((line) => line.includes("General") && line.includes(theme.runStates.running)));
  assert.ok(text.some((line) => line.includes("Implement") && line.includes(theme.wall.statuses.done)));
  encode = false;
  await assert.rejects(exports.renderBattleReport(data), { message: theme.report.pngError });
  context = false;
  await assert.rejects(exports.renderBattleReport(data), { message: theme.report.canvasError });
});
