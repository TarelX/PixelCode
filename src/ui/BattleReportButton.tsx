import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { Camp } from "../app/store";
import { useRoster } from "../camp/roster";
import { useRuns } from "../camp/run";
import type { StoredPlan } from "./plan-layout";
import { reportDay, renderBattleReport } from "./battle-report";
import { PixelButton } from "./components/PixelButton";
import theme from "../../themes/three-kingdoms/ui.json";

export function BattleReportButton({ camp, snapshot }: { camp: Camp; snapshot: () => HTMLCanvasElement | undefined }) {
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  async function exportReport() {
    setBusy(true); setMessage(""); setError("");
    try {
      const day = reportDay();
      const path = await save({ defaultPath: `pixelcode-battle-report-${day.date}.png`, filters: [{ name: "PNG", extensions: ["png"] }] });
      if (!path) return;
      const [plan, counts] = await Promise.all([
        invoke<StoredPlan | null>("latest_plan", { campId: camp.id }),
        invoke<Record<string, number>>("battle_report_counts", { campId: camp.id, start: day.start, end: day.end }),
      ]);
      const blob = await renderBattleReport({ camp, date: day.date, plan, counts, roles: useRoster.getState().roles, runs: useRuns.getState().runs, snapshot: snapshot() });
      await invoke("export_battle_report", { path, bytes: Array.from(new Uint8Array(await blob.arrayBuffer())) });
      setMessage(`${theme.report.saved}${path}`);
    } catch (reason) { setError(`${theme.report.error}${String(reason)}`); }
    finally { setBusy(false); }
  }
  return <div className="battle-report-action">
    <PixelButton disabled={busy} onClick={() => void exportReport()}>{busy ? theme.report.busy : theme.report.export}</PixelButton>
    {message && <p role="status">{message}</p>}
    {error && <p role="alert">{error}</p>}
  </div>;
}
