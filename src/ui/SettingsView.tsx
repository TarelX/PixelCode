import { useCliDetection } from "../app/cli-detection";
import { CliStatus } from "./components/CliStatus";

export function SettingsView() {
  useCliDetection(true);
  return <CliStatus />;
}
