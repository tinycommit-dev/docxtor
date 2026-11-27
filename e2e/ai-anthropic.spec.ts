// The assistant's Anthropic provider, against a stand-in for Anthropic's server: what it sends (the key in X-Api-Key, as
// every gateway that worked with Docxtor before reads it; no Authorization header; the API version the SDK speaks) and
// that a streamed answer reaches the panel. No request leaves the machine.
import { expect, fakeAnthropic, inPage, openFixture, test, type Page } from "./helpers";

type Panel = { settings: Record<string, unknown>; input: HTMLTextAreaElement; send(): Promise<void>; toggle(open?: boolean): void };

async function ask(page: Page, settings: Record<string, unknown>, question: string) {
  await inPage(
    page,
    async (w, [s, q]) => {
      const ai = (w as unknown as { ai: Panel }).ai;
      ai.toggle(true);
      ai.settings = { ...ai.settings, provider: "anthropic", model: "claude-opus-5-5", ...s };
      ai.input.value = q;
      await ai.send();
    },
    [settings, question] as const,
  );
}

test.describe("the Anthropic provider", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page, "simple.docx");
  });

  test("it sends the key in X-Api-Key, not Authorization, and shows the streamed answer", async ({ page }) => {
    const seen = await fakeAnthropic(page, "https://api.anthropic.com", "Hello from the stand-in.");
    await ask(page, { apiKey: "test-key", baseUrl: "" }, "Say hello");
    await expect(page.locator(".entry.bot")).toContainText("Hello from the stand-in.");
    expect(seen).toHaveLength(1);
    const { headers, body } = seen[0];
    expect(headers["x-api-key"]).toBe("test-key");
    expect(headers.authorization).toBeUndefined();
    expect(headers["anthropic-version"]).toBe("2026-10-21");
    expect(body.model).toBe("claude-opus-5-5");
    expect(body.temperature).toBeUndefined();
  });

  test("a gateway set as the base URL gets the same header", async ({ page }) => {
    const seen = await fakeAnthropic(page, "https://gateway.example.test", "Through the gateway.");
    await ask(page, { apiKey: "gateway-key", baseUrl: "https://gateway.example.test" }, "Say hello");
    await expect(page.locator(".entry.bot")).toContainText("Through the gateway.");
    expect(seen).toHaveLength(1);
    expect(seen[0].headers["x-api-key"]).toBe("gateway-key");
    expect(seen[0].headers.authorization).toBeUndefined();
  });
});
