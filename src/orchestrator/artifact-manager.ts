import { invoke } from "@tauri-apps/api/core";

export interface Artifact {
  id: string;
  task_id: string;
  type: string;
  file_path: string;
  absolute_path: string;
  content_preview: string;
  created_at: number;
}

export const readArtifact = (artifactId: string) => invoke<{ artifact: Artifact; content: string }>("read_artifact", { artifactId });
export const listArtifacts = (taskId: string) => invoke<Artifact[]>("list_artifacts", { taskId });
