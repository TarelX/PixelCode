import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("../src/camp/bubble.ts", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } });
const exports = {};
new Function("exports", outputText)(exports);
const { bubblePosition } = exports;
const canvas = { left: 40, top: 30, width: 640, height: 360 };

test("uses space above a general or places the bubble below top-row generals", () => {
  const above = bubblePosition({ x: 320, y: 260, footY: 312, canvas }, 100);
  assert.equal(above.below, false);
  assert.equal(above.top, canvas.top + 160);
  const below = bubblePosition({ x: 320, y: 44, footY: 96, canvas }, 100);
  assert.equal(below.below, true);
  assert.equal(below.top, canvas.top + 104);
  assert.equal(below.left + below.tail + 4, canvas.left + 320);
});

test("clamps both edges and stays within a small canvas", () => {
  for (const x of [24, 616]) {
    const position = bubblePosition({ x, y: 44, footY: 96, canvas }, 100);
    assert.ok(position.left >= canvas.left + 8);
    assert.ok(position.left + position.width <= canvas.left + canvas.width - 8);
    assert.equal(position.left + position.tail + 4, canvas.left + x);
  }
  const small = { left: 0, top: 0, width: 160, height: 180 };
  const position = bubblePosition({ x: 80, y: 22, footY: 48, canvas: small }, 150);
  assert.equal(position.width, 144);
  assert.ok(position.top >= 8);
  assert.ok(position.top + 150 <= 172);
});
