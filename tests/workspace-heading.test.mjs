import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { createElement } from "react";
import * as jsx from "react/jsx-runtime";
import { renderToStaticMarkup } from "react-dom/server";
import ts from "typescript";

async function load(path, dependencies = {}) {
  const source = await readFile(new URL(path, import.meta.url), "utf8");
  const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX } });
  const exports = {};
  new Function("require", "exports", outputText)((id) => dependencies[id], exports);
  return exports;
}
const roles = [{ id: "designer", role_type: "designer" }, { id: "backend", role_type: "backend" }, { id: "reviewer", role_type: "reviewer" }];
let runs = {};
const { WorkspaceHeading } = await load("../src/ui/WorkspaceHeading.tsx", {
  "react/jsx-runtime": jsx,
  "../app/store": { useAppStore: (select) => select({ camp: { name: "QA" } }) },
  "../camp/roster": { useRoster: (select) => select({ roles }) },
  "../camp/run": { useRuns: (select) => select({ runs }) },
  "../camp/collaboration": await load("../src/camp/collaboration.ts"),
  "../i18n": { useI18n: () => ({ theme: { workspace: { title: "Workspace" }, workbench: { active: "running", agents: "agents" } } }) },
  "./components/PixelIcon": { PixelIcon: () => null },
});
const plan = (statuses) => ({ status: "executing", plan: { tasks: statuses.map((status) => ({ role_id: "backend", title: "Task", status })) } });
function count(snapshot) {
  const html = renderToStaticMarkup(createElement(WorkspaceHeading, { plan: snapshot }));
  return Number(html.match(/<strong>(\d+)<\/strong>running/)[1]);
}

test("workspace counter includes scheduled work and review, then returns idle", () => {
  runs = {};
  assert.equal(count(plan(["running"])), 1);
  assert.equal(count(plan(["checking"])), 1);
  assert.equal(count(plan(["reviewing"])), 2);
  assert.equal(count(plan(["done"])), 0);
  assert.equal(count(plan(["failed"])), 0);
});

test("workspace counter counts agents once across multiple tasks and direct runs", () => {
  runs = { backend: { status: "running" }, designer: { status: "waiting" }, foreign: { status: "running" } };
  assert.equal(count(plan(["running", "checking"])), 2);
  assert.equal(count(null), 2);
  runs = { backend: { status: "completed" } };
  assert.equal(count(plan(["done"])), 0);
});
