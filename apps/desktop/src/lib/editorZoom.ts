export const MIN_TIMELINE_ZOOM = 1;
export const MAX_TIMELINE_ZOOM = 32;

export function clampTimelineZoom(z: number) {
  return Math.min(MAX_TIMELINE_ZOOM, Math.max(MIN_TIMELINE_ZOOM, Math.round(z * 100) / 100));
}
