import "@docxtor/editor/style.css";
import "./fonts.css";
import "./app.css";
import { Engine, loadEngine } from "@docxtor/core";
import { Editor, DocxtorEditor, t, toast, ZoomControl, type DocumentSource } from "@docxtor/editor";
import { cleanPrefs, DEFAULT_PREFS, PREFS_KEY, type AppPrefs } from "./settings";
import { baseName, canPickFiles, canUseFolders, DOC_EXT, join, SingleFileFs, type Backend, type Recent } from "./files/fs";
import { dbNotices } from "./files/db";
import { checkBrowser, dataFromNewerVersion, startFailed, warnIfNotSecure, watchForEngineCrash, watchForErrors } from "./failsafe";
import { parseRoute } from "./route";
import { type Version, type VersionKind } from "./files/versions";
import { AccessibilityPanel } from "./ui/accessibility";
import { AIPanel } from "./ui/ai-panel";
import { storage } from "./storage";
import { SideBySide } from "./side-by-side";
import { SaveQueue } from "./save-queue";
import { FilesPanel } from "./ui/files-panel";
import { HistoryPanel, type DiffSummary } from "./ui/history-panel";
import { Sidebar } from "./ui/sidebar";
import { RECOVERY_DELAYS, type DocRef, type SaveState, type Taken } from "./app-shared";
import { showStart } from "./start-screen";
import { addStatus, darkButton, manageExtensions, mount, pickAppLanguage, pickIconPack, pickTheme, setState, setTitle, showFidelity, toggleAutosave, toggleDark, updateWords } from "./app-chrome.ts";
import { createFromSource, createIn, importFiles, load, newFromTemplate, openDropped, openFromUrl, openPicker, openRecent, openRef, pickFileFromComputer, protect, readRecent, sample, syncUrl, unlockBytes } from "./app-open.ts";
import { accept, checkpoint, diff, download, downloadAll, downloadAs, exportAs, makeCopy, markDirty, newQueue, onRemoved, onRenamed, onRestored, renameCurrent, renameDocument, restoreVersion, save, saveAs, stampAuthor } from "./app-save.ts";
import { compareDocuments, countRecoverable, offerRecovery, onRecentsChange, openRecentKey, recentPlace, rememberRecent, showDetails, showRecovered, showTrash, togglePin, togglePinOf, viewSideBySide } from "./app-recent.ts";



export class App {
  engine: Engine | null = null;
  ref: DocRef | null = null;
  /** The editor component (toolbar, pages, in-page tools); the app arranges its pieces. */
  fe!: DocxtorEditor;
  /** View side by side: another document beside the open one. */
  side!: SideBySide;
  editor!: Editor;
  a11y!: AccessibilityPanel;
  ai!: AIPanel;
  files!: FilesPanel;
  root!: HTMLElement;
  history!: HistoryPanel;
  sidebar!: Sidebar;
  original: Uint8Array | null = null;
  /** Saving the open document: one write at a time, and "saved" means the written revision is the engine's revision now. */
  queue: SaveQueue<Taken> | null = null;
  lastVersionAt = 0;
  dirtySinceVersion = false;
  /** The document is open in another window, which saves it: this one never writes it (nor a recovery copy). */
  savedElsewhere = false;
  saveTimer = 0;
  titleInput!: HTMLInputElement;
  led!: HTMLElement;
  /** The padlock beside the save state when the document is protected with a password. */
  lockMark!: HTMLElement;
  /** Where the open document lives, in words ("your Library", "Reports/2026", "a file on disk"). */
  where = "";
  saveState: SaveState = "saved";
  pageReadout!: HTMLElement;
  /** Says in the status bar that the document is protected or marked as final (empty otherwise). */
  protectStat!: HTMLElement;
  wordsReadout!: HTMLElement;
  fidelityStat!: HTMLElement;
  zoom!: ZoomControl;
  aiBtn!: HTMLButtonElement;
  saveBtn!: HTMLButtonElement;
  fileInput!: HTMLInputElement;
  recentListeners = new Set<() => void>();
  prefs: AppPrefs = cleanPrefs(storage<Partial<AppPrefs>>(PREFS_KEY, DEFAULT_PREFS));
  /** The footer of the window, where a line of status goes (the offline note). */
  footer!: HTMLElement;
  readonly canUseFolders = canUseFolders;
  readonly canPickFiles = canPickFiles;

  mount(root: HTMLElement) { return mount(this, root); }

