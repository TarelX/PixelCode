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
const actions = await load("../src/camp/work-actions.ts");
const { SceneMotion } = await load("../src/camp/scene-motion.ts", { "./collaboration": model, "./work-actions": actions });
const { layoutSceneLabels, stationLabelRect } = await load("../src/camp/scene-labels.ts");
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
    const general = { role: { id }, ...homes[id], state: "idle", setState(state) { this.state = state; }, pose(value) { this.workPose = value; } };
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

test("work happens at the keyboard, with distinct scheduler actions and no repeated pacing", () => {
  const { controller, generals } = motion();
  const snapshot = plan([task("contract", "architect", "running")]);
  controller.update(snapshot, {});
  const taps = new Set();
  for (let i = 0; i < 200; i++) {
    controller.tick(.05, false);
    assert.equal(generals[0].x, 48);
    assert.equal(generals[0].y, 144);
    taps.add(`${generals[0].workPose.left}:${generals[0].workPose.right}`);
  }
  assert.deepEqual([...taps].sort(), ["0:0", "0:2", "2:0"]);
  for (const status of ["checking", "reviewing", "blocked", "done"]) {
    snapshot.plan.tasks[0].status = status;
    controller.update(snapshot, {});
    controller.tick(.05, false);
    assert.equal(generals[0].workPose.action, actions.workAction(status));
    assert.equal(generals[0].workPose.left + generals[0].workPose.right, 0);
  }
  controller.update(null, { architect: { status: "waiting" } });
  controller.tick(.05, false);
  assert.equal(generals[0].workPose.action, "waiting");
  const still = actions.workPose("typing", 1, 0, true);
  assert.deepEqual(still, actions.workPose("typing", 200, 5, true));
  assert.equal(still.left + still.right + still.lean + still.nod, 0);
});

test("desk name boxes fit the bottom/right edges at every integer scene scale", async () => {
  const layout = JSON.parse(await readFile(new URL("../themes/pokemon/camp.json", import.meta.url), "utf8"));
  for (const scale of [1, 2, 3, 4]) {
    const width = layout.width * scale, height = layout.height * scale;
    const seats = Object.values(layout.workstations.stations).map(({ seat }) => seat);
    const labels = seats.map((seat) => stationLabelRect(seat, width, height, layout.width, seats));
    for (const [i, label] of labels.entries()) {
      assert.ok(label.left >= 4 && label.left + label.width <= width - 3);
      assert.ok(label.top >= 4 && label.top + label.height <= height - 4);
      for (const other of labels.slice(i + 1)) assert.ok(label.left + label.width <= other.left || other.left + other.width <= label.left
        || label.top + label.height <= other.top || other.top + other.height <= label.top);
    }
    // Lucario is four CJK glyphs: do not fit the box by truncating the name.
    assert.ok(labels[seats.findIndex((seat) => seat.x === layout.workstations.stations.frontend.seat.x)].width >= 50);
  }
});

test("a stopped/failed/completed direct run stays visible after an older plan, but not a newer executing plan", () => {
  const snapshot = { ...plan([task("old", "architect", "done")], "awaiting_delivery"), created_at: 1000 };
  for (const status of ["stopped", "failed", "completed"]) {
    const direct = { task_id: "run-2000-2", prompt: "Latest direct task", status };
    assert.equal(roleActivity("architect", snapshot, direct).status, status);
    assert.equal(roleActivity("architect", snapshot, direct).title, "Latest direct task");
    assert.equal(roleActivity("architect", { ...snapshot, created_at: 3000 }, direct).status, "done");
    assert.equal(roleActivity("architect", { ...snapshot, ...plan([task("new", "architect", "checking")]) }, direct).status, "checking");
    const { controller, generals } = motion();
    controller.update(snapshot, { architect: direct });
    controller.tick(.05, false);
    assert.equal(generals[0].workPose.action, actions.workAction(status));
  }
  const aborted = { ...snapshot, status: "aborted", plan: { tasks: [task("cancelled", "architect", "running")] } };
  assert.equal(roleActivity("architect", aborted).status, "stopped");
  const { controller, generals } = motion();
  controller.update(aborted, { architect: { task_id: "run-2000-3", prompt: "New task", status: "running" } });
  controller.tick(.05, false);
  assert.equal(generals[0].workPose.action, "typing");
});

test("bespoke desk poses follow taps and reading pauses, never idle or completed travel poses", () => {
  const frames = new Set();
  for (let i = 0; i < 120; i++) frames.add(actions.workFrame(actions.workPose("typing", i / 10, 0)));
  assert.deepEqual([...frames].sort(), [0, 1, 2]);
  for (const action of ["checking", "reviewing"]) assert.equal(actions.workFrame(actions.workPose(action, 1, 0)), 3);
  for (const action of ["idle", "waiting", "done", "error", "stopped"]) assert.equal(actions.workFrame(actions.workPose(action, 1, 0)), null);
  assert.equal(actions.workFrame(actions.workPose("typing", 40, 4, true)), 0);
});
