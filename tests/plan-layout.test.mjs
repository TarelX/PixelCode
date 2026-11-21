import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("../src/ui/plan-layout.ts", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } });
const exports = {};
new Function("exports", outputText)(exports);
const { layoutPlan } = exports;
const fixture = JSON.parse(await readFile(new URL("./scheduler/captain-plan.json", import.meta.url), "utf8"));

test("lays out an out-of-order DAG with stable rows and integer positions", () => {
  const [fix, reproduce, peer] = layoutPlan([...fixture.tasks, { id: "peer", depends_on: [] }]);
  assert.ok(reproduce.x < fix.x);
  assert.equal(peer.x, reproduce.x);
  assert.ok(peer.y > reproduce.y);
  assert.ok([fix, reproduce, peer].every(({ x, y }) => Number.isInteger(x) && Number.isInteger(y)));
});
test("handles empty plans and rejects missing dependencies and cycles", () => {
  assert.deepEqual(layoutPlan([]), []);
  assert.throws(() => layoutPlan([{ id: "a", depends_on: ["missing"] }]));
  assert.throws(() => layoutPlan([{ id: "a", depends_on: ["b"] }, { id: "b", depends_on: ["a"] }]));
});
