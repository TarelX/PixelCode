import type { PlanTask } from "./plan-layout";
import theme from "../../themes/three-kingdoms/ui.json";

export function TaskDetail({ task, role, tasks }: { task: PlanTask; role: string; tasks: PlanTask[] }) {
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
    <h3>{theme.wall.check}</h3>
    {task.check_command ? <dl>
      <div><dt>{theme.wall.program}</dt><dd><code>{task.check_command.program}</code></dd></div>
      <div><dt>{theme.wall.args}</dt><dd><code>{JSON.stringify(task.check_command.args)}</code></dd></div>
    </dl> : <p>{theme.wall.none}</p>}
  </div>;
}
