import { useCliDetection } from "../../app/cli-detection";
import { useI18n } from "../../i18n";
import { PixelButton } from "./PixelButton";
import { PixelIcon } from "./PixelIcon";

export function CliStatus({ compact = false, onOpenSettings }: { compact?: boolean; onOpenSettings?: () => void }) {
  const { results, loading, error, refresh } = useCliDetection();
  const { theme: { cli } } = useI18n();
  if (compact) return <footer className="workspace-status" aria-label={cli.label} aria-busy={loading}>
    <span><PixelIcon name="terminal" /><span>{loading ? cli.loading : error ? cli.scanError : cli.tools}</span></span>
    <button type="button" onClick={onOpenSettings} aria-label={cli.openSettings}>
      {results.map((result) => <span key={result.id}><i className={`status-dot status-dot--${result.error && result.status !== "not_found" ? "error" : result.status}`} />{result.id === "claude-code" ? "Claude Code" : "Codex"}<span className="workspace-status__label">{result.error && result.status !== "not_found" ? cli.check : cli.statuses[result.status]}</span></span>)}
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
            <div className="cli-tool__heading"><span className="cli-tool__icon"><PixelIcon name="terminal" /></span><strong>{result.id === "claude-code" ? "Claude Code" : "Codex"}</strong><span className="cli-tool__state"><i className={`status-dot status-dot--${result.error && result.status !== "not_found" ? "error" : result.status}`} />{result.error && result.status !== "not_found" ? cli.unknown : cli.statuses[result.status]}</span></div>
            {result.version && <code className="cli-tool__version">{result.version}</code>}
            {result.error && <p role="alert">{result.status === "not_found" ? cli.missing.replace("{command}", result.id === "claude-code" ? "claude" : "codex") : result.error}</p>}
            {!result.error && result.status === "found_needs_login" && (
              <p>{cli.login.replace("{command}", result.id === "claude-code" ? "claude auth login" : "codex login")}</p>
            )}
            {result.status === "not_found" && <a href={result.id === "claude-code" ? "https://code.claude.com/docs/en/setup" : "https://developers.openai.com/codex/cli"} target="_blank" rel="noreferrer">{cli.install}</a>}
          </li>
        ))}
      </ul>
      {results.length > 0 && results.every((result) => result.status === "not_found") && (
        <p>{cli.notInstalled}
          <a href="https://code.claude.com/docs/en/setup" target="_blank" rel="noreferrer">{cli.claudeDocs}</a>
          {" / "}
          <a href="https://developers.openai.com/codex/cli" target="_blank" rel="noreferrer">{cli.codexDocs}</a>
        </p>
      )}
    </section>
  );
}
