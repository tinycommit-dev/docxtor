// Citation styles people add themselves: a .csl file they choose, or the address of one (the Zotero style
// repository has thousands). The style is checked with the CSL processor, kept in this browser with the other
// settings of this extension, and offered again under "Your styles" after a reload. Turn the extension off and the
// picker offers none of this.
import { timeLimit, defineExtension, Engine, h, modal, msg, t, toast, type Disposable, type Extension } from "@docxtor/editor";

/** The largest style that is kept. Real styles are well under this; the biggest in the repository is a few hundred KB. */
export const MAX_STYLE_BYTES = 1_000_000;

interface Stored {
  id: string;
  title: string;
  csl: string;
}

/** Check a style with the CSL processor; the error is the processor's own plain message. */
export function checkStyle(csl: string, id = "check"): { title: string } {
  if (csl.length > MAX_STYLE_BYTES) throw new Error(t("That style is larger than 1 MB, which is more than Docxtor keeps."));
  const probe = Engine.blank("check.docx");
  try {
    return probe.registerCitationStyle(id, csl);
  } finally {
    probe.dispose();
  }
}

/** A short id from the style's title, made unique among the ids taken. */
function makeId(title: string, taken: Set<string>): string {
  const slug =
    title
      .toLowerCase()
      .normalize("NFKD")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 40) || "style";
  let id = `user-${slug}`;
  for (let n = 2; taken.has(id); n++) id = `user-${slug}-${n}`;
  return id;
}

export function userStyles(): Extension {
  return defineExtension({
    id: "docxtor.citations.userStyles",
    name: msg("Your own citation styles"),
    description: msg("Add a citation style from a CSL file or from its address, and keep it in this browser. An address you give is fetched from your browser."),
    activate(ctx) {
      const stored = (): Stored[] => ctx.state.get<Stored[]>("styles", []);
      const live = new Map<string, Disposable>();

      const show = (s: Stored) => live.set(s.id, ctx.citations.styles.register({ id: s.id, label: s.title, group: t("Your styles"), csl: s.csl }));
      for (const s of stored()) if (s && typeof s.csl === "string" && s.id) show(s);

      /** Keep a checked style and list it. A style with the same title replaces the one before. */
      const keep = (csl: string): string => {
        const { title } = checkStyle(csl);
        const all = stored();
        const same = all.find((s) => s.title === title);
        const id = same?.id ?? makeId(title, new Set(all.map((s) => s.id)));
        const entry = { id, title, csl };
        ctx.state.set("styles", [...all.filter((s) => s.id !== id), entry]);
        live.get(id)?.dispose();
        show(entry);
        return title;
      };

      const remove = (id: string) => {
        ctx.state.set("styles", stored().filter((s) => s.id !== id));
        live.get(id)?.dispose();
        live.delete(id);
      };

      const add = () => {
        const file = h("input", { type: "file", accept: ".csl,.xml,text/xml", hidden: true, "aria-label": t("Style file") });
        const choose = h("button", { class: "btn", type: "button" }, t("Choose a .csl file…"));
        const address = h("input", { type: "text", id: "user-style-url", autocomplete: "off", spellcheck: "false", placeholder: "https://…/nature.csl" });
        const go = h("button", { class: "btn primary", type: "submit" }, t("Add from address"));
        const status = h("p", { class: "bm-error", role: "alert", hidden: true });
        const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
        const form = h(
          "form",
          { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Add a citation style"), style: "width:min(var(--dialog-m),100%)", novalidate: true },
          h("div", { class: "dialog-head" }, h("h2", {}, t("Add a citation style")), h("p", { class: "sub" }, t("Choose a CSL file, or give the address of one, for example from the Zotero style repository."))),
          h(
            "div",
            { class: "dialog-body" },
            h("div", { class: "field" }, choose, file),
            h("div", { class: "field" }, h("label", { for: "user-style-url" }, t("Address of a .csl file")), address, h("span", { class: "help" }, t("Docxtor fetches this address from your browser. Nothing else is sent.")), status),
          ),
          h("div", { class: "dialog-actions" }, cancel, go),
        );
        const close = modal(form);
        cancel.addEventListener("click", close);
        const finish = (csl: string) => {
          try {
            const title = keep(csl);
            close();
            toast(t("Added the style {title}.", { title }));
          } catch (e) {
            status.textContent = String((e as Error).message ?? e);
            status.hidden = false;
          }
        };
        choose.addEventListener("click", () => file.click());
        file.addEventListener("change", async () => {
          const f = file.files?.[0];
          file.value = "";
          if (!f) return;
          status.hidden = true;
          if (f.size > MAX_STYLE_BYTES) {
            status.textContent = t("That style is larger than 1 MB, which is more than Docxtor keeps.");
            status.hidden = false;
            return;
          }
          finish(await f.text());
        });
        address.addEventListener("input", () => (status.hidden = true));
        form.addEventListener("submit", async (e) => {
          e.preventDefault();
          status.hidden = true;
          let url: URL;
          try {
            url = new URL(address.value.trim());
            if (url.protocol !== "https:" && url.protocol !== "http:") throw new Error("not http");
          } catch {
            status.textContent = t("That doesn't look like a web address. It should start with https://.");
            status.hidden = false;
            return;
          }
          go.disabled = true;
          go.textContent = t("Fetching…");
          try {
            let res: Response;
            try {
              res = await fetch(url.toString(), { signal: timeLimit(20000) });
            } catch {
              throw new Error(t("Couldn't fetch that address. Check it and your connection, and try again."));
            }
            if (!res.ok) throw new Error(t("That address didn't answer (error {status}).", { status: res.status }));
            finish(await res.text());
          } catch (err) {
            status.textContent = String((err as Error).message ?? err);
            status.hidden = false;
          } finally {
            go.disabled = false;
            go.textContent = t("Add from address");
          }
        });
        choose.focus();
      };

      const manage = () => {
        const list = h("ul", { class: "src-list" });
        const draw = () => {
          const all = stored();
          list.replaceChildren(
            ...(all.length
              ? all.map((s) => {
                  const b = h("button", { class: "btn sm", type: "button", "aria-label": t("Remove {title}", { title: s.title }) }, t("Remove"));
                  b.addEventListener("click", async () => {
                    if (!(await ctx.ask("citations.removeStyle", s.title))) return;
                    remove(s.id);
                    draw();
                  });
                  return h("li", { class: "src-item", "data-id": s.id }, h("div", { class: "src-title" }, s.title), h("div", { class: "src-actions" }, b));
                })
              : [h("li", { class: "empty-note" }, t("You haven't added any styles."))]),
          );
        };
        draw();
        const done = h("button", { class: "btn primary", type: "button" }, t("Done"));
        const dlg = h(
          "div",
          { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Your styles"), style: "width:min(var(--dialog-m),100%)" },
          h("div", { class: "dialog-head" }, h("h2", {}, t("Your styles")), h("p", { class: "sub" }, t("Styles you added from a file or an address are kept in this browser."))),
          h("div", { class: "dialog-body" }, list),
          h("div", { class: "dialog-actions" }, done),
        );
        const close = modal(dlg);
        done.addEventListener("click", close);
      };

      ctx.commands.register("citations.addStyle", add, { title: t("Add a style from a CSL file…"), category: t("Tools"), palette: false });
      ctx.commands.register("citations.manageStyles", manage, { title: t("Manage styles…"), category: t("Tools"), palette: false, when: () => stored().length > 0 });
    },
  });
}