  // ---------------------------------------------------------------------------
  // Chrome
  // ---------------------------------------------------------------------------



  /** Pick the interface language. Docxtor reloads to rebuild the interface in it. */
  async pickLanguage() { return pickAppLanguage(this); }


  /** Show a navigator panel (opening the navigator if it's closed). */
  showPanel(id: string) {
    this.sidebar.toggle(true);
    this.sidebar.show(id);
  }

  /** Files on disk: save on every change, or only on ⌘S. */
  toggleAutosave() { return toggleAutosave(this); }

  /** Choose a theme: the system's, light, dark, or one an extension added. Remembered. */
  async pickTheme() { return pickTheme(this); }

  /** Choose an icon pack: Docxtor's, the light one, or one an extension added. Remembered. */
  async pickIconPack() { return pickIconPack(this); }
  /** Light if the app shows dark, dark if it shows light. Remembered. */
  toggleDark() { toggleDark(this); }
  /** A light and dark switch button, with the given classes. */
  darkButton(className: string) { return darkButton(this, className); }



  manageExtensions() { return manageExtensions(this); }

  /** Every format Docxtor can open, for file pickers. */
  accept(): string { return accept(this); }

  /** Download the open document in another format. */
  async exportAs(id: string) { return exportAs(this, id); }

  /** Pick an export format, then download. */
  async downloadAs() { return downloadAs(this); }

  /** Every document of the Library in one .zip file, in its folders: a backup a person can keep outside the browser. */
  async downloadAll() { return downloadAll(this); }

  /** Pick a template (a built-in one, one you saved, or a file on your computer) and start a document from it in the Library. */
  async newFromTemplate() { return newFromTemplate(this); }

  /**
   * Make a .docx from an importer or template result and open it from the Library. A Word template (.dotx) becomes a
   * document, made now, as Word makes one from a template.
   */
  async createFromSource(source: DocumentSource, name: string, backend: Backend = this.files.library, dir = "") { return createFromSource(this, source, name, backend, dir); }

  /** A new document is by the person whose name is in Tools, User information (instead of "Docxtor"). */
  stampAuthor() { return stampAuthor(this); }

  setTitle(name: string) { return setTitle(this, name); }




  /** Put a line of status in the status bar, before the zoom. */
  addStatus(el: HTMLElement) { return addStatus(this, el); }



  setState(s: SaveState) { return setState(this, s); }



  // ---------------------------------------------------------------------------
  // Documents
  // ---------------------------------------------------------------------------

  /** The open document has changes that no finished write holds (the engine's revision is not the one last written). */
  get dirty(): boolean {
    return this.queue?.dirty ?? false;
  }

  docKey(): string | null {
    return this.ref ? `${this.ref.backend.id}:${this.ref.path}` : null;
  }

  fileName(): string {
    return this.ref ? baseName(this.ref.path) : this.engine?.fileName ?? "Untitled.docx";
  }

  stem(): string {
    return this.fileName().replace(DOC_EXT, "");
  }

  /**
   * Swap the open document, always in this order: stop what looks at the old engine (the split window, with the view it
   * left; the old save queue), hand the new engine to the editor, and only then dispose the old one.
   */
  load(engine: Engine, ref: DocRef, original: Uint8Array | null) { return load(this, engine, ref, original); }

  /** View, New window: the same document in a new browser window (read-only there while this one has it open). */
  newWindow() {
    window.open(location.href, "_blank", "noopener");
  }


  /** The bytes of a protected file as a plain .docx (asking for the password); null when the person cancels. */
  async unlockBytes(bytes: Uint8Array, name: string, known?: string | null): Promise<Uint8Array | null> { return unlockBytes(this, bytes, name, known); }



  async openRef(ref: DocRef, password?: string | null) { return openRef(this, ref, password); }

  // ---------------------------------------------------------------------------
  // Addresses: every document has its own URL
  // ---------------------------------------------------------------------------

  /** True while opening a document because the address changed (so we don't push a new entry). */
  navigating = false;


  /** Storage an extension provides (by id), if it's on. */
  provider(id: string): Backend | undefined {
    return this.fe.extensions.storage.list().find((p) => p.id === id);
  }

  isProvided(b: Backend): boolean {
    return this.fe.extensions.storage.list().includes(b);
  }

  /** Point the address bar at the open document (a new history entry, unless replacing). */
  syncUrl(replace = false) { return syncUrl(this, replace); }

  /** Open whatever the address names: a document, or the start screen. */
  async openFromUrl() { return openFromUrl(this); }


