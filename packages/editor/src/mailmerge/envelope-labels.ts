// Mailings, Envelopes and Labels: addresses on an envelope or on a sheet of labels, on their own or as the main document of a
// mail merge. An envelope is added to the document as a new first section with the envelope's page size; a sheet of labels is
// a table made the way Word makes one, in a new document.
import { Engine, type EnvelopeSize, type LabelFill, type LabelSpec, type Op, type TextLook } from "@docxtor/core";
import { t, type ExtensionContext, type LabelProduct } from "@docxtor/extensions";
import { h, modal, toast } from "../ui/dom";
import type { MergeSession } from "./session";

let seq = 0;

/** A label product in twips, the way the engine builds it. */
export function specOf(p: LabelProduct): LabelSpec {
  const f = p.unit === "mm" ? 1440 / 25.4 : p.unit === "pt" ? 20 : 1440;
  const n = (v: number) => Math.round(v * f);
  return { id: p.id, vendor: p.vendor, name: p.name, page_w: n(p.pageWidth), page_h: n(p.pageHeight), top: n(p.top), left: n(p.left), label_w: n(p.labelWidth), label_h: n(p.labelHeight), pitch_h: n(p.pitchH), pitch_v: n(p.pitchV), across: p.across, down: p.down };
}

/** Docxtor's own label products as contributions (sizes in points), for the built-in extension to register. */
export function builtinProducts(): LabelProduct[] {
  return Engine.labelProducts().map((s) => ({
    id: s.id,
    vendor: s.vendor,
    name: s.name,
    unit: "pt" as const,
    pageWidth: s.page_w / 20,
    pageHeight: s.page_h / 20,
    top: s.top / 20,
    left: s.left / 20,
    labelWidth: s.label_w / 20,
    labelHeight: s.label_h / 20,
    pitchH: s.pitch_h / 20,
    pitchV: s.pitch_v / 20,
    across: s.across,
    down: s.down,
  }));
}

const FONTS = ["Calibri", "Cambria", "Arial", "Times New Roman", "Georgia", "Verdana", "Courier New"];

const inch = (tw: number) => String(+(tw / 1440).toFixed(3));
const mmOf = (tw: number) => String(+((tw * 25.4) / 1440).toFixed(1));
const size = (tw: number, metric: boolean) => (metric ? `${mmOf(tw)} mm` : `${inch(tw)} in`);

function fontSelect(id: string, value = "") {
  const sel = h("select", { id }, h("option", { value: "" }, t("Document font")), ...FONTS.map((f) => h("option", { value: f }, f)));
  sel.value = value;
  return sel;
}

function numberInput(id: string, value: number, min = 0, step = "any") {
  return h("input", { type: "number", id, value: String(value), min: String(min), step });
}

// ---------------------------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------------------------

export type LabelMode = "document" | "merge";

