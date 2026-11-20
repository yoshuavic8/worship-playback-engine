import { useEffect } from "react";
import { toast } from "sonner";
import { useApp } from "@/store/app";
import { ipc } from "./ipc";
import { isKeyMapped, useMidi } from "./midi";
import { DEFAULT_SHORTCUTS, isTyping, keyLabel, matchShortcut } from "./shortcuts";
import { ACTIONS, panicStop, vampPress, vampRelease } from "./transport";

/**
 * Global transport shortcuts. PANIC always works, even while typing.
 * Other keys have no hardcoded meaning: they can be mapped through MIDI Learn
 * (click a control, press a key) and are then forwarded to the mapping engine.
 */
export function useShortcuts() {
  const shortcuts = useApp((s) => s.settings?.shortcuts);
  useEffect(() => {
    const map = { ...DEFAULT_SHORTCUTS, ...(shortcuts ?? {}) };
    const songId = () => useApp.getState().loaded?.songId ?? null;
    const modified = (e: KeyboardEvent) => e.metaKey || e.ctrlKey || e.altKey;
    const onKey = (e: KeyboardEvent) => {
      if (e.repeat) return;
      if (document.querySelector("[data-capturing-shortcut]")) return;
      const action = matchShortcut(e, map);
      const midi = useMidi.getState();

      // Learn armed: the next plain key becomes the mapping (Esc cancels).
      if (midi.armed && !isTyping(e) && !modified(e)) {
        e.preventDefault();
        e.stopPropagation();
        if (e.code === "Escape") return midi.disarm();
        if (action) {
          toast.warning(`${keyLabel(e.code)} is already a shortcut`, { description: "Pick another key, or change shortcuts in Settings." });
          return;
        }
        ipc.midiKey(e.code, true).catch(() => {});
        return;
      }

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
        if (!modified(e) && isKeyMapped(midi.mappings, e.code, songId())) {
          e.preventDefault();
          ipc.midiKey(e.code, true).catch(() => {});
        }
        return;
      }
      if (action === "loopSection") {
        // Tap toggles the vamp, hold loops only until the key is released.
        e.preventDefault();
        vampPress();
        return;
      }
      const fn = ACTIONS[action];
      if (fn) {
        e.preventDefault();
        fn();
      }
    };
    const onKeyUp = (e: KeyboardEvent) => {
      if (map.loopSection && e.code === map.loopSection) vampRelease();
      if (isTyping(e) || modified(e)) return;
      if (isKeyMapped(useMidi.getState().mappings, e.code, songId())) ipc.midiKey(e.code, false).catch(() => {});
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("keyup", onKeyUp, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("keyup", onKeyUp, true);
    };
  }, [shortcuts]);
}
