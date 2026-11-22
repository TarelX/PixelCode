// Adapted from MonoCode's src/shared/lib/popover.ts (MIT, Copyright (c) 2026 Nick).
// See THIRD_PARTY_NOTICES.md for the complete license and upstream revision.
export function placePicker(anchor: DOMRect, height: number, viewport: { width: number; height: number }) {
  const gap = 6, padding = 8;
  const width = Math.min(264, Math.max(0, viewport.width - padding * 2));
  const above = anchor.top - gap - padding;
  const below = viewport.height - anchor.bottom - gap - padding;
  const topSide = above >= Math.min(height, 330) || below <= above;
  const available = Math.max(0, topSide ? above : below);
  const maxHeight = Math.min(330, available);
  return {
    left: Math.min(Math.max(anchor.left, padding), Math.max(padding, viewport.width - width - padding)),
    ...(topSide ? { bottom: Math.max(padding, viewport.height - anchor.top + gap) } : { top: Math.max(padding, anchor.bottom + gap) }),
    width, maxHeight,
  };
}