export function openLabels(s: MergeSession, ctx: ExtensionContext, mode: LabelMode) {
  const ed = s.x.editor;
  const fe = s.x.fe;
  const id = `mml-${++seq}`;
  const groups = fe.extensions.labelProductGroups();
  const CUSTOM = "custom";
  const vendors = [...groups.map((g) => g.vendor), CUSTOM];
  let vendor = ctx.state.get("labelVendor", groups[0]?.vendor ?? CUSTOM);
  if (!vendors.includes(vendor)) vendor = vendors[0];
  const remembered = ctx.state.get("labelProduct", "");

  const vendorSel = h("select", { id: `${id}-vendor` }, ...groups.map((g) => h("option", { value: g.vendor }, g.vendor)), h("option", { value: CUSTOM }, t("Other (your own size)")));
  const productSel = h("select", { id: `${id}-product` });
  const details = h("p", { class: "help mm-details", "aria-live": "polite" });
  // A size of your own: the numbers of a sheet.
  const custom = {
    unit: h("select", { id: `${id}-unit` }, h("option", { value: "in" }, t("Inches")), h("option", { value: "mm" }, t("Millimetres"))),
    pageW: numberInput(`${id}-pw`, 8.5),
    pageH: numberInput(`${id}-ph`, 11),
    top: numberInput(`${id}-top`, 0.5),
    left: numberInput(`${id}-left`, 0.19),
    w: numberInput(`${id}-lw`, 2.63, 0.1),
    hgt: numberInput(`${id}-lh`, 1, 0.1),
    pitchH: numberInput(`${id}-ph2`, 2.75, 0.1),
    pitchV: numberInput(`${id}-pv`, 1, 0.1),
    across: numberInput(`${id}-across`, 3, 1, "1"),
    down: numberInput(`${id}-down`, 10, 1, "1"),
  };
  const grid = h(
    "div",
    { class: "mm-custom", hidden: true },
    h("div", { class: "field" }, h("label", { for: `${id}-unit` }, t("Measure in")), custom.unit),
    ...(
      [
        [t("Page width"), custom.pageW],
        [t("Page height"), custom.pageH],
        [t("Top margin"), custom.top],
        [t("Side margin"), custom.left],
        [t("Label width"), custom.w],
        [t("Label height"), custom.hgt],
        [t("Horizontal pitch"), custom.pitchH],
        [t("Vertical pitch"), custom.pitchV],
        [t("Labels across"), custom.across],
        [t("Labels down"), custom.down],
      ] as const
    ).map(([label, input]) => h("div", { class: "field" }, h("label", { for: input.id }, label), input)),
  );

  const productsOf = (v: string) => groups.find((g) => g.vendor === v)?.products ?? [];
  const current = (): LabelSpec | null => {
    if (vendorSel.value === CUSTOM) {
      const f = custom.unit.value === "mm" ? 1440 / 25.4 : 1440;
      const n = (i: HTMLInputElement) => Math.round(Number(i.value) * f);
      return { id: "custom", vendor: "", name: t("Your own size"), page_w: n(custom.pageW), page_h: n(custom.pageH), top: n(custom.top), left: n(custom.left), label_w: n(custom.w), label_h: n(custom.hgt), pitch_h: n(custom.pitchH), pitch_v: n(custom.pitchV), across: Math.round(Number(custom.across.value)), down: Math.round(Number(custom.down.value)) };
    }
    const p = productsOf(vendorSel.value).find((x) => x.id === productSel.value);
    return p ? specOf(p) : null;
  };
  const paintDetails = () => {
    const spec = current();
    grid.hidden = vendorSel.value !== CUSTOM;
    if (!spec) {
      details.textContent = "";
      return;
    }
    const metric = vendorSel.value === CUSTOM ? custom.unit.value === "mm" : Math.abs(spec.page_w - 11906) < 30;
    details.textContent = t("{n} labels on a sheet, {across} across and {down} down. Each is {w} by {h}.", { n: spec.across * spec.down, across: spec.across, down: spec.down, w: size(spec.label_w, metric), h: size(spec.label_h, metric) });
  };
  const paintProducts = () => {
    productSel.replaceChildren(...productsOf(vendorSel.value).map((p) => h("option", { value: p.id }, p.name)));
    productSel.hidden = vendorSel.value === CUSTOM;
    if (productsOf(vendorSel.value).some((p) => p.id === remembered)) productSel.value = remembered;
    paintDetails();
  };
  vendorSel.value = vendor;
  vendorSel.addEventListener("change", paintProducts);
  productSel.addEventListener("change", paintDetails);
  for (const i of Object.values(custom)) i.addEventListener("input", paintDetails);
  paintProducts();

  // What goes on the labels (a new document only; a merge fills them from the list).
  let fill: "same" | "single" = "same";
  const fillBox = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Print") });
  const row = numberInput(`${id}-row`, 1, 1, "1");
  const col = numberInput(`${id}-col`, 1, 1, "1");
  const where = h("div", { class: "mm-where", hidden: true }, h("div", { class: "field" }, h("label", { for: `${id}-row` }, t("Row")), row), h("div", { class: "field" }, h("label", { for: `${id}-col` }, t("Column")), col));
  for (const [k, label] of [["same", t("A full page of the same label")], ["single", t("A single label")]] as const) {
    const b = h("button", { type: "button", role: "radio", "aria-checked": String(k === fill), class: k === fill ? "on" : "" }, label);
    b.addEventListener("click", () => {
      fill = k;
      fillBox.querySelectorAll("button").forEach((x) => {
        const on = x === b;
        x.classList.toggle("on", on);
        x.setAttribute("aria-checked", String(on));
      });
      where.hidden = k !== "single";
    });
    fillBox.append(b);
  }
  const selected = ed.engine && !ed.collapsed() ? ed.selectedText().trim() : "";
  const address = h("textarea", { id: `${id}-address`, rows: 5, spellcheck: "false" });
  address.value = selected || ctx.state.get("labelAddress", "");
  const font = fontSelect(`${id}-font`, ctx.state.get("labelFont", ""));
  const sizeInput = numberInput(`${id}-size`, ctx.state.get("labelSize", 10), 4, "0.5");
  const error = h("p", { class: "help mm-error", role: "alert" });
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, mode === "merge" ? t("OK") : t("New document"));

  const body = h(
    "div",
    { class: "dialog-body" },
    h("div", { class: "mm-pair" }, h("div", { class: "field" }, h("label", { for: `${id}-vendor` }, t("Label vendor")), vendorSel), h("div", { class: "field" }, h("label", { for: `${id}-product` }, t("Product number")), productSel)),
    details,
    grid,
    ...(mode === "document" ? [h("div", { class: "field" }, h("label", { for: `${id}-address` }, t("Address")), address), h("div", { class: "field" }, h("label", {}, t("Print")), fillBox), where] : []),
    h("div", { class: "mm-pair" }, h("div", { class: "field" }, h("label", { for: `${id}-font` }, t("Font")), font), h("div", { class: "field" }, h("label", { for: `${id}-size` }, t("Size in points")), sizeInput)),
    error,
  );
  const dialog = h(
    "div",
    { class: "dialog mm-dialog mm-labels", role: "dialog", "aria-modal": "true", "aria-label": t("Labels") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Labels")), h("p", { class: "sub" }, mode === "merge" ? t("Choose the sheet you will print on. Each label takes the next recipient.") : t("Make a sheet of address labels in a new document."))),
    body,
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  ok.addEventListener("click", async () => {
    const spec = current();
    if (!spec) return;
    const look: TextLook = { ...(font.value ? { font: font.value } : {}), size_pt: Number(sizeInput.value) || 10 };
    const lines = address.value.split(/\r?\n/).map((l) => l.trimEnd());
    let labelFill: LabelFill;
    if (mode === "merge") labelFill = { kind: "merge" };
    else if (fill === "single") labelFill = { kind: "single", lines, row: Math.round(Number(row.value)), col: Math.round(Number(col.value)) };
    else labelFill = { kind: "same", lines };
    const op: Op = { op: "make_labels", spec, fill: labelFill, look };
    ctx.state.set("labelVendor", vendorSel.value);
    ctx.state.set("labelProduct", productSel.value);
    ctx.state.set("labelFont", font.value);
    ctx.state.set("labelSize", look.size_pt);
    if (mode === "document") {
      ctx.state.set("labelAddress", address.value);
      try {
        const eng = Engine.blank(t("Labels") + ".docx");
        try {
          eng.apply(op);
          await fe.openNewDocument(eng.save(), `${t("Labels")}.docx`);
        } finally {
          eng.dispose();
        }
        close();
      } catch (e) {
        error.textContent = String((e as Error).message ?? e);
      }
      return;
    }
    // A merge: this document becomes the sheet of labels.
    if (ed.engine && ed.engine.text().trim() && !(await ed.ask("mailMerge.makeLabels"))) return;
    close();
    s.run([op, { op: "set_mail_merge", merge: { main_type: "mailingLabels" } }]);
    toast(t("Insert the address block in the first label, then choose Update labels."), 6000);
  });
}

// ---------------------------------------------------------------------------------------------
// Envelopes
// ---------------------------------------------------------------------------------------------

export type EnvelopeMode = "document" | "merge";

/** Where the engine puts the addresses (see merge::envelope): fractions of the envelope. */
export function envelopeLayout(sizeTw: { w: number; h: number }, returnLines: number, rSize: number) {
  const margin = 360;
  const rline = Math.round(rSize * 1.2 * 20);
  const dx = Math.max(margin, Math.round(sizeTw.w * 0.44));
  const dy = Math.max(margin, Math.round(sizeTw.h * 0.46));
  const used = returnLines ? margin + returnLines * rline : margin;
  return { margin, dx, dy: Math.max(dy, used) };
}

export function openEnvelope(s: MergeSession, ctx: ExtensionContext, mode: EnvelopeMode) {
  const ed = s.x.editor;
  const fe = s.x.fe;
  const id = `mme-${++seq}`;
  const sizes: EnvelopeSize[] = Engine.envelopeSizes();
  const selected = ed.engine && !ed.collapsed() ? ed.selectedText().trim() : "";
  const delivery = h("textarea", { id: `${id}-to`, rows: 4, spellcheck: "false" });
  delivery.value = mode === "merge" ? "" : selected;
  const from = h("textarea", { id: `${id}-from`, rows: 3, spellcheck: "false" });
  from.value = ctx.state.get("returnAddress", fe.fieldOptions.user.address ?? "");
  const omit = h("input", { type: "checkbox", id: `${id}-omit`, checked: ctx.state.get("omitReturn", false) });
  const sizeSel = h("select", { id: `${id}-size` }, ...sizes.map((z) => h("option", { value: z.id }, t(z.name))), h("option", { value: "custom" }, t("Custom size…")));
  sizeSel.value = ctx.state.get("envelopeSize", "10");
  if (!sizeSel.value) sizeSel.value = "10";
  const cw = numberInput(`${id}-cw`, 9.5, 1);
  const ch = numberInput(`${id}-ch`, 4.125, 1);
  const customBox = h("div", { class: "mm-pair", hidden: true }, h("div", { class: "field" }, h("label", { for: `${id}-cw` }, t("Width in inches")), cw), h("div", { class: "field" }, h("label", { for: `${id}-ch` }, t("Height in inches")), ch));
  const toFont = fontSelect(`${id}-tofont`, ctx.state.get("envelopeToFont", ""));
  const toSize = numberInput(`${id}-tosize`, ctx.state.get("envelopeToSize", 12), 4, "0.5");
  const fromFont = fontSelect(`${id}-fromfont`, ctx.state.get("envelopeFromFont", ""));
  const fromSize = numberInput(`${id}-fromsize`, ctx.state.get("envelopeFromSize", 10), 4, "0.5");
  const picture = h("div", { class: "mm-envelope", role: "img", "aria-label": t("A preview of the envelope") });
  const error = h("p", { class: "help mm-error", role: "alert" });
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, mode === "merge" ? t("OK") : t("Add to document"));

  const sizeTw = (): { w: number; h: number } => {
    if (sizeSel.value === "custom") {
      const a = Math.round(Number(cw.value) * 1440);
      const b = Math.round(Number(ch.value) * 1440);
      return { w: Math.max(a, b), h: Math.min(a, b) };
    }
    const z = sizes.find((x) => x.id === sizeSel.value) ?? sizes[0];
    return { w: z.w, h: z.h };
  };
  const lines = (box: HTMLTextAreaElement) => box.value.split(/\r?\n/).map((l) => l.trimEnd()).filter((l, i, a) => l || a.slice(i + 1).some(Boolean));
  const paint = () => {
    customBox.hidden = sizeSel.value !== "custom";
    from.disabled = omit.checked;
    const sz = sizeTw();
    const width = 300;
    const k = width / sz.w;
    const returnLines = omit.checked ? [] : lines(from);
    const rpt = Number(fromSize.value) || 10;
    const dpt = Number(toSize.value) || 12;
    const pos = envelopeLayout(sz, returnLines.length, rpt);
    picture.style.width = `${width}px`;
    picture.style.height = `${Math.round(sz.h * k)}px`;
    const block = (cls: string, text: string[], x: number, y: number, pt: number, family: string) => {
      const el = h("div", { class: `mm-env-text ${cls}` }, ...text.flatMap((l, i) => [i ? h("br") : null, l || " "]));
      el.style.left = `${Math.round(x * k)}px`;
      el.style.top = `${Math.round(y * k)}px`;
      el.style.fontSize = `${Math.max(3, pt * 20 * k).toFixed(2)}px`;
      el.style.lineHeight = "1.2";
      if (family) el.style.fontFamily = `"${family}", Calibri, Carlito, sans-serif`;
      el.dataset.x = String(x);
      el.dataset.y = String(y);
      return el;
    };
    const rY = returnLines.length ? pos.margin : 0;
    picture.replaceChildren(
      ...(returnLines.length ? [block("from", returnLines, pos.margin, rY, rpt, fromFont.value)] : []),
      block("to", mode === "merge" ? [t("«AddressBlock»")] : lines(delivery), pos.dx, pos.dy, dpt, toFont.value),
    );
  };
  for (const el of [delivery, from, omit, sizeSel, cw, ch, toFont, toSize, fromFont, fromSize]) el.addEventListener("input", paint);
  for (const el of [omit, sizeSel, toFont, fromFont]) el.addEventListener("change", paint);

  const body = h(
    "div",
    { class: "dialog-body mm-envelope-body" },
    h(
      "div",
      { class: "mm-env-cols" },
      h(
        "div",
        { class: "mm-env-form" },
        ...(mode === "document" ? [h("div", { class: "field" }, h("label", { for: `${id}-to` }, t("Delivery address")), delivery)] : []),
        h("div", { class: "field" }, h("label", { for: `${id}-from` }, t("Return address")), from, h("label", { class: "check", for: `${id}-omit` }, omit, h("span", {}, t("Leave out the return address")))),
        h("div", { class: "field" }, h("label", { for: `${id}-size` }, t("Envelope size")), sizeSel),
        customBox,
        h("div", { class: "mm-pair" }, h("div", { class: "field" }, h("label", { for: `${id}-tofont` }, t("Delivery address font")), toFont), h("div", { class: "field" }, h("label", { for: `${id}-tosize` }, t("Size in points")), toSize)),
        h("div", { class: "mm-pair" }, h("div", { class: "field" }, h("label", { for: `${id}-fromfont` }, t("Return address font")), fromFont), h("div", { class: "field" }, h("label", { for: `${id}-fromsize` }, t("Size in points")), fromSize)),
      ),
      h("div", { class: "mm-env-side" }, h("label", {}, t("Preview")), picture),
    ),
    error,
  );
  const dialog = h(
    "div",
    { class: "dialog mm-dialog mm-envelopes", role: "dialog", "aria-modal": "true", "aria-label": t("Envelopes") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Envelopes")), h("p", { class: "sub" }, mode === "merge" ? t("Set up the envelope. Each one takes the next recipient's address.") : t("The envelope becomes the first page of the document, with its own page size."))),
    body,
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  ok.addEventListener("click", async () => {
    const sz = sizeTw();
    if (mode === "document" && !delivery.value.trim() && !(await s.ui.confirm(t("There is no delivery address. Add the envelope anyway?"), { title: t("Add the envelope"), confirm: t("Add it") }))) return;
    const toLook: TextLook = { ...(toFont.value ? { font: toFont.value } : {}), size_pt: Number(toSize.value) || 12 };
    const fromLook: TextLook = { ...(fromFont.value ? { font: fromFont.value } : {}), size_pt: Number(fromSize.value) || 10 };
    const op: Op = {
      op: "insert_envelope",
      ...(sizeSel.value === "custom" ? { size: "custom", width: sz.w, height: sz.h } : { size: sizeSel.value }),
      layout: { delivery: delivery.value, return: omit.checked ? "" : from.value.replace(/\s+$/, ""), delivery_look: toLook, return_look: fromLook },
      replace: mode === "merge",
      address_block: mode === "merge",
    };
    ctx.state.set("returnAddress", from.value);
    ctx.state.set("omitReturn", omit.checked);
    ctx.state.set("envelopeSize", sizeSel.value);
    ctx.state.set("envelopeToFont", toFont.value);
    ctx.state.set("envelopeToSize", toLook.size_pt);
    ctx.state.set("envelopeFromFont", fromFont.value);
    ctx.state.set("envelopeFromSize", fromLook.size_pt);
    if (mode === "merge") {
      if (ed.engine && ed.engine.text().trim() && !(await ed.ask("mailMerge.makeEnvelope"))) return;
      close();
      s.run([op, { op: "set_mail_merge", merge: { main_type: "envelopes" } }]);
      return;
    }
    close();
    const done = s.run(op);
    if (done) toast(t("The envelope is the first page of the document."), 3500);
  });
  paint();
}
