// File explorer: the in-browser Library plus an optional connected folder on disk.
import { canUseFolders, DOC_EXT, ensurePermission, hasPermission, join, LibraryFs, FolderFs, rememberFolder, rememberedFolder, type Backend, type FsNode } from "../files/fs";
import { h, popover, promptDialog, svg, t, toast } from "@docxtor/editor";
import { I } from "@docxtor/editor";
import { deleteToTrash, KEEP_DAYS } from "../files/trash";
import { TrashSection, type TrashHost } from "./trash";

export interface FilesHost {
  /** Ask before a risky action, by its caution id (web/src/cautions.ts): true to go ahead. */
  ask(id: string, ...args: unknown[]): Promise<boolean>;
  /** Currently open document, if any. */
  current(): { backend: Backend; path: string } | null;
  open(backend: Backend, path: string): Promise<void>;
  create(backend: Backend, dir: string): Promise<void>;
  importFiles(backend: Backend, dir: string, files: File[]): Promise<void>;
  renamed(backend: Backend, from: string, to: string): void;
  removed(backend: Backend, path: string): void;
  download(backend: Backend, path: string): Promise<void>;
  /** Storage that extensions provide (shown after the Library and the folder). */
  providers(): Backend[];
  /** Whether a file can be imported (a .docx, or a format an importer reads). */
  canOpen(name: string): boolean;
  /** File-picker `accept` for importing. */
  accept(): string;
  /** Pin a document to the start screen, or let go of it. */
  togglePin(backend: Backend, path: string): Promise<boolean>;
  /** Whether a document is pinned. */
  isPinned(backend: Backend, path: string): Promise<boolean>;
  /** What the trash needs from the app. */
  trash: Omit<TrashHost, "ask">;
}

const DIR_ICON = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M3 7.5V18a1.5 1.5 0 0 0 1.5 1.5h15A1.5 1.5 0 0 0 21 18V9a1.5 1.5 0 0 0-1.5-1.5H12L10 5H4.5A1.5 1.5 0 0 0 3 6.5z"/></svg>`;
const DOC_ICON = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M14 3H7a1.5 1.5 0 0 0-1.5 1.5v15A1.5 1.5 0 0 0 7 21h10a1.5 1.5 0 0 0 1.5-1.5V7.5z"/><path d="M14 3v4.5h4.5M9 12.5h6M9 16h4"/></svg>`;

