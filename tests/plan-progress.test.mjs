import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("../src/ui/plan-progress.ts", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } });
const exports = {};
new Function("exports", outputText)(exports);
const { planProgress } = exports;
const plan = (status, tasks) => ({ status, plan: { tasks: tasks.map((status) => ({ status })) } });

test("progress distinguishes approval and execution from task completion", () => {
  assert.deepEqual(planProgress(plan("draft", [undefined, undefined])), { total: 2, done: 0, phase: "draft" });
  assert.equal(planProgress(plan("approved", ["pending", "pending"])).phase, "notStarted");
  for (const active of ["running", "checking", "reviewing"]) {
    assert.deepEqual(planProgress(plan("executing", ["done", active])), { total: 2, done: 1, phase: "running" });
  }
  assert.deepEqual(planProgress(plan("awaiting_delivery", ["done", "done"])), { total: 2, done: 2, phase: "done" });
  assert.equal(planProgress(plan("completed", ["done", "done"])).phase, "done");
});

test("empty, failed and rejected plans do not claim completion", () => {
  assert.equal(planProgress(plan("approved", [])).phase, "notStarted");
  assert.equal(planProgress(plan("executing", ["done", "failed"])).phase, "running");
  assert.equal(planProgress(plan("aborted", ["done", "blocked"])).phase, "aborted");
  assert.equal(planProgress(plan("draft", ["done", "done"])).phase, "draft");
});
