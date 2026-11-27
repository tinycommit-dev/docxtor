// Language detectors (Detect language automatically): the latest sure answer wins, an unsure or failing one hands on.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { defineExtension, ExtensionHost, type DocumentApi, type LanguageDetector, type UiApi } from "../src/index.ts";

const doc = {} as DocumentApi;
const ui = { notify() {} } as unknown as UiApi;
const newHost = (errors: unknown[] = []) => new ExtensionHost({ document: doc, ui, mac: true, onError: (_id, err) => errors.push(err) });
const detector = (id: string, detect: LanguageDetector["detect"]): LanguageDetector => ({ id, label: id, detect });

test("the latest detector that is sure answers; turned off, the one before does", async () => {
  const host = newHost();
  await host.add(defineExtension({ id: "test.words", name: "Words", activate: (ctx) => void ctx.spelling.languageDetectors.register(detector("words", () => ({ lang: "es", confidence: 0.8 }))) }));
  await host.add(defineExtension({ id: "test.model", name: "Model", activate: (ctx) => void ctx.spelling.languageDetectors.register(detector("model", async () => ({ lang: "pt-BR", confidence: 0.95 }))) }));
  assert.deepEqual(await host.detectLanguage("Olá, tudo bem com você hoje?"), { lang: "pt-BR", confidence: 0.95 });
  await host.disable("test.model");
  assert.deepEqual(await host.detectLanguage("Hola, ¿cómo estás hoy?"), { lang: "es", confidence: 0.8 });
});

test("an unsure answer, no answer, or a detector that fails hands the text on; none sure is null", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  await host.add(defineExtension({ id: "test.base", name: "Base", activate: (ctx) => void ctx.spelling.languageDetectors.register(detector("base", () => ({ lang: "fr", confidence: 0.7 }))) }));
  await host.add(defineExtension({ id: "test.unsure", name: "Unsure", activate: (ctx) => void ctx.spelling.languageDetectors.register(detector("unsure", () => ({ lang: "de", confidence: 0.3 }))) }));
  await host.add(defineExtension({ id: "test.silent", name: "Silent", activate: (ctx) => void ctx.spelling.languageDetectors.register(detector("silent", () => null)) }));
  await host.add(
    defineExtension({
      id: "test.broken",
      name: "Broken",
      activate: (ctx) =>
        void ctx.spelling.languageDetectors.register(
          detector("broken", () => {
            throw new Error("no model");
          }),
        ),
    }),
  );
  assert.deepEqual(await host.detectLanguage("Bonjour à tous et bienvenue"), { lang: "fr", confidence: 0.7 });
  assert.equal(errors.length, 1);
  // Asking for more certainty than anyone has.
  assert.equal(await host.detectLanguage("Bonjour à tous", 0.9), null);
});

test("a detector needs an id and a detect function", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  await host.add(defineExtension({ id: "test.bad", name: "Bad", activate: (ctx) => void ctx.spelling.languageDetectors.register({ id: "x", label: "x" } as unknown as LanguageDetector) }));
  assert.equal(host.languageDetectors.all().length, 0);
});
