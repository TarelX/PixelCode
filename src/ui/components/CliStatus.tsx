import { useCliDetection } from "../../app/cli-detection";
import { PixelButton } from "./PixelButton";
import { PixelIcon } from "./PixelIcon";

const labels = {
  not_found: "未找到",
  found: "已找到",
  found_needs_login: "已找到但未登录",
};

export function CliStatus({ compact = false, onOpenSettings }: { compact?: boolean; onOpenSettings?: () => void }) {
  const { results, loading, error, refresh } = useCliDetection();
  if (compact) return <footer className="workspace-status" aria-label="CLI 状态" aria-busy={loading}>
    <span><PixelIcon name="terminal" /><span>{loading ? "正在探测编码工具…" : error ? "工具探测失败" : "编码工具"}</span></span>
    <button type="button" onClick={onOpenSettings} aria-label="查看编码工具状态与设置">
      {results.map((result) => <span key={result.id}><i className={`status-dot status-dot--${result.error && result.status !== "not_found" ? "error" : result.status}`} />{result.id === "claude-code" ? "Claude Code" : "Codex"}<span className="workspace-status__label">{result.error && result.status !== "not_found" ? "需检查" : labels[result.status]}</span></span>)}
      <span className="workspace-status__settings">工具设置 <PixelIcon name="arrow" /></span>
    </button>
  </footer>;
  return (
    <section className="cli-status" aria-label="CLI 状态" aria-busy={loading}>
      <div className="section-heading"><div><span className="section-label">CONNECTED TOOLS</span><h3>连接你的编码工具</h3></div><PixelButton disabled={loading} onClick={() => void refresh()}>重新探测 CLI</PixelButton></div>
      <p className="cli-status__intro">使用电脑上已安装并登录的 CLI。未安装工具，也可以先打开大营。</p>
      {loading && <p role="status">正在探测 CLI…</p>}
      {error && <p role="alert">{error}</p>}
      <ul>
        {results.map((result) => (
          <li key={result.id} className="cli-tool">
            <div className="cli-tool__heading"><span className="cli-tool__icon"><PixelIcon name="terminal" /></span><strong>{result.id === "claude-code" ? "Claude Code" : "Codex"}</strong><span className="cli-tool__state"><i className={`status-dot status-dot--${result.error && result.status !== "not_found" ? "error" : result.status}`} />{result.error && result.status !== "not_found" ? "无法确认状态" : labels[result.status]}</span></div>
            {result.version && <code className="cli-tool__version">{result.version}</code>}
            {result.error && <p role="alert">{result.error}</p>}
            {!result.error && result.status === "found_needs_login" && (
              <p>请在终端运行 {result.id === "claude-code" ? "claude auth login" : "codex login"} 后重试。</p>
            )}
            {result.status === "not_found" && <a href={result.id === "claude-code" ? "https://code.claude.com/docs/en/setup" : "https://developers.openai.com/codex/cli"} target="_blank" rel="noreferrer">查看安装文档 ↗</a>}
          </li>
        ))}
      </ul>
      {results.length > 0 && results.every((result) => result.status === "not_found") && (
        <p>未找到任何 CLI，仍可打开大营。请先安装并登录：
          <a href="https://code.claude.com/docs/en/setup" target="_blank" rel="noreferrer">Claude Code 安装文档</a>
          {" / "}
          <a href="https://developers.openai.com/codex/cli" target="_blank" rel="noreferrer">Codex 安装文档</a>
        </p>
      )}
    </section>
  );
}
