import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { create } from "zustand";
import ts from "typescript";

async function store() {
  let callback;
  let failure;
  let request;
  let result;
  const source = await readFile(new URL("../src/camp/run.ts", import.meta.url), "utf8");
  const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } });
  const exports = {};
  const dependencies = {
    zustand: { create },
    "@tauri-apps/api/core": { invoke: async (name, args) => { request = { name, args }; if (failure) throw failure; return result; } },
    "@tauri-apps/api/event": { listen: async (_, receive) => { callback = receive; return () => {}; } },
  };
  new Function("require", "exports", outputText)((id) => dependencies[id], exports);
  await exports.listenToRuns("camp");
  return {
    state: () => exports.useRuns.getState(),
    event: (type, rest = {}) => callback({ payload: { camp_id: "camp", role_id: "role", run_id: "run-1", type, ...rest } }),
    fail: (error) => { failure = error; },
    reply: (value) => { result = value; },
    request: () => request,
  };
}

test("captain can pause for questions and resume with a delegated answer", async () => {
  const run = await store();
  const questions = ["Which browser?"];
  run.reply({ questions, pending_id: "pending-1" });
  assert.deepEqual(await run.state().start("camp", "captain", 0, "Fix login"), { questions, pending_id: "pending-1" });
  run.event("started");
  run.event("waiting");
  assert.equal(run.state().runs.role.status, "waiting");
  assert.equal(run.state().runs.role.artifact, undefined);
  run.reply(null);
  assert.equal(await run.state().start("camp", "captain", 0, "Fix login", { questions, answer: null }, "pending-1", "key-1"), null);
  assert.deepEqual(run.request(), {
    name: "start_role_run",
    args: { campId: "camp", roleId: "captain", loadoutIndex: 0, prompt: "Fix login", clarification: { questions, answer: null }, pendingAnswer: { id: "pending-1", key: "key-1" } },
  });
});

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
  run.event("text", { run_id: "run-2", text: "stale" });
  assert.equal(run.state().runs.role.text, "Reading notes");
  assert.equal(run.state().runs.role.tool, "Read");
  run.event("completed");
  assert.equal(run.state().runs.role.status, "completed");
  run.state().dismiss("role");
  assert.deepEqual(run.state().runs, {});
});

test("usage limit and turn metrics update the run without changing its state", async () => {
  const run = await store();
  run.event("started");
  run.event("usage_limited", { resets_at: 1700000060 });
  run.event("turn_metrics", { input_tokens: 12, output_tokens: 4, cache_tokens: 5 });
  assert.equal(run.state().runs.role.status, "running");
  assert.deepEqual(run.state().runs.role.usage_limited, { resets_at: 1700000060 });
  assert.deepEqual(run.state().runs.role.turn_metrics, { input_tokens: 12, output_tokens: 4, cache_tokens: 5 });
  run.event("turn_metrics", { input_tokens: 13, output_tokens: 4 });
  assert.deepEqual(run.state().runs.role.turn_metrics, { input_tokens: 13, output_tokens: 4, cache_tokens: null });
});

test("failed and stopped states, bounded text, and command errors", async () => {
  const run = await store();
  run.event("started");
  run.event("text", { text: "a".repeat(40000) });
  assert.equal(run.state().runs.role.text.length, 32000);
  run.event("failed", { message: "CLI unavailable" });
  assert.equal(run.state().runs.role.error, "CLI unavailable");
  assert.equal(run.state().runs.role.status, "failed");
  run.event("started", { run_id: "run-2" });
  run.event("stopped", { run_id: "run-2" });
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

test("blocked state carries the reason and stays visible", async () => {
  const run = await store();
  run.event("started");
  run.event("text", { text: "分析完成。\nBLOCKED: 需要 git push 权限" });
  run.event("blocked", { message: "需要 git push 权限" });
  assert.equal(run.state().runs.role.status, "blocked");
  assert.equal(run.state().runs.role.error, "需要 git push 权限");
});

test("completion retains task and artifact metadata without leaking it into a new run", async () => {
  const run = await store();
  const artifact = { id: "artifact-1", file_path: "camp/task/output.md", absolute_path: "local-output.md" };
  run.event("started", { task_id: "task" });
  run.event("completed", { task_id: "task", artifact });
  assert.equal(run.state().runs.role.task_id, "task");
  assert.deepEqual(run.state().runs.role.artifact, artifact);
  run.event("started", { run_id: "run-2", task_id: "next-task" });
  assert.equal(run.state().runs.role.artifact, undefined);
  run.event("failed", { run_id: "run-2", message: "无法保存产物记录" });
  assert.equal(run.state().runs.role.artifact, undefined);
  assert.equal(run.state().runs.role.error, "无法保存产物记录");
});
