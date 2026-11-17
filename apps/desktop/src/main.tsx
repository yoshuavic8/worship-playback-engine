import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";

// Native-app feel: no browser context menu except in text fields.
window.addEventListener("contextmenu", (e) => {
  const t = e.target as HTMLElement;
  if (!(t.tagName === "INPUT" || t.tagName === "TEXTAREA")) e.preventDefault();
});

function webKitTooOld(): boolean {
  try {
    return !CSS.supports("color", "color-mix(in oklab, red, blue)");
  } catch {
    return true;
  }
}

function showWebKitGuard(root: HTMLElement) {
  root.innerHTML = "";
  const box = document.createElement("div");
  box.setAttribute(
    "style",
    "min-height:100vh;display:flex;align-items:center;justify-content:center;padding:32px;background:#0d0e10;color:#e8eaed;font-family:system-ui,-apple-system,sans-serif;line-height:1.5;",
  );
  box.innerHTML =
    "<div style=\"max-width:28rem;text-align:center\">" +
    "<h1 style=\"font-size:1.25rem;font-weight:600;margin:0 0 12px\">Safari / WebKit update required</h1>" +
    "<p style=\"margin:0 0 8px;font-size:0.95rem;color:#b0b5bc\">" +
    "Worship Playback Engine needs WebKit 16.4+ (CSS color-mix and @property). " +
    "Please update macOS and Safari via <strong>System Settings → General → Software Update</strong>, then reopen the app." +
    "</p>" +
    "<p style=\"margin:0;font-size:0.85rem;color:#7a8088\">macOS 12 Monterey can reach Safari 17 when fully updated.</p>" +
    "</div>";
  root.appendChild(box);
}

async function start() {
  const root = document.getElementById("root")!;
  if (webKitTooOld()) {
    showWebKitGuard(root);
    return;
  }
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
    (await import("./dev/mock")).install();
  }
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
start();
