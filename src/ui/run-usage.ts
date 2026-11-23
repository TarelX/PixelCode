import type { RoleRun } from "../camp/run";
import type { Locale, ThemeCopy } from "../i18n";

export function usageLimitText(run: RoleRun, theme: ThemeCopy, locale: Locale) {
  const limited = run.usage_limited;
  if (!limited) return "";
  if (limited.resets_at == null) return theme.usageLimitedNoReset;
  const time = new Date(limited.resets_at * 1000).toLocaleTimeString(locale === "en" ? "en-US" : "zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false });
  return theme.usageLimited.replace("{time}", time);
}

export function tokenUsageText(run: RoleRun, theme: ThemeCopy) {
  const metrics = run.turn_metrics;
  if (!metrics) return "";
  return theme.tokenUsage
    .replace("{input}", String(metrics.input_tokens))
    .replace("{output}", String(metrics.output_tokens))
    .replace("{cache}", metrics.cache_tokens == null ? "—" : String(metrics.cache_tokens));
}
