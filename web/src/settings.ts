// The app's lines in Tools, Preferences: how soon documents save, how often history keeps a version, and the font new
// documents start in. They are settings contributions like any extension's (`ctx.settings.register`); the app keeps
// the values in the browser with its other small preferences.
import { defineExtension, msg, t, type Extension } from "@docxtor/editor";
import type { App } from "./main";
import { store } from "./storage";

export interface AppPrefs {
  /** Files on disk (not the Library) save on their own. */
  autosaveFiles: boolean;
  /** Seconds after the last change before a Library document saves. */
  autosaveSeconds: number;
  /** Minutes between the versions history keeps by itself. */
  checkpointMinutes: number;
  /** The font blank documents start in; empty keeps the theme's font. */
  defaultFont: string;
  /** Its size in points; 0 keeps the theme's size. */
  defaultSize: number;
}

export const DEFAULT_PREFS: AppPrefs = { autosaveFiles: false, autosaveSeconds: 1, checkpointMinutes: 5, defaultFont: "", defaultSize: 0 };

export const PREFS_KEY = "docxtor.prefs";

/** The preferences as stored, with anything missing or out of range put right. */
export function cleanPrefs(raw: Partial<AppPrefs>): AppPrefs {
  const num = (v: unknown, fallback: number, min: number, max: number) => (typeof v === "number" && Number.isFinite(v) ? Math.min(max, Math.max(min, v)) : fallback);
  return {
    autosaveFiles: !!raw.autosaveFiles,
    autosaveSeconds: num(raw.autosaveSeconds, DEFAULT_PREFS.autosaveSeconds, 1, 600),
    checkpointMinutes: num(raw.checkpointMinutes, DEFAULT_PREFS.checkpointMinutes, 1, 240),
    defaultFont: typeof raw.defaultFont === "string" ? raw.defaultFont.trim() : "",
    defaultSize: num(raw.defaultSize, 0, 0, 200),
  };
}

export function appSettings(app: App): Extension {
  return defineExtension({
    id: "docxtor.app-settings",
    name: msg("App settings"),
    description: msg("How soon documents save, how often history keeps a version, and the font new documents start in."),
    activate(ctx) {
      const set = (patch: Partial<AppPrefs>) => {
        app.prefs = cleanPrefs({ ...app.prefs, ...patch });
        store(PREFS_KEY, app.prefs);
      };
      const saving = t("Saving");
      ctx.settings.register({
        id: "docxtor.save.seconds",
        group: saving,
        label: t("Save documents in your Library after"),
        help: t("Changes are saved this long after you stop typing."),
        kind: "number",
        min: 1,
        max: 600,
        unit: t("seconds"),
        get: () => app.prefs.autosaveSeconds,
        set: (v) => set({ autosaveSeconds: Number(v) }),
      });
      ctx.settings.register({
        id: "docxtor.save.files",
        group: saving,
        label: t("Save files on your computer by themselves"),
        get help() {
          return t("Off, they save when you press {key}.", { key: app.fe.extensions.shortcut("file.save") });
        },
        kind: "toggle",
        get: () => app.prefs.autosaveFiles,
        set: (v) => set({ autosaveFiles: !!v }),
      });
      ctx.settings.register({
        id: "docxtor.save.versions",
        group: saving,
        label: t("Keep a version in the history every"),
        help: t("Only when the document has changed since the last one."),
        kind: "number",
        min: 1,
        max: 240,
        unit: t("minutes"),
        get: () => app.prefs.checkpointMinutes,
        set: (v) => set({ checkpointMinutes: Number(v) }),
      });
      const fresh = t("New documents");
      ctx.settings.register({
        id: "docxtor.new.font",
        group: fresh,
        label: t("Font"),
        help: t("Blank documents start in this font. Leave it empty for the theme's font."),
        kind: "font",
        get: () => app.prefs.defaultFont,
        set: (v) => set({ defaultFont: String(v) }),
      });
      ctx.settings.register({
        id: "docxtor.new.size",
        group: fresh,
        label: t("Font size"),
        help: t("0 keeps the theme's size."),
        kind: "number",
        min: 0,
        max: 200,
        step: 0.5,
        unit: t("points"),
        get: () => app.prefs.defaultSize,
        set: (v) => set({ defaultSize: Number(v) }),
      });
    },
  });
}
