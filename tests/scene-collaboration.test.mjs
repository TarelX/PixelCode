import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

async function load(path, dependencies = {}) {
  const source = await readFile(new URL(path, import.meta.url), "utf8");
  const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } });
  const exports = {};
  new Function("require", "exports", outputText)((id) => dependencies[id], exports);
  return exports;
}
const model = await load("../src/camp/collaboration.ts");
const { planHandoffs, roleActivity, advanceRoute, handoffRoute } = model;
const { SceneMotion } = await load("../src/camp/scene-motion.ts", { "./collaboration": model });
const { layoutSceneLabels } = await load("../src/camp/scene-labels.ts");
const task = (id, role_id, status, depends_on = []) => ({ id, role_id, title: id, status, depends_on, input_artifacts: ["contract.md"], output_artifacts: ["contract.md"] });
const plan = (tasks, status = "executing") => ({ id: "plan", status, plan: { tasks } });

test("only completed cross-role dependencies produce handoffs; fan-in waits for every parent", () => {
  const tasks = [task("design", "architect", "done"), task("api", "backend", "running"),
    task("ui", "frontend", "pending", ["design", "api"]), task("docs", "architect", "pending", ["design"])];
  let [handoff] = planHandoffs(plan(tasks));
  assert.equal(handoff.from.id, "design");
  assert.equal(handoff.to.id, "ui");
  assert.equal(handoff.ready, false);
  assert.deepEqual(handoff.artifacts, ["contract.md"]);
  tasks[1].status = "done";
  assert.equal(planHandoffs(plan(tasks)).length, 2);
  assert.ok(planHandoffs(plan(tasks)).every((item) => item.ready));
  assert.deepEqual(planHandoffs(plan(tasks, "draft")), []);
  assert.deepEqual(planHandoffs(plan(tasks, "aborted")), []);
});

test("task cards choose real active work, pending before old done work, and direct prompts", () => {
  const snapshot = plan([task("old", "frontend", "done"), task("next", "frontend", "pending"), task("active", "frontend", "checking")]);
  assert.equal(roleActivity("frontend", snapshot).title, "active");
  snapshot.plan.tasks.pop();
  assert.equal(roleActivity("frontend", snapshot).title, "next");
  assert.equal(roleActivity("frontend", snapshot, { status: "running", prompt: "New direct task" }).title, "New direct task");
  assert.equal(roleActivity("unassigned", snapshot).status, "idle");
  assert.equal(roleActivity("frontend", { ...snapshot, status: "draft" }).status, "draft");
  snapshot.plan.tasks[1].status = "reviewing";
  assert.equal(roleActivity("reviewer", snapshot, undefined, true).title, "next");
});

test("routes pass through configured corridor, return home, and consume distance without overshoot", () => {
  const home = { x: 48, y: 144 }, target = { x: 160, y: 48 };
  const route = handoffRoute(home, target, { x: 80, y: 144 }, { x: 200, y: 48 }, 94);
  assert.deepEqual(route[1], { x: 80, y: 94 });
  assert.deepEqual(route[4], target);
  assert.deepEqual(route[9], home);
  assert.deepEqual(advanceRoute(home, route, 10), { x: 58, y: 144 });
  assert.equal(route.length, 10);
  assert.deepEqual(advanceRoute({ x: 58, y: 144 }, route, 10000), home);
  assert.deepEqual(route, []);
});

function motion() {
  const homes = { architect: { x: 48, y: 144 }, frontend: { x: 144, y: 144 } };
  const generals = Object.keys(homes).map((id) => {
    const general = { role: { id }, ...homes[id], state: "idle", setState(state) { this.state = state; } };
    general.position = { set(x, y) { general.x = x; general.y = y; } };
    return general;
  });
  return { generals, controller: new SceneMotion(generals, homes, { corridorY: 94, exits: { architect: { x: 80, y: 144 }, frontend: { x: 192, y: 144 } } }) };
}
test("scene walks on new completed edges once, never replays historical transfers, and honors reduced motion", () => {
  const snapshot = plan([task("contract", "architect", "running"), task("ui", "frontend", "pending", ["contract"])]);
  const { controller, generals } = motion();
  controller.update(snapshot, {});
  snapshot.plan.tasks[0].status = "done";
  controller.update(snapshot, {});
  controller.tick(.05, false);
  assert.equal(controller.active.to.id, "ui");
  assert.ok(generals[0].x > 48);
  assert.equal(generals[0].state, "running");
  for (let i = 0; i < 1000; i++) controller.tick(.05, false);
  assert.equal(controller.active, null);
  assert.equal(generals[0].x, 48);
  controller.update(snapshot, {});
  controller.tick(.05, false);
  assert.equal(controller.active, null);
  const historical = motion();
  historical.controller.update(snapshot, {});
  historical.controller.tick(.05, false);
  assert.equal(historical.controller.active, null);
  snapshot.plan.tasks[0].status = "running";
  const reduced = motion();
  reduced.controller.update(snapshot, {});
  snapshot.plan.tasks[0].status = "done";
  reduced.controller.update(snapshot, {});
  reduced.controller.tick(.05, true);
  assert.equal(reduced.controller.active, null);
  assert.equal(reduced.generals[0].x, 48);
});

test("moving and adjacent labels stay readable and inside the scene", () => {
  for (const width of [320, 640, 960]) {
    for (let step = 0; step < 30; step++) {
      const labels = Array.from({ length: 7 }, (_, i) => ({ id: String(i),
        left: (i === 4 ? step / 30 : i / 7) * width, top: i < 3 ? 4 : 180, width: Math.min(132, width / 4), height: 40 }));
      const placed = layoutSceneLabels(labels, width, 360);
      for (const [i, card] of placed.entries()) {
        assert.ok(card.left >= 4 && card.left + card.width <= width - 4);
        assert.ok(card.top >= 4 && card.top + card.height <= 356);
        for (const other of placed.slice(i + 1)) assert.ok(card.left + card.width <= other.left || other.left + other.width <= card.left
          || card.top + card.height <= other.top || other.top + other.height <= card.top);
      }
    }
  }
});
