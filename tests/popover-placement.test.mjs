import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("../src/ui/components/popover-placement.ts", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } });
const exports = {};
new Function("exports", outputText)(exports);
const { placePicker } = exports;
const viewport = { width: 800, height: 600 };

test("composer picker opens above a bottom toolbar without clipping", () => {
  const result = placePicker({ left: 240, top: 540, bottom: 570 }, 250, viewport);
  assert.deepEqual(result, { left: 240, bottom: 66, width: 264, maxHeight: 330 });
  assert.ok(viewport.height - result.bottom - 250 >= 8);
});

test("picker flips below an anchor near the top and clamps its right edge", () => {
  const result = placePicker({ left: 740, top: 20, bottom: 50 }, 200, viewport);
  assert.deepEqual(result, { left: 528, top: 56, width: 264, maxHeight: 330 });
  assert.equal(result.left + result.width, viewport.width - 8);
});

test("zoomed viewports constrain picker width and height to available space", () => {
  const result = placePicker({ left: -10, top: 80, bottom: 100 }, 400, { width: 220, height: 160 });
  assert.deepEqual(result, { left: 8, bottom: 86, width: 204, maxHeight: 66 });
});
