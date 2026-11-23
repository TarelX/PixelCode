import { useCliDetection, type DetectResult } from "../../app/cli-detection";
import { useI18n } from "../../i18n";
import { PixelButton } from "./PixelButton";
import { PixelIcon } from "./PixelIcon";

type Summary = "not_found" | "found" | "found_needs_login" | "unverified" | "unknown";

function summary(result: DetectResult): Summary {
  if (!result.installed) return "not_found";
  if (!result.signed_in && !result.error) return "found_needs_login";
  if (result.version_compatibility === "unverified") return "unverified";
  if (result.version_compatibility === "unknown") return "unknown";
  return "found";
}

function dotClass(result: DetectResult) {
  if (!result.installed) return "status-dot--not_found";
  if (result.error) return "status-dot--error";
  if (!result.signed_in) return "status-dot--found_needs_login";
  if (result.version_compatibility === "unverified") return "status-dot--found_needs_login";
  if (result.version_compatibility === "unknown") return "status-dot--unknown";
  return "status-dot--found";
}

function command(result: DetectResult) {
  return result.id === "claude-code" ? "claude" : "codex";
}

export function CliStatus({ compact = false, onOpenSettings }: { compact?: boolean; onOpenSettings?: () => void }) {
  const { results, loading, error, refresh } = useCliDetection();
  const { theme: { cli } } = useI18n();
  if (compact) return <footer className="workspace-status" aria-label={cli.label} aria-busy={loading}>
    <span><PixelIcon name="terminal" /><span>{loading ? cli.loading : error ? cli.scanError : cli.tools}</span></span>
    <button type="button" onClick={onOpenSettings} aria-label={cli.openSettings}>
      {results.map((result) => <span key={result.id}><i className={`status-dot ${dotClass(result)}`} />{result.id === "claude-code" ? "Claude Code" : "Codex"}<span className="workspace-status__label">{result.error && result.installed ? cli.check : cli.statuses[summary(result)]}</span></span>)}
      <span className="workspace-status__settings">{cli.settings} <PixelIcon name="arrow" /></span>
    </button>
  </footer>;
  return (
    <section className="cli-status" aria-label={cli.label} aria-busy={loading}>
      <div className="section-heading"><div><span className="section-label">{cli.eyebrow}</span><h3>{cli.title}</h3></div><PixelButton disabled={loading} onClick={() => void refresh()}>{cli.refresh}</PixelButton></div>
      <p className="cli-status__intro">{cli.intro}</p>
      {loading && <p role="status">{cli.loading}</p>}
      {error && <p role="alert">{cli.scanError} · {error}</p>}
      <ul>
        {results.map((result) => (
          <li key={result.id} className="cli-tool">
            <div className="cli-tool__heading"><span className="cli-tool__icon"><PixelIcon name="terminal" /></span><strong>{result.id === "claude-code" ? "Claude Code" : "Codex"}</strong><span className="cli-tool__state"><i className={`status-dot ${dotClass(result)}`} />{result.error && result.installed ? cli.unknown : cli.statuses[summary(result)]}</span></div>
            {result.version && <code className="cli-tool__version">{result.version}</code>}
            <ul className="cli-checks">
              <li><span>{cli.checks.install}</span><strong><i className={`status-dot ${result.installed ? "status-dot--found" : "status-dot--not_found"}`} />{result.installed ? cli.checks.found : cli.checks.missing}</strong></li>
              <li><span>{cli.checks.login}</span><strong><i className={`status-dot ${result.signed_in ? "status-dot--found" : result.error ? "status-dot--error" : "status-dot--found_needs_login"}`} />{result.signed_in ? cli.checks.signedIn : result.error ? cli.checks.unknown : cli.checks.signInRequired}</strong></li>
              <li><span>{cli.checks.version}</span><strong><i className={`status-dot ${result.version_compatibility === "compatible" ? "status-dot--found" : result.version_compatibility === "unverified" ? "status-dot--found_needs_login" : "status-dot--unknown"}`} />{result.version_compatibility === "compatible" ? cli.checks.compatible : result.version_compatibility === "unverified" ? cli.checks.unverified : cli.checks.unknown}</strong></li>
            </ul>
            {result.error && <p role="alert">{result.installed ? result.error : cli.missing.replace("{command}", command(result))}</p>}
            {result.installed && !result.signed_in && !result.error && (
              <p>{cli.login.replace("{command}", result.id === "claude-code" ? "claude auth login" : "codex login")}</p>
            )}
            {result.installed && result.signed_in && result.version_compatibility !== "compatible" && !result.error && (
              <p role="status">{cli.unverifiedHint}</p>
            )}
            {!result.installed && <a href={result.id === "claude-code" ? "https://code.claude.com/docs/en/setup" : "https://developers.openai.com/codex/cli"} target="_blank" rel="noreferrer">{cli.install}</a>}
          </li>
        ))}
      </ul>
      {results.length > 0 && results.every((result) => !result.installed) && (
        <p>{cli.notInstalled}
          <a href="https://code.claude.com/docs/en/setup" target="_blank" rel="noreferrer">{cli.claudeDocs}</a>
          {" / "}
          <a href="https://developers.openai.com/codex/cli" target="_blank" rel="noreferrer">{cli.codexDocs}</a>
        </p>
      )}
    </section>
  );
}
