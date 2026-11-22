export type WorkAction = "idle" | "typing" | "checking" | "reviewing" | "waiting" | "done" | "error" | "stopped";

export function workAction(status: string): WorkAction {
  if (status === "running") return "typing";
  if (status === "checking" || status === "reviewing") return status;
  if (status === "waiting" || status === "pending" || status === "draft") return "waiting";
  if (status === "done" || status === "completed") return "done";
  if (status === "failed" || status === "blocked") return "error";
  return status === "stopped" ? "stopped" : "idle";
}

export function workPose(action: WorkAction, seconds: number, phase: number, reduced = false) {
  const time = reduced ? 0 : seconds + phase * .73;
  const beat = Math.floor(time * 8);
  const typing = action === "typing" && !reduced && time % 5.6 < 3.8 && beat % 9 < 7;
  return {
    action, frame: reduced ? 0 : Math.floor(time * 3),
    left: typing && beat % 2 === 0 ? 2 : 0,
    right: typing && beat % 2 === 1 ? 2 : 0,
    lean: !reduced && (typing || action === "reviewing") ? -1 : 0,
    nod: reduced ? 0 : typing ? beat % 4 === 0 ? 1 : 0
      : action === "done" && seconds < .9 && Math.floor(seconds * 6) % 2 === 0 ? -1 : 0,
  };
}

export type WorkPose = ReturnType<typeof workPose>;
