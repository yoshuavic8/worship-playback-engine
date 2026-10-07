// Keyboard shortcuts are configured as KeyboardEvent.code values ("Space", "KeyN", "ArrowLeft").

export const SHORTCUT_ACTIONS: { id: string; label: string; live?: boolean }[] = [
  { id: "playPause", label: "Play / Pause" },
  { id: "stop", label: "Stop" },
  { id: "panic", label: "PANIC stop", live: true },
  { id: "fadeOut", label: "Fade out" },
  { id: "nextSection", label: "Queue next section" },
  { id: "prevSection", label: "Queue previous section" },
  { id: "loopSection", label: "Loop current section" },
  { id: "nextSong", label: "Next song" },
  { id: "prevSong", label: "Previous song" },
  { id: "toggleClick", label: "Click on / off" },
];

export const DEFAULT_SHORTCUTS: Record<string, string> = {
  playPause: "Space",
  prevSection: "ArrowLeft",
  nextSection: "ArrowRight",
  panic: "Escape",
  nextSong: "KeyN",
  prevSong: "KeyP",
  fadeOut: "KeyF",
  loopSection: "KeyL",
  stop: "KeyS",
  toggleClick: "KeyC",
};

export function keyLabel(code: string | undefined): string {
  if (!code) return "—";
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  const map: Record<string, string> = {
    Space: "Space", Escape: "Esc", ArrowLeft: "←", ArrowRight: "→", ArrowUp: "↑", ArrowDown: "↓",
    Enter: "Enter", Backspace: "⌫", Tab: "Tab", Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]",
    Comma: ",", Period: ".", Slash: "/", Semicolon: ";", Quote: "'", Backquote: "`",
  };
  return map[code] ?? code.replace(/^Numpad/, "Num ");
}

/** True when the event comes from a text field — shortcuts must not steal typing. */
export function isTyping(e: KeyboardEvent): boolean {
  const t = e.target as HTMLElement | null;
  if (!t) return false;
  const tag = t.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || t.isContentEditable;
}

/** Returns the action id bound to this key event, if any. Modifier combos are ignored. */
export function matchShortcut(e: KeyboardEvent, map: Record<string, string>): string | null {
  if (e.metaKey || e.ctrlKey || e.altKey) return null;
  for (const [action, code] of Object.entries(map)) {
    if (code && code === e.code) return action;
  }
  return null;
}
