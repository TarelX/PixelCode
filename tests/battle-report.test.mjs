import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";
import { parse } from "yaml";
import { create } from "zustand";

const load = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");
async function loadModule(path, imports) {
  const source = await load(path);
  const result = {};
  new Function("require", "exports", ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } }).outputText)((id) => imports[id], result);
  return result;
}
const theme = JSON.parse(await load("themes/three-kingdoms/ui.json"));
const english = JSON.parse(await load("themes/three-kingdoms/ui.en.json"));
let storedLocale = "zh-CN";
const i18n = await loadModule("src/i18n/index.ts", {
  "@tauri-apps/api/core": { invoke: async (command, { locale } = {}) => {
    if (command === "get_locale") return storedLocale;
    assert.equal(command, "set_locale");
    storedLocale = locale;
  } },
  yaml: { parse },
  zustand: { create },
  "../../themes/three-kingdoms/ui.json": theme,
  "../../themes/three-kingdoms/ui.en.json": english,
  "../../themes/three-kingdoms/names.yaml?raw": await load("themes/three-kingdoms/names.yaml"),
});
const exports = await loadModule("src/ui/battle-report.ts", { "../i18n": i18n });
const chineseCopy = { locale: "zh-CN", theme };

test("the UI locale loads and updates through settings, and custom role text is preserved", async () => {
  const { useLocale, roleName, roleTitle } = i18n;
  assert.equal(useLocale.getState().locale, "zh-CN");
  await useLocale.getState().change("en");
  assert.equal(storedLocale, "en");
  assert.equal(useLocale.getState().locale, "en");
  storedLocale = "zh-CN";
  await useLocale.getState().load();
  assert.equal(useLocale.getState().locale, "zh-CN");

  const captain = { id: "captain", source: "preset", name: "诸葛亮", name_en: "Zhuge Liang", title: "队长" };
  assert.equal(roleName(captain, "en"), "Zhuge Liang");
  assert.equal(roleTitle(captain, "en"), "The Strategist");
  assert.equal(roleName({ ...captain, name_en: null }, "en"), "诸葛亮");
  assert.equal(roleTitle({ ...captain, source: "modified_preset", title: "My captain" }, "en"), "My captain");
});

test("report uses local calendar boundaries and groups all task states", () => {
  const day = exports.reportDay(new Date(2026, 10, 21, 23, 59));
  assert.equal(day.date, "2026-11-21");
  assert.equal(day.start, new Date(2026, 10, 21).getTime() / 1000);
  assert.equal(day.end, new Date(2026, 10, 22).getTime() / 1000);
  assert.deepEqual(exports.reportCounts({ done: 2, checking: 1, reviewing: 1, running: 1, blocked: 2, failed: 1, pending: 4 }), { done: 2, running: 3, failed: 3, pending: 4 });
  assert.deepEqual(exports.reportCounts({}), { done: 0, running: 0, failed: 0, pending: 0 });
});

test("renders populated and empty reports, propagates canvas and encoding failures", async () => {
  const text = [];
  let context = true, encode = true;
  const ctx = { measureText: (value) => ({ width: value.length * 12 }), scale() {}, fillRect() {}, drawImage() {}, fillText: (value) => text.push(value) };
  globalThis.document = { fonts: { ready: Promise.resolve() }, documentElement: {}, body: {}, createElement: () => ({ getContext: () => context ? ctx : null, toBlob: (callback) => callback(encode ? new Blob(["png"]) : null) }) };
  globalThis.getComputedStyle = () => ({ fontFamily: "monospace", getPropertyValue: () => "#123456" });
  const data = { camp: { name: "Test camp", root_path: "/camp" }, date: "2026-11-21", roles: [], runs: {}, plan: null, counts: {} };
  assert.ok(await exports.renderBattleReport(data, chineseCopy) instanceof Blob);
  assert.ok(text.includes(theme.report.emptyRoles));
  const populated = { ...data, roles: [{ id: "a", name: "名将", name_en: "General" }], runs: { a: { status: "running" } }, plan: { plan: { tasks: [{ title: "Implement", role_id: "a", status: "done" }] } } };
  await exports.renderBattleReport(populated, { locale: "en", theme: english });
  assert.ok(text.includes(english.report.title));
  assert.ok(text.some((line) => line.includes("General") && line.includes(english.runStates.running)));
  assert.ok(text.some((line) => line.includes("Implement") && line.includes(english.wall.statuses.done)));
  const scopeWords = english.report.scope.split(" ");
  const scopeStart = text.findIndex((line) => line.startsWith(scopeWords[0]));
  const scopeEnd = text.indexOf("", scopeStart);
  assert.deepEqual(text.slice(scopeStart, scopeEnd).join(" ").split(" "), scopeWords);
  await exports.renderBattleReport(populated, chineseCopy);
  assert.ok(text.some((line) => line.includes("名将") && line.includes(theme.runStates.running)));
  encode = false;
  await assert.rejects(exports.renderBattleReport(data, chineseCopy), { message: theme.report.pngError });
  context = false;
  await assert.rejects(exports.renderBattleReport(data, chineseCopy), { message: theme.report.canvasError });
});
