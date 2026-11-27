// Which way a language reads, and the right-to-left pseudo-language for checking the interface.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { directionOf, pickLanguage, PSEUDO_RTL_LOCALE, setLanguage, t, textDirection } from "../src/l10n.ts";

test("Arabic, Hebrew, Persian and Urdu are written right to left; a pack can say so itself", () => {
  assert.deepEqual(["ar", "he-IL", "fa", "ur", "en", "es", "zh-Hans"].map(directionOf), ["rtl", "rtl", "rtl", "rtl", "ltr", "ltr", "ltr"]);
  setLanguage({ locale: "ar", label: "العربية", messages: {} });
  assert.equal(textDirection(), "rtl");
  setLanguage({ locale: "xx", label: "Own", messages: {}, dir: "rtl" });
  assert.equal(textDirection(), "rtl");
  setLanguage(pickLanguage(PSEUDO_RTL_LOCALE, []));
  assert.equal(textDirection(), "rtl");
  assert.equal(t("Save"), "‮Save‬");
  setLanguage(null);
  assert.equal(textDirection(), "ltr");
});
