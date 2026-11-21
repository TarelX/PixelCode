import { useEffect, useState, type ReactNode } from "react";
import { listArtifacts, readArtifact, type Artifact } from "../orchestrator/artifact-manager";
import { PixelButton } from "./components/PixelButton";
import type { PlanTask } from "./plan-layout";
import { useI18n } from "../i18n";

export function TaskDetail({ task, role, tasks, assignment, resultId }: {
  task: PlanTask; role: string; tasks: PlanTask[]; assignment?: ReactNode; resultId?: string;
}) {
  const { theme } = useI18n();
  const fields = [
    [theme.wall.description, task.description],
    [theme.wall.role, role],
    [theme.wall.dependencies, task.depends_on.map((id) => tasks.find((item) => item.id === id)?.title ?? id).join(" · ")],
    [theme.wall.inputs, task.input_artifacts.join("\n")],
    [theme.wall.outputs, task.output_artifacts.join("\n")],
    [theme.wall.acceptance, task.acceptance_criteria],
  ];
  return <div className="task-detail">
    <dl>{fields.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value || theme.wall.none}</dd></div>)}</dl>
    {assignment}
    <h3>{theme.wall.check}</h3>
    {task.check_command ? <dl>
      <div><dt>{theme.wall.program}</dt><dd><code>{task.check_command.program}</code></dd></div>
      <div><dt>{theme.wall.args}</dt><dd><code>{JSON.stringify(task.check_command.args)}</code></dd></div>
    </dl> : <p>{theme.wall.none}</p>}
    {resultId && <TaskResults taskId={resultId} />}
  </div>;
}

function TaskResults({ taskId }: { taskId: string }) {
  const { theme } = useI18n();
  const [files, setFiles] = useState<Artifact[] | null>(null);
  const [view, setView] = useState<{ artifact: Artifact; content: string } | null>(null);
  const [reading, setReading] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    void listArtifacts(taskId).then((value) => { if (active) setFiles(value); })
      .catch((reason) => { if (active) setError(String(reason)); });
    return () => { active = false; };
  }, [taskId]);
  async function open(id: string) {
    setReading(true);
    setError("");
    try { setView(await readArtifact(id)); }
    catch (reason) { setError(String(reason)); }
    finally { setReading(false); }
  }
  return <section>
    <h3>{theme.artifactTitle}</h3>
    <div className="wall-gate__actions">{files?.map((file) => <PixelButton key={file.id} disabled={reading} onClick={() => void open(file.id)}>
      {file.file_path.split("/").pop()}
    </PixelButton>)}</div>
    {error && <p role="alert">{error}</p>}
    {reading || (!files && !error) ? <p role="status">{theme.artifactLoading}</p> : null}
    {view && <><p className="artifact-location">{view.artifact.absolute_path}</p><pre className="artifact-content">{view.content}</pre></>}
  </section>;
}
