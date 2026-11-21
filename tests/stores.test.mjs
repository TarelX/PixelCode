import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";
import { create } from "zustand";
import { createStore } from "zustand/vanilla";

async function loadModule(path, imports) {
  const source = await readFile(new URL(`../${path}`, import.meta.url), "utf8");
  const result = {};
  new Function("require", "exports", ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } }).outputText)((id) => imports[id], result);
  return result;
}

function backend(handlers) {
  const calls = [];
  return { calls, invoke: (name, args) => { calls.push({ name, args }); return handlers[name](args); } };
}

const deferred = () => { let resolve; const promise = new Promise((done) => { resolve = done; }); return { promise, resolve }; };

test("CLI detection refreshes once at a time and reports command errors", async () => {
  let reply = deferred();
  const api = backend({ detect_clis: () => reply.promise });
  const hooklessCreate = (init) => { const store = createStore(init); return Object.assign(() => store.getState(), store); };
  const { useCliDetection } = await loadModule("src/app/cli-detection.ts", {
    zustand: { create: hooklessCreate },
    "@tauri-apps/api/core": { invoke: api.invoke }, react: { useEffect: (effect) => effect() },
  });
  useCliDetection(true);
  assert.equal(useCliDetection().loading, true);
  await useCliDetection().refresh();
  assert.equal(api.calls.length, 1);
  const results = [{ id: "claude-code", status: "found", version: "2.1.330", error: null }];
  reply.resolve(results);
  await reply.promise;
  await new Promise((done) => setImmediate(done));
  assert.deepEqual(useCliDetection().results, results);
  assert.equal(useCliDetection().loading, false);
  reply = { promise: Promise.reject("无法启动 Claude Code：未找到 claude 命令") };
  await useCliDetection().refresh();
  assert.deepEqual(useCliDetection().results, []);
  assert.equal(useCliDetection().error, "无法启动 Claude Code：未找到 claude 命令");
});

test("roster ignores stale loads and keeps selection after save and delete", async () => {
  const pending = [];
  let roles = [{ id: "captain" }, { id: "custom" }];
  const api = backend({
    list_roles: () => { const reply = deferred(); pending.push(reply); return reply.promise; },
    list_skills: async () => [{ id: "api-design-conventions", summary: "API" }],
    save_role: async () => {},
    delete_role: async ({ id }) => { roles = roles.filter((role) => role.id !== id); },
  });
  const { useRoster } = await loadModule("src/camp/roster.ts", { "@tauri-apps/api/core": { invoke: api.invoke }, zustand: { create } });
  const roster = () => useRoster.getState();
  const first = roster().load("camp-a");
  const second = roster().load("camp-b");
  pending[1].resolve([{ id: "b" }]);
  pending[0].resolve([{ id: "a" }]);
  await Promise.all([first, second]);
  assert.deepEqual(roster().roles, [{ id: "b" }]);

  const saving = roster().save({ id: "custom" }, "camp-b");
  await new Promise((done) => setImmediate(done));
  pending[2].resolve(roles);
  await saving;
  assert.equal(roster().selectedId, "custom");
  const removing = roster().remove("custom", "camp-b");
  await new Promise((done) => setImmediate(done));
  pending[3].resolve(roles);
  await removing;
  assert.equal(roster().selectedId, null);
  assert.deepEqual(roster().roles, [{ id: "captain" }]);

  const failing = roster().load("camp-c");
  pending[4].promise.catch(() => {});
  pending[4].resolve(Promise.reject("无法读取角色配置"));
  await assert.rejects(failing);
  assert.equal(roster().error, "无法读取角色配置");

  await roster().loadSkills();
  await roster().loadSkills();
  assert.equal(api.calls.filter((call) => call.name === "list_skills").length, 1);
});

test("app store switches between the title screen and an opened camp", async () => {
  const { useAppStore } = await loadModule("src/app/store.ts", { zustand: { create } });
  const camp = { id: "c", name: "camp", root_path: "/camp", created_at: 1, last_opened_at: 2 };
  useAppStore.getState().openCamp(camp);
  assert.equal(useAppStore.getState().screen, "camp");
  assert.equal(useAppStore.getState().camp, camp);
  useAppStore.getState().showTitle();
  assert.equal(useAppStore.getState().screen, "title");
});
