import assert from "node:assert/strict";
import { access, readdir, readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";
import { parse } from "yaml";

const root = new URL("../", import.meta.url);
const load = (path) => readFile(new URL(path, root), "utf8");
const roles = await Promise.all((await readdir(new URL("roles/", root))).filter((file) => file.endsWith(".yaml")).map(async (file) => parse(await load(`roles/${file}`))));
const zh = JSON.parse(await load("themes/pokemon/ui.json"));
const en = JSON.parse(await load("themes/pokemon/ui.en.json"));

function shape(value, path = "") {
  if (value === null || typeof value !== "object") return [`${path}:${typeof value}`];
  if (Array.isArray(value)) return [`${path}:array`];
  return Object.entries(value).flatMap(([key, child]) => shape(child, `${path}.${key}`)).sort();
}

test("Chinese and English UI copy have the same keys and no empty strings", () => {
  assert.deepEqual(shape(en), shape(zh));
  const empty = (value, path = "") => typeof value === "string" ? (value.trim() ? [] : [path])
    : Object.entries(value ?? {}).flatMap(([key, child]) => empty(child, `${path}.${key}`));
  assert.deepEqual([...empty(zh), ...empty(en)], []);
});

test("every preset general has a sprite, a theme name and a camp post", async () => {
  const { generals } = parse(await load("themes/pokemon/names.yaml"));
  const camp = JSON.parse(await load("themes/pokemon/camp.json"));
  assert.equal(roles.length, 7);
  assert.equal(new Set(roles.map((role) => role.id)).size, roles.length);
  for (const role of roles) {
    await access(new URL(`assets/sprites/${role.avatar}`, root));
    const general = generals.find((item) => item.id === role.id);
    assert.ok(general, `${role.id} is missing from names.yaml`);
    assert.equal(general.name, role.name);
    assert.equal(general.name_en, role.name_en);
    assert.equal(general.avatar, role.avatar);
    assert.equal(general.title, role.title);
    assert.ok(general.title_en && role.personality.includes(role.name));
    const sheet = await readFile(new URL(`assets/sprites/${role.avatar}`, root));
    assert.equal(sheet.subarray(1, 4).toString(), "PNG");
    assert.deepEqual([sheet.readUInt32BE(16), sheet.readUInt32BE(20)], [128, 32]);
    assert.ok(camp.zones.some((zone) => zone.role === role.id), `${role.id} has no camp post`);
    assert.ok(role.loadouts.length > 0 && role.loadouts.every(({ cli }) => ["claude-code", "codex"].includes(cli)));
    assert.ok(Object.hasOwn(zh.roleTypes, role.role_type) && Object.hasOwn(en.roleTypes, role.role_type));
  }
  const reviewer = roles.find((role) => role.role_type === "reviewer");
  assert.deepEqual(Object.fromEntries(roles.map(({ id, name_en }) => [id, name_en])), {
    reviewer: "Alakazam", frontend: "Lucario", backend: "Machamp", architect: "Metagross",
    captain: "Pikachu", debugger: "Porygon", "web-designer": "Smeargle",
  });
  assert.ok(reviewer.permissions.allow.every((tool) => ["Read", "Grep", "Glob"].includes(tool)));
});

test("camp zones fit the 320x180 logical map on whole tiles", async () => {
  const camp = JSON.parse(await load("themes/pokemon/camp.json"));
  assert.deepEqual([camp.width, camp.height, camp.tileSize], [320, 180, 16]);
  for (const { id, x, y, w, h } of camp.zones) {
    assert.ok([x, y, w, h].every(Number.isInteger), id);
    assert.ok(x >= 0 && y >= 0 && (x + w) * camp.tileSize <= camp.width && (y + h) * camp.tileSize <= camp.height, id);
  }
  await access(new URL(`themes/pokemon/tiles/${camp.background}`, root));
});

test("every general state maps to a clip with frames inside the 4-frame sheet", async () => {
  const { outputText } = ts.transpileModule(await load("src/camp/animations.ts"), { compilerOptions: { module: ts.ModuleKind.CommonJS } });
  const exports = {};
  new Function("exports", outputText)(exports);
  const { stateAnimations, clipFrames, clipSpeed } = exports;
  assert.deepEqual(Object.keys(stateAnimations).sort(), ["cancelled", "done", "error", "idle", "running", "waiting_user"]);
  for (const { clip, bubble } of Object.values(stateAnimations)) {
    assert.ok(clipFrames[clip].every((frame) => frame >= 0 && frame < 4));
    assert.ok(clipSpeed[clip] >= 0);
    assert.ok(bubble === undefined || ["wait", "error"].includes(bubble));
  }
  assert.equal(stateAnimations.error.bubble, "error");
  assert.equal(stateAnimations.waiting_user.bubble, "wait");
});

test("reference flows name preset role types and distinct Markdown outputs", async () => {
  const types = new Set(roles.map((role) => role.role_type));
  const files = (await readdir(new URL("flows/", root))).filter((file) => file.endsWith(".yaml"));
  assert.deepEqual(files.sort(), ["bug-fix.yaml", "new-project.yaml", "refactor.yaml", "research.yaml"]);
  for (const file of files) {
    const flow = parse(await load(`flows/${file}`));
    assert.equal(`${flow.id}.yaml`, file);
    assert.ok(flow.name && flow.fits && flow.steps.length, file);
    assert.ok(flow.steps.every((step) => step.step && types.has(step.role_type) && step.output.endsWith(".md")), file);
    assert.equal(new Set(flow.steps.map((step) => step.output)).size, flow.steps.length, file);
  }
});
