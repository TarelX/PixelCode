export type GeneralState = "idle" | "running" | "waiting_user" | "done" | "error" | "cancelled";
export type Clip = "idle" | "walk" | "still";
export type Bubble = "wait" | "error";

export interface StateAnimation {
  clip: Clip;
  bubble?: Bubble;
}

export const stateAnimations: Record<GeneralState, StateAnimation> = {
  running: { clip: "walk" },
  waiting_user: { clip: "idle", bubble: "wait" },
  done: { clip: "walk" },
  error: { clip: "still", bubble: "error" },
  idle: { clip: "idle" },
  cancelled: { clip: "still" },
};

export const clipFrames: Record<Clip, number[]> = {
  idle: [0, 1],
  walk: [2, 3],
  still: [0],
};

export const clipSpeed: Record<Clip, number> = {
  idle: 1 / 40,
  walk: 1 / 10,
  still: 0,
};