  /** Record the open document in Recents, with a text preview of its first page. */
  rememberRecent(pinned = false) { return rememberRecent(this, pinned); }

  /** Called after the list of recent documents changes (a document opened, saved or renamed). */
  onRecentsChange(fn: () => void): { dispose(): void } { return onRecentsChange(this, fn); }

  /** Where a recent document lives, in words. */
  recentPlace(r: Recent): string { return recentPlace(this, r); }

  /** Open one of the recent documents by its key (from the Open recent menu). */
  async openRecentKey(key: string) { return openRecentKey(this, key); }

  /** Whether the open document can be copied next to itself or renamed (a file opened from disk can't). */
  get canManageFile(): boolean {
    return !!this.ref && !!this.engine && !(this.ref.backend instanceof SingleFileFs);
  }

  /** Write the open document to "Copy of {name}" in the same place, then open the copy. */
  async makeCopy() { return makeCopy(this); }

  /** Put the cursor in the document name and select it; leaving the field does the renaming. */
  renameDocument() { return renameDocument(this); }

  /**
   * Tools, Compare documents: pick another version, then show the differences. Combine documents is the same with the
   * open document as the original and the one picked as the revised version. How they are compared is up to the
   * comparers extensions register (Docxtor's own is the built-in "Compare and combine").
   */
  async compareDocuments(mode: "compare" | "combine" = "compare") { return compareDocuments(this, mode); }



  /** View side by side: another document beside this one; again, it closes. */
  async viewSideBySide() { return viewSideBySide(this); }

  /** The bytes of a recent document; null when the browser's permission is refused. */
  async readRecent(r: Recent): Promise<Uint8Array | null> { return readRecent(this, r); }

  /** A file input for one document (converted to .docx when it is another format); null if the person backs out. */
  pickFileFromComputer(): Promise<{ name: string; bytes: Uint8Array } | null> { return pickFileFromComputer(this); }

  /** The Document details dialog. */
  showDetails() { return showDetails(this); }

  /** File, Protect with password: set, change or remove the document's password. The next save writes it (or leaves it out). */
  async protect() { return protect(this); }

  /** Make a new document in `dir` (from Markdown, if given) and open it, unless `open` is false; resolves to its path. */
  async createIn(backend: Backend, dir: string, name = "Untitled.docx", markdown?: string, opts: { open?: boolean } = {}): Promise<string> { return createIn(this, backend, dir, name, markdown, opts); }


  /**
   * Help, Open the tour in a new tab: the Welcome document opens in a tab of its own, so the one being worked on stays
   * where it is (as Google Docs opens Help and new documents). From the start screen (`newTab` false) it opens here;
   * when the browser won't open a tab it opens here too, after the document is saved, and says so. A tour no one has
   * changed is opened again, not copied.
   */
  async sample(newTab = true) { return sample(this, newTab); }


  /** Copy files into `dir`; formats other than .docx are converted by their importer first. */
  async importFiles(backend: Backend, dir: string, files: File[]) { return importFiles(this, backend, dir, files); }


  /** Open a file with save-in-place when the browser allows it, else import into the Library. */
  async openPicker() { return openPicker(this); }

  async openDropped(item: DataTransferItem | undefined, f: File) { return openDropped(this, item, f); }

  async openRecent(r: Recent) { return openRecent(this, r); }

  /** Save to where the document lives. `explicit` = the user asked (⌘S / button). */
  async save(explicit: boolean) { return save(this, explicit); }

  /** The queue that saves `engine`'s document to wherever the open document lives, from the revision it was saved at. */
  newQueue(engine: Engine, savedRevision: number): SaveQueue<Taken> { return newQueue(this, engine, savedRevision); }


  async saveAs() { return saveAs(this); }

  markDirty() { return markDirty(this); }


  recoveryTimer = 0;
  /** When the changes that are waiting for a copy began. */
  recoverySince = 0;
  /** How long to wait; the tests lower these. */
  recoveryDelays = { ...RECOVERY_DELAYS };



  /** Record a version of the current document. */
  async checkpoint(kind: VersionKind, label: string, force = false, model?: string, bytes?: Uint8Array) { return checkpoint(this, kind, label, force, model, bytes); }

  diff(older: Uint8Array, newer: Uint8Array | null): DiffSummary | null { return diff(this, older, newer); }

  async restoreVersion(v: Version, bytes: Uint8Array) { return restoreVersion(this, v, bytes); }

