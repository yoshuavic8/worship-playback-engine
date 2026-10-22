import { useEffect } from "react";
import { useApp } from "@/store/app";
import { DEFAULT_SHORTCUTS, isTyping, matchShortcut } from "./shortcuts";
import { ACTIONS, jumpTo, panicStop } from "./transport";

/** Global transport shortcuts. PANIC always works, even while typing. */
export function useShortcuts() {
  const shortcuts = useApp((s) => s.settings?.shortcuts);
  useEffect(() => {
    const map = { ...DEFAULT_SHORTCUTS, ...(shortcuts ?? {}) };
    const onKey = (e: KeyboardEvent) => {
      if (e.repeat) return;
      if (document.querySelector("[data-capturing-shortcut]")) return;
      const action = matchShortcut(e, map);
      if (action === "panic") {
        const onLive = useApp.getState().route.page === "live";
        // Esc = PANIC only on Live. Elsewhere use Shift+Esc (or the PANIC button).
        if (map.panic === "Escape" && !onLive && !e.shiftKey) return;
        if (map.panic === "Escape" && document.querySelector("[role=dialog]") && !e.shiftKey) return;
        e.preventDefault();
        panicStop();
        return;
      }
      if (isTyping(e) || document.querySelector("[role=dialog]")) return;
      if (!action) {
        // Keys 1–9 jump to that section (same rule as tapping: once = end of part, twice = end of bar).
        const m = /^Digit([1-9])$/.exec(e.code);
        const segs = useApp.getState().loaded?.segments;
        if (m && segs && !e.metaKey && !e.ctrlKey && !e.altKey) {
          const i = Number(m[1]) - 1;
          if (i < segs.length) {
            e.preventDefault();
            jumpTo(i);
          }
        }
        return;
      }
      const fn = ACTIONS[action];
      if (fn) {
        e.preventDefault();
        fn();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [shortcuts]);
}
