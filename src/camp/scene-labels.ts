export interface SceneLabel { id: string; left: number; top: number; width: number; height: number }

export function layoutSceneLabels(labels: SceneLabel[], width: number, height: number): SceneLabel[] {
  const placed: SceneLabel[] = [];
  for (const label of labels) {
    const clamp = (left: number, top: number) => ({ ...label,
      left: Math.round(Math.max(4, Math.min(width - label.width - 4, left))),
      top: Math.round(Math.max(4, Math.min(height - label.height - 4, top))),
    });
    const preferred = clamp(label.left, label.top);
    const candidates = [preferred];
    for (let row = 4; row + label.height <= height - 4; row += label.height + 5) {
      for (const left of [preferred.left, ...placed.flatMap((item) => [item.left - label.width - 5, item.left + item.width + 5])]) {
        candidates.push(clamp(left, row));
      }
    }
    candidates.sort((a, b) => Math.hypot(a.left - preferred.left, a.top - preferred.top) - Math.hypot(b.left - preferred.left, b.top - preferred.top));
    const free = candidates.find((candidate) => placed.every((item) => candidate.left + candidate.width + 4 <= item.left
      || item.left + item.width + 4 <= candidate.left || candidate.top + candidate.height + 4 <= item.top || item.top + item.height + 4 <= candidate.top));
    placed.push(free ?? preferred);
  }
  return placed;
}