  async renameCurrent(value: string) { return renameCurrent(this, value); }

  onRenamed(backend: Backend, from: string, to: string) { return onRenamed(this, backend, from, to); }

  onRemoved(backend: Backend, path: string) { return onRemoved(this, backend, path); }

  /** The place a deleted document came from, if it can be written to right now. */
  backendFor(id: string): Backend | undefined {
    if (id === "library") return this.files.library;
    if (this.files.folder && this.files.folder.id === id) return this.files.folder;
    return this.fe.extensions.storage.list().find((p) => p.id === id);
  }

  /** A document came back from the Trash: the lists show it again. */
  onRestored(_backend: Backend, _path: string) { return onRestored(this, _backend, _path); }

  // ---------------------------------------------------------------------------
  // Pinned documents
  // ---------------------------------------------------------------------------


  /** Whether the open document is pinned (the File menu's check). */
  pinned = false;

  /** Pin a document to the start screen or let go of it. Returns whether it is pinned now. */
  async togglePinOf(backend: Backend, path: string): Promise<boolean> { return togglePinOf(this, backend, path); }

  /** File, Pin to the start screen. */
  async togglePin() { return togglePin(this); }

  // ---------------------------------------------------------------------------
  // Recovered documents
  // ---------------------------------------------------------------------------

  /** Copies of documents whose changes were lost when a page closed badly, for the File menu's line. */
  recoverable = 0;


  async countRecoverable() { return countRecoverable(this); }

  /** File, Trash: the navigator's Files view with the trash list open. */
  async showTrash() { return showTrash(this); }

  /** File, Recovered documents. */
  showRecovered() { return showRecovered(this); }

  /** At start: when a page closed before its changes were saved, say so and offer them back. */
  async offerRecovery() { return offerRecovery(this); }

  download(name: string, bytes: Uint8Array, mime = "application/vnd.openxmlformats-officedocument.wordprocessingml.document") { return download(this, name, bytes, mime); }

  showFidelity() { return showFidelity(this); }

  wordsTimer = 0;
  updateWords() { return updateWords(this); }
}

async function main() {
  const root = document.getElementById("app")!;
  const cannot = checkBrowser();
  if (cannot) return startFailed(root, new Error(cannot));
  watchForErrors();
  watchForEngineCrash();
  // The data in this browser was saved by a newer Docxtor than this page: say so and leave it alone (see NewerDataError).
  dbNotices.newer = () => dataFromNewerVersion(document.getElementById("app") ?? document.body);
  // The browser keeps one copy of the database for every Docxtor tab, so an update has to wait for the tabs on the old version.
  dbNotices.waiting = () => toast(t("Docxtor is updating. Close your other Docxtor tabs, or reload them, and this one will carry on."), 30000);
  const app = new App();
  // Unsaved changes can no longer be kept in the browser from this page: say how to keep them before reloading.
  dbNotices.outdated = () =>
    toast(
      app.dirty
        ? t("Docxtor was updated in another tab, so this page can't save any more. Use File, Download a copy to keep your changes, then reload this page.")
        : t("Docxtor was updated in another tab. Reload this page to use the new version."),
      60000,
    );
  app.mount(root);
  await app.fe.ready;
  const initial = parseRoute(location.hash);
  if (initial.kind === "home") void showStart(app);
  await loadEngine();
  (window as unknown as { docxtor: App }).docxtor = app;
  warnIfNotSecure();
  window.addEventListener("hashchange", () => void app.openFromUrl());
  void app.offerRecovery();
  // Dev convenience: ?open=rich.docx imports a file from ../fixtures into the Library.
  const fixture = new URLSearchParams(location.search).get("open");
  if (import.meta.env.DEV && fixture) {
    const res = await fetch(`/@fs${import.meta.env.VITE_FIXTURES ?? ""}/${fixture}`).catch(() => null);
    if (res?.ok) {
      const name = baseName(fixture);
      await app.files.library.mkdir("Fixtures");
      await app.files.library.write(join("Fixtures", name), new Uint8Array(await res.arrayBuffer()));
      // Drop ?open= so a reload reopens the document from its own address instead of re-importing.
      history.replaceState(null, "", location.pathname);
      await app.openRef({ backend: app.files.library, path: join("Fixtures", name) });
      return;
    }
  }
  if (initial.kind !== "home") await app.openFromUrl();
}

main().catch((e) => startFailed(document.getElementById("app") ?? document.body, e));
