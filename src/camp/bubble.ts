export interface BubbleAnchor {
  x: number;
  y: number;
  footY: number;
  canvas: { left: number; top: number; width: number; height: number };
}

export function bubblePosition(anchor: BubbleAnchor, height: number) {
  const { x, y, footY, canvas } = anchor;
  const width = Math.min(220, canvas.width - 16);
  const left = Math.max(8, Math.min(canvas.width - width - 8, x - width / 2));
  const below = y - height < 8;
  const top = Math.max(8, Math.min(canvas.height - height - 8, below ? footY + 8 : y - height));
  return { left: canvas.left + left, top: canvas.top + top, width, below, tail: Math.max(8, Math.min(width - 16, x - left - 4)) };
}
