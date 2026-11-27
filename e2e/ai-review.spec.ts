// The AI panel's review card: "Revert all" must take back everything the run changed, even when the undo history is held to a
// memory budget that would otherwise let the run's first steps go.
import { expect, inPage, openFixture, test, type Page } from "./helpers";

type Panel = {
  settings: { apiKey: string };
  agent: { run: () => Promise<void> };
  input: HTMLTextAreaElement;
  send(): Promise<void>;
  toggle(open?: boolean): void;
};
type Raw = { setUndoBudget(n: number): void; undoLabels(): string };

/** A run of `steps` separate edits to the first paragraph, standing in for a model's tool calls; the history holds only the last few steps (a budget of nothing). */
async function runWithEdits(page: Page, steps: number) {
  await inPage(
    page,
    async (w, n) => {
      const app = w as unknown as { ai: Panel; fe: { editor: { engine: { raw: Raw; renderAll(): { order: number[] } } | null; exec(op: object): void } } };
      const ed = app.fe.editor;
      ed.engine!.raw.setUndoBudget(1);
      app.ai.toggle(true);
      app.ai.settings = { ...app.ai.settings, apiKey: "test-key" };
      const p = ed.engine!.renderAll().order[0];
      app.ai.agent.run = async () => {
        for (let i = 0; i < n; i++) ed.exec({ op: "insert_text", at: { p, o: 0 }, text: "ai " });
      };
      app.ai.input.value = "Add words";
      await app.ai.send();
    },
    steps,
  );
}

const firstText = (page: Page) => inPage(page, (w) => (w as unknown as { fe: { editor: { engine: { text(): string } } } }).fe.editor.engine.text().split("\n")[0]);
const steps = (page: Page) => inPage(page, (w) => JSON.parse((w as unknown as { fe: { editor: { engine: { raw: Raw } } } }).fe.editor.engine.raw.undoLabels()).length as number);

test.describe("the AI review card", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page, "simple.docx");
  });

  test("it keeps every step of a long run while the card waits, and Revert all takes them all back", async ({ page }) => {
    const before = await firstText(page);
    await runWithEdits(page, 30);
    await expect(page.locator(".review")).toBeVisible();
    expect(await firstText(page)).toContain("ai ai ai");
    expect(await steps(page)).toBeGreaterThanOrEqual(30);
    await page.locator(".review").getByRole("button", { name: "Revert all" }).click();
    await expect(page.locator(".review")).toContainText("Reverted.");
    expect(await firstText(page)).toBe(before);
  });

  test("Keep lets the history shrink back to its budget", async ({ page }) => {
    await runWithEdits(page, 30);
    await expect(page.locator(".review")).toBeVisible();
    await page.locator(".review").getByRole("button", { name: "Keep" }).click();
    await expect(page.locator(".review")).toContainText("Kept.");
    expect(await steps(page)).toBe(10);
  });
});