function ago(t?: number): string {
  if (!t) return "";
  const s = (Date.now() - t) / 1000;
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  if (s < 86400 * 30) return `${Math.floor(s / 86400)}d`;
  return new Date(t).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export class FilesPanel {
  el: HTMLElement;
  readonly library = new LibraryFs();
  folder: FolderFs | null = null;
  private folderNeedsPermission = false;
  private open = new Set<string>([""]);
  /** Resolves once the remembered folder (if any) is restored. */
  readonly ready: Promise<void>;
  private trash: TrashSection;

  constructor(private host: FilesHost) {
    this.el = h("div", { class: "files" });
    this.trash = new TrashSection({
      ask: (id, ...args) => host.ask(id, ...args),
      backendFor: (id) => host.trash.backendFor(id),
      restored: (b, p) => {
        host.trash.restored(b, p);
        void this.render();
      },
    });
    this.ready = this.init();
  }

  private async init() {
    const handle = await rememberedFolder();
    if (handle) {
      this.folder = new FolderFs(handle);
      this.folderNeedsPermission = !(await hasPermission(handle));
    }
    await this.render();
  }

  async connectFolder() {
    try {
      const picker = (window as unknown as { showDirectoryPicker(o?: object): Promise<FileSystemDirectoryHandle> }).showDirectoryPicker;
      const dir = await picker({ mode: "readwrite", id: "docxtor-folder" });
      this.folder = new FolderFs(dir);
      this.folderNeedsPermission = false;
      await rememberFolder(dir);
      await this.render();
    } catch (e) {
      if ((e as Error).name !== "AbortError") toast(t("Couldn't open the folder: {reason}", { reason: (e as Error).message }));
    }
  }

  async refresh() {
    await this.render();
  }

  /** Show the Trash list, open. */
  async showTrash() {
    this.trash.show();
    await this.render();
  }

  private key(b: Backend, path: string) {
    return `${b.id}|${path}`;
  }

  async render() {
    // Reveal the open document: expand the folders that contain it.
    const cur = this.host.current();
    if (cur) {
      const parts = cur.path.split("/").slice(0, -1);
      for (let i = 1; i <= parts.length; i++) this.open.add(this.key(cur.backend, parts.slice(0, i).join("/")));
    }
    const sections: HTMLElement[] = [];
    sections.push(await this.section(this.library, t("Library"), t("Stored in this browser")));
    if (this.folder) {
      if (this.folderNeedsPermission) {
        const btn = h("button", { class: "btn", type: "button" }, svg(I.open), t("Reconnect “{folder}”", { folder: this.folder.label }));
        btn.addEventListener("click", async () => {
          if (this.folder && (await ensurePermission(this.folder.root))) {
            this.folderNeedsPermission = false;
            await this.render();
          }
        });
        sections.push(h("div", {}, h("div", { class: "panel-title" }, t("Folder")), h("div", { class: "fs-actions" }, btn)));
      } else sections.push(await this.section(this.folder, this.folder.label, t("Folder on this computer")));
    }
    for (const p of this.host.providers()) sections.push(await this.section(p, p.label, (p as { description?: string }).description ?? p.label));
    const actions = h("div", { class: "fs-actions" });
    if (canUseFolders) {
      const b = h("button", { class: "btn ghost", type: "button" }, svg(I.open), this.folder ? t("Switch folder…") : t("Open a folder…"));
      b.addEventListener("click", () => void this.connectFolder());
      actions.append(b);
    } else {
      actions.append(h("div", { class: "empty-note" }, t("To work directly in a folder on disk, use a Chromium browser (Chrome, Edge, Arc, Brave).")));
    }
    await this.trash.render();
    this.el.replaceChildren(...sections, actions, this.trash.el);
  }

  private async section(fs: Backend, title: string, hint: string): Promise<HTMLElement> {
    const add = h("button", { class: "tb", type: "button", title: t("New document"), "aria-label": t("New document in {place}", { place: title }) }, svg(I.filePlus));
    add.addEventListener("click", () => void this.host.create(fs, ""));
    const more = h("button", { class: "tb", type: "button", title: t("More"), "aria-label": t("{place} actions", { place: title }) }, svg(I.more));
    more.addEventListener("click", () => this.dirMenu(more, fs, ""));
    const head = h("div", { class: "panel-title", title: hint }, h("span", {}, title), h("span", { style: "display:flex;gap:2px" }, add, more));
    const body = h("div", {});
    try {
      await this.fill(body, fs, "");
    } catch (e) {
      body.append(h("div", { class: "empty-note" }, t("Couldn't read it: {reason}", { reason: (e as Error).message })));
    }
    const wrap = h("section", { class: "fs-section" }, head, body);
    // Drop .docx files onto a section to import them.
    wrap.addEventListener("dragover", (e) => {
      if (e.dataTransfer?.types.includes("Files")) {
        e.preventDefault();
        e.stopPropagation();
      }
    });
    wrap.addEventListener("drop", (e) => {
      const files = Array.from(e.dataTransfer?.files ?? []).filter((f) => this.host.canOpen(f.name));
      if (!files.length) return;
      e.preventDefault();
      e.stopPropagation();
      void this.host.importFiles(fs, "", files).then(() => this.render());
    });
    return wrap;
  }

  private async fill(container: HTMLElement, fs: Backend, dir: string) {
    const nodes = (await fs.list(dir)).filter((n) => n.kind === "dir" || DOC_EXT.test(n.name));
    if (!nodes.length && dir === "") {
      container.append(h("div", { class: "empty-note" }, fs.id === "library" ? t("No documents yet. New documents and imports land here.") : t("No Word documents in this folder.")));
      return;
    }
    const cur = this.host.current();
    for (const n of nodes) container.append(await this.row(fs, n, cur));
  }

  private async row(fs: Backend, n: FsNode, cur: { backend: Backend; path: string } | null): Promise<HTMLElement> {
    const isOpen = this.open.has(this.key(fs, n.path));
    const btn = h(
      "button",
      { class: `fs-item${n.kind === "dir" ? " dir" : ""}`, type: "button", title: n.path, "aria-expanded": n.kind === "dir" ? String(isOpen) : undefined },
      svg(n.kind === "dir" ? DIR_ICON : DOC_ICON),
      h("span", { class: "nm" }, n.kind === "file" ? n.name.replace(DOC_EXT, "") : n.name),
      n.kind === "file" ? h("span", { class: "meta" }, ago(n.modified)) : null,
    );
    if (cur && cur.backend.id === fs.id && cur.path === n.path) btn.classList.add("active");
    const wrap = h("div", {}, btn);
    if (n.kind === "dir") {
      const kids = h("div", { class: "fs-children" });
      if (isOpen) await this.fill(kids, fs, n.path);
      wrap.append(kids);
      btn.addEventListener("click", async () => {
        const k = this.key(fs, n.path);
        if (this.open.has(k)) {
          this.open.delete(k);
          kids.replaceChildren();
          btn.setAttribute("aria-expanded", "false");
        } else {
          this.open.add(k);
          await this.fill(kids, fs, n.path);
          btn.setAttribute("aria-expanded", "true");
        }
      });
    } else {
      btn.addEventListener("click", () => void this.host.open(fs, n.path));
    }
    btn.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      if (n.kind === "dir") this.dirMenu(btn, fs, n.path);
      else this.fileMenu(btn, fs, n);
    });
    return wrap;
  }

  private dirMenu(anchor: HTMLElement, fs: Backend, dir: string) {
    const list = h("div", {});
    const close = popover(anchor, list, { align: "right" });
    const item = (label: string, fn: () => void | Promise<void>) => {
      const b = h("button", { class: "menu-item", type: "button" }, label);
      b.addEventListener("click", () => {
        close();
        void fn();
      });
      list.append(b);
    };
    item(t("New document"), () => this.host.create(fs, dir));
    item(t("New folder"), async () => {
      const name = await promptDialog({ title: t("New folder"), label: t("Name"), value: t("New folder"), ok: t("Create") });
      if (!name) return;
      await fs.mkdir(join(dir, name));
      this.open.add(this.key(fs, dir));
      await this.render();
    });
    item(t("Import files…"), async () => {
      const input = h("input", { type: "file", accept: this.host.accept(), multiple: true });
      input.addEventListener("change", () => void this.host.importFiles(fs, dir, Array.from(input.files ?? [])).then(() => this.render()));
      input.click();
    });
    if (dir) {
      list.append(h("div", { class: "menu-sep" }));
      item(t("Delete folder…"), async () => {
        if (!(await this.host.ask("files.deleteFolder", dir, KEEP_DAYS))) return;
        await deleteToTrash(fs, dir, "dir");
        this.host.removed(fs, dir);
        await this.render();
      });
    }
  }

  private fileMenu(anchor: HTMLElement, fs: Backend, n: FsNode) {
    const list = h("div", {});
    const close = popover(anchor, list, { align: "right" });
    const item = (label: string, fn: () => void | Promise<void>, cls = "") => {
      const b = h("button", { class: `menu-item ${cls}`, type: "button" }, label);
      b.addEventListener("click", () => {
        close();
        Promise.resolve(fn()).catch((e) => toast((e as Error).message));
      });
      list.append(b);
    };
    item(t("Open"), () => this.host.open(fs, n.path));
    item(t("Rename…"), async () => {
      const stem = n.name.replace(DOC_EXT, "");
      const ext = n.name.slice(stem.length) || ".docx";
      const name = await promptDialog({ title: t("Rename document"), label: t("Name"), value: stem, ok: t("Rename") });
      if (!name || name === stem) return;
      const to = await fs.rename(n.path, name + ext);
      this.host.renamed(fs, n.path, to);
      await this.render();
    });
    item(t("Duplicate"), async () => {
      const bytes = await fs.read(n.path);
      const stem = n.name.replace(DOC_EXT, "");
      let name = `${t("{name} copy", { name: stem })}.docx`;
      for (let i = 2; await fs.exists(join(n.path.slice(0, Math.max(0, n.path.lastIndexOf("/"))), name)); i++) name = `${t("{name} copy {n}", { name: stem, n: i })}.docx`;
      await fs.write(join(n.path.slice(0, Math.max(0, n.path.lastIndexOf("/"))), name), bytes);
      await this.render();
    });
    item(t("Download a copy"), () => this.host.download(fs, n.path));
    // The label is read when the menu opens, so it says what a click will do.
    const pin = h("button", { class: "menu-item", type: "button" }, t("Pin to the start screen"));
    list.append(pin);
    void this.host.isPinned(fs, n.path).then((on) => (pin.textContent = on ? t("Unpin from the start screen") : t("Pin to the start screen")));
    pin.addEventListener("click", () => {
      close();
      Promise.resolve(this.host.togglePin(fs, n.path)).catch((e) => toast((e as Error).message));
    });
    list.append(h("div", { class: "menu-sep" }));
    item(t("Delete…"),
      async () => {
        if (!(await this.host.ask("files.delete", n.name, KEEP_DAYS))) return;
        await deleteToTrash(fs, n.path, "file");
        this.host.removed(fs, n.path);
        await this.render();
      },
      "danger",
    );
  }
}
