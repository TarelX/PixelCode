import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { create } from "zustand";
import ts from "typescript";

async function store() {
  let callback;
  let failure;
  const source = await readFile(new URL("../src/camp/run.ts", import.meta.url), "utf8");
  const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } });
  const exports = {};
  const dependencies = {
    zustand: { create },
    "@tauri-apps/api/core": { invoke: async () => { if (failure) throw failure; } },
    "@tauri-apps/api/event": { listen: async (_, receive) => { callback = receive; return () => {}; } },
  };
  new Function("require", "exports", outputText)((id) => dependencies[id], exports);
  await exports.listenToRuns("camp");
  return {
    state: () => exports.useRuns.getState(),
    event: (type, rest = {}) => callback({ payload: { camp_id: "camp", role_id: "role", run_id: 1, type, ...rest } }),
    fail: (error) => { failure = error; },
  };
}

test("stream accumulation, tool, completion, dismissal, and stale or other-camp events", async () => {
  const run = await store();
  run.event("started", { camp_id: "other" });
  run.event("text", { text: "orphan" });
  assert.deepEqual(run.state().runs, {});
  run.event("started");
  assert.equal(run.state().runs.role.text, "");
  run.event("text", { text: "Reading " });
  run.event("text", { text: "notes" });
  run.event("tool", { name: "Read" });
  run.event("text", { run_id: 2, text: "stale" });
  assert.equal(run.state().runs.role.text, "Reading notes");
  assert.equal(run.state().runs.role.tool, "Read");
  run.event("completed");
  assert.equal(run.state().runs.role.status, "completed");
  run.state().dismiss("role");
  assert.deepEqual(run.state().runs, {});
});

test("failed and stopped states, bounded text, and command errors", async () => {
  const run = await store();
  run.event("started");
  run.event("text", { text: "a".repeat(40000) });
  assert.equal(run.state().runs.role.text.length, 32000);
  run.event("failed", { message: "CLI unavailable" });
  assert.equal(run.state().runs.role.error, "CLI unavailable");
  assert.equal(run.state().runs.role.status, "failed");
  run.event("started", { run_id: 2 });
  run.event("stopped", { run_id: 2 });
  assert.equal(run.state().runs.role.status, "stopped");
  assert.equal(run.state().runs.role.error, "");
  run.fail("Cannot launch CLI");
  await run.state().start("camp", "role", 0, "task");
  assert.equal(run.state().error, "Cannot launch CLI");
  assert.equal(run.state().busy, false);
  run.fail("Cannot stop CLI");
  await run.state().stop();
  assert.equal(run.state().error, "Cannot stop CLI");
});
