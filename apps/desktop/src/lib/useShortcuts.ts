import { useEffect } from "react";
import { useApp } from "@/store/app";
import { DEFAULT_SHORTCUTS, isTyping, matchShortcut } from "./shortcuts";
import { ACTIONS, panicStop } from "./transport";

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
        // Escape also closes dialogs: only panic when no dialog is open, or with Shift+Escape.
        if (map.panic === "Escape" && document.querySelector("[role=dialog]") && !e.shiftKey) return;
        e.preventDefault();
        panicStop();
        return;
      }
      if (isTyping(e) || document.querySelector("[role=dialog]")) return;
      if (!action) return;
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
