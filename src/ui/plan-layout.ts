export interface PlanTask {
  id: string;
  title: string;
  description: string;
  role_id: string;
  depends_on: string[];
  input_artifacts: string[];
  output_artifacts: string[];
  acceptance_criteria: string;
  check_command: { program: string; args: string[] } | null;
  files?: string[];
  status?: string;
}

export interface StoredPlan {
  id: string;
  status: string;
  scenario: string;
  user_request: string;
  created_at: number;
  execution_message: string | null;
  plan: { scenario: string; recruited_roles: string[]; tasks: PlanTask[] };
}

export function layoutPlan(tasks: PlanTask[]) {
  const byId = new Map(tasks.map((task) => [task.id, task]));
  const depths = new Map<string, number>();
  const visiting = new Set<string>();
  function depth(id: string): number {
    if (depths.has(id)) return depths.get(id)!;
    const task = byId.get(id);
    if (!task || visiting.has(id)) throw new Error(id);
    visiting.add(id);
    const value = Math.max(0, ...task.depends_on.map((parent) => depth(parent) + 1));
    visiting.delete(id);
    depths.set(id, value);
    return value;
  }
  const rows = new Map<number, number>();
  return tasks.map((task) => {
    const column = depth(task.id);
    const row = rows.get(column) ?? 0;
    rows.set(column, row + 1);
    return { task, x: 16 + column * 288, y: 16 + row * 160 };
  });
}
