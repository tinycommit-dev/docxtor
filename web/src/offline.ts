// Working offline: registers the service worker the build writes (see vite.config.ts and offline/sw.js), says in the
// status bar when there is no connection, and opens files the installed app is asked to open.
import { h, t } from "@docxtor/editor";
import type { App } from "./main";
import { SingleFileFs } from "./files/fs";

/** Whether this page is controlled by the service worker, so it will open without a connection. */
export function isKept(): boolean {
  return !!navigator.serviceWorker?.controller;
}

export function registerOffline(app: App) {
  // Developing, the dev server has no service worker (its files are not a build); the tests run a built copy.
  if ("serviceWorker" in navigator && import.meta.env.PROD) {
    const go = () => void navigator.serviceWorker.register("./sw.js").catch((e) => console.warn("Docxtor can't be kept for offline use:", e));
    if (document.readyState === "complete") go();
    else window.addEventListener("load", go, { once: true });
  }
  // The status bar names the state in words, and goes away when there is nothing to say.
  const chip = h("div", { class: "stat offline", hidden: true, role: "status", title: t("Docxtor is working without a connection. Your documents are safe here; the assistant and lookups wait for you to be back online.") }, h("span", { class: "readout" }, t("Offline")));
  const sync = () => (chip.hidden = navigator.onLine);
  window.addEventListener("online", sync);
  window.addEventListener("offline", sync);
  sync();
  app.addStatus(chip);

  // Installed as an app, a Word file opened with it comes here.
  const launch = (window as unknown as { launchQueue?: { setConsumer(fn: (p: { files: FileSystemFileHandle[] }) => void): void } }).launchQueue;
  launch?.setConsumer(async (params) => {
    const [handle] = params.files ?? [];
    if (handle) await app.openRef({ backend: new SingleFileFs(handle), path: handle.name });
  });
}
