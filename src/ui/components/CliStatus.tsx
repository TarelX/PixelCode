import { useCliDetection } from "../../app/cli-detection";

const labels = {
  not_found: "未找到",
  found: "已找到",
  found_needs_login: "已找到但未登录",
};

export function CliStatus() {
  const { results, loading, error, refresh } = useCliDetection();
  return (
    <section aria-label="CLI 状态" aria-busy={loading}>
      <h2>CLI 状态</h2>
      {loading && <p role="status">正在探测 CLI…</p>}
      {error && <p role="alert">{error}</p>}
      <ul>
        {results.map((result) => (
          <li key={result.id}>
            {result.id === "claude-code" ? "Claude Code" : "Codex"}：
            {result.error && result.status !== "not_found" ? "无法确认状态" : labels[result.status]}
            {result.version && `（${result.version}）`}
            {result.error && <p role="alert">{result.error}</p>}
            {!result.error && result.status === "found_needs_login" && (
              <p>请在终端运行 {result.id === "claude-code" ? "claude auth login" : "codex login"} 后重试。</p>
            )}
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
      <button disabled={loading} onClick={() => void refresh()}>重新探测 CLI</button>
    </section>
  );
}
