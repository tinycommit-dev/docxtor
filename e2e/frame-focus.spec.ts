import { expect } from "@playwright/test";
import { inPage, openFixture, test } from "./helpers";

// A click on the frame round the text (the desk round the pages, the ruler, the status bar, the document's name, a panel's empty
// space) leaves the keyboard in the text, as in Word: the browser would move it to the page body (`keepFocusOnFrame`).

test.describe("the keyboard stays in the text", () => {
  test("a click on any empty part of the frame keeps the keyboard and the caret where they were", async ({ page }) => {
    await openFixture(page, "rich.docx");
    // Every element of the frame with empty space under a point that hits the element itself (not a control or the text).
    const spots = await page.evaluate(() => {
      const out: { name: string; x: number; y: number }[] = [];
      const seen = new Set<string>();
      const all = Array.from(document.querySelectorAll<HTMLElement>(".docxtor *")).filter((el) => !el.closest(".flow, .story-edit, [role=dialog]"));
      for (const el of all) {
        const r = el.getBoundingClientRect();
        if (r.width < 20 || r.height < 8 || r.bottom < 0 || r.top > innerHeight) continue;
        const name = `${el.tagName.toLowerCase()}.${[...el.classList].join(".")}`;
        if (seen.has(name)) continue;
        // Text a reader may want to select is the browser's, on purpose (the third test): only in Firefox and Safari is there such text here, the
        // note that folders can't be opened. `user-select` of an element that says `auto` is its nearest ancestor's that does not.
        const effective = (() => {
          for (let e: Element | null = el; e; e = e.parentElement) {
            const v = getComputedStyle(e).userSelect;
            if (v && v !== "auto") return v;
          }
          return "auto";
        })();
        if (effective !== "none" && Array.from(el.childNodes).some((n) => n.nodeType === Node.TEXT_NODE && n.textContent!.trim())) continue;
        for (const [fx, fy] of [[0.5, 0.5], [0.98, 0.5], [0.02, 0.5], [0.5, 0.1]]) {
          // (Firefox and Safari click on whole pixels, so the point is found there: at a fraction it can be a neighbour's.)
          const x = Math.round(r.left + r.width * fx);
          const y = Math.round(r.top + r.height * fy);
          if (x < 0 || y < 0 || x > innerWidth || y > innerHeight) continue;
          if (document.elementFromPoint(x, y) !== el) continue;
          if (el.closest("button, input, select, textarea, a, label, summary, [tabindex], [role=button], [role=menuitem], [role=tab], [draggable=true]")) continue;
          out.push({ name, x, y });
          seen.add(name);
          break;
        }
      }
      return out;
    });
    // The canvas round the pages, the ruler and the status bar at least.
    for (const name of ["div.canvas", "div.ruler", "span.grow"]) expect(spots.map((s) => s.name)).toContain(name);
    const lost: string[] = [];
    for (const s of spots) {
      const before = await inPage(page, (w) => {
        (document.querySelector(".flow") as HTMLElement).focus();
        if (!w.fe.editor.sel) throw new Error("no caret in the text");
        return JSON.stringify(w.fe.editor.sel);
      });
      await page.mouse.click(s.x, s.y);
      await page.waitForTimeout(40);
      const after = await inPage(page, (w) => ({ flow: document.activeElement === document.querySelector(".flow"), sel: JSON.stringify(w.fe.editor.sel), open: !!document.querySelector('[role="dialog"], [role="alertdialog"], .menu-drop, .ctx-menu, .popover') }));
      if (after.open) await page.keyboard.press("Escape");
      if (!after.flow || after.sel !== before) lost.push(s.name);
    }
    expect(lost).toEqual([]);
  });

  test("text in the frame that is there to read can still be selected", async ({ page }) => {
    await openFixture(page, "rich.docx");
    const r = await page.evaluate(() => {
      const root = document.querySelector(".docxtor") as HTMLElement;
      const box = document.createElement("div");
      const para = document.createElement("p");
      para.textContent = "An answer a reader may want to copy.";
      box.append(para);
      root.append(box);
      const down = (el: Element) => {
        const e = new MouseEvent("mousedown", { bubbles: true, cancelable: true, button: 0 });
        el.dispatchEvent(e);
        return e.defaultPrevented;
      };
      const out = { text: down(para), empty: down(box) };
      box.remove();
      return out;
    });
    expect(r).toEqual({ text: false, empty: true });
  });

  test("leaving a footer by clicking the desk puts the keyboard back in the text, where the caret was", async ({ page }) => {
    await openFixture(page, "rich.docx");
    const before = await inPage(page, (w) => {
      w.fe.editor.focus();
      const sel = JSON.stringify(w.fe.editor.sel);
      if (!w.fe.editor.sel) throw new Error("no caret in the text");
      w.fe.editor.enterStory("footer", 0);
      return sel;
    });
    await expect(page.locator(".story-edit")).toBeFocused();
    const pt = await page.evaluate(() => {
      const c = (document.querySelector(".canvas") as HTMLElement).getBoundingClientRect();
      const f = (document.querySelector(".flow") as HTMLElement).getBoundingClientRect();
      return { x: Math.max(c.left + 4, f.left - 30), y: c.top + c.height / 2 };
    });
    expect(await page.evaluate((p) => document.elementFromPoint(p.x, p.y)?.className, pt)).toContain("canvas");
    await page.mouse.click(pt.x, pt.y);
    await expect(page.locator(".story-edit")).toHaveCount(0);
    await expect(page.locator(".flow")).toBeFocused();
    expect(await inPage(page, (w) => JSON.stringify(w.fe.editor.sel))).toBe(before);
  });

  // Leaving a header, footer or note without asking for the caret (opening another document, going
  // read-only, updating fields) took away the box the keyboard was in and left it on the page body.
  for (const [how, leave] of [
    ["opening another document", "open"],
    ["View, Viewing", "view.viewing"],
    ["Update fields", "edit.updateFields"],
  ] as const) {
    test(`leaving a footer by ${how} puts the keyboard back in the text`, async ({ page }) => {
      await openFixture(page, "rich.docx");
      await inPage(page, (w) => {
        (window as any).__bytes = w.fe.editor.engine!.save();
        w.fe.editor.focus();
        w.fe.editor.enterStory("footer", 0);
      });
      await expect(page.locator(".story-edit")).toBeFocused();
      await page.evaluate(async (id) => {
        const fe = (window as any).docxtor.fe;
        if (id === "open") await fe.open((window as any).__bytes, "Again.docx");
        else await fe.extensions.execute(id);
      }, leave);
      await expect(page.locator(".story-edit")).toHaveCount(0);
      await expect(page.locator(".flow")).toBeFocused();
    });
  }

  test("leaving a footer while the keyboard is somewhere else leaves the keyboard there", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await inPage(page, (w) => {
      (window as any).__bytes = w.fe.editor.engine!.save();
      w.fe.editor.enterStory("footer", 0);
      const box = document.createElement("input");
      box.id = "elsewhere";
      document.body.append(box);
      box.focus();
    });
    await page.evaluate(async () => (window as any).docxtor.fe.open((window as any).__bytes, "Again.docx"));
    await expect(page.locator(".story-edit")).toHaveCount(0);
    await expect(page.locator("#elsewhere")).toBeFocused();
  });
});
