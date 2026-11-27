// Insert, Image, From a URL: the browser fetches the picture at an address the person types. Many sites do not let
// other pages read their pictures (CORS), so the dialog says plainly why a fetch failed and what to do instead.
import type { PictureResult } from "@docxtor/extensions";
import { t } from "@docxtor/extensions";
import { h, modal } from "./dom";

/** The biggest picture the dialog takes, in bytes. */
export const MAX_URL_PICTURE = 25 * 1024 * 1024;

const EXTENSIONS: Record<string, string> = {
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/jpg": "jpg",
  "image/gif": "gif",
  "image/webp": "webp",
  "image/svg+xml": "svg",
  "image/bmp": "bmp",
  "image/tiff": "tiff",
  "image/x-emf": "emf",
  "image/emf": "emf",
  "image/x-wmf": "wmf",
  "image/wmf": "wmf",
};

/** The file name an address suggests (the last part of its path), with an extension from the media type if it has none. */
export function nameFromUrl(url: URL, type: string): string {
  let name = "";
  try {
    name = decodeURIComponent(url.pathname.split("/").filter(Boolean).pop() ?? "");
  } catch {
    name = url.pathname.split("/").filter(Boolean).pop() ?? "";
  }
  if (!name || url.protocol === "data:") name = "picture";
  const ext = EXTENSIONS[type.split(";")[0].trim().toLowerCase()];
  if (ext && !/\.[a-z0-9]{2,5}$/i.test(name)) name += `.${ext}`;
  return name;
}

/**
 * Fetch a picture from an address. Rejects with a plain message when the address is not a web address, the fetch is
 * blocked (usually CORS), the site refuses, or what comes back is not a picture.
 */
export async function fetchPicture(address: string, signal?: AbortSignal): Promise<PictureResult> {
  let url: URL;
  try {
    url = new URL(address.trim());
  } catch {
    throw new Error(t("That isn't a web address. It should start with https://"));
  }
  if (!["http:", "https:", "data:"].includes(url.protocol)) throw new Error(t("That isn't a web address. It should start with https://"));
  let res: Response;
  try {
    res = await fetch(url, { signal, credentials: "omit", referrerPolicy: "no-referrer" });
  } catch (e) {
    if ((e as Error).name === "AbortError") throw e;
    throw new Error(t("The picture could not be fetched. The site may not let other pages read its pictures, or the address may be wrong. Save the picture to your computer and use From your computer instead."));
  }
  if (!res.ok) throw new Error(t("The site answered {status} instead of sending the picture.", { status: String(res.status) }));
  const type = (res.headers.get("content-type") ?? "").split(";")[0].trim().toLowerCase();
  const length = Number(res.headers.get("content-length") ?? 0);
  if (length > MAX_URL_PICTURE) throw new Error(t("That picture is larger than 25 MB."));
  const buf = await res.arrayBuffer();
  if (buf.byteLength > MAX_URL_PICTURE) throw new Error(t("That picture is larger than 25 MB."));
  if (type && !type.startsWith("image/") && type !== "application/octet-stream") throw new Error(t("That address is not a picture (it sent {type}).", { type }));
  const bytes = new Uint8Array(buf);
  if (!type.startsWith("image/") && !looksLikePicture(bytes)) throw new Error(t("That address is not a picture."));
  return { bytes, name: nameFromUrl(url, type), type: type || undefined };
}

/** Whether the first bytes are those of a picture format Docxtor knows. */
function looksLikePicture(b: Uint8Array): boolean {
  const is = (...sig: number[]) => sig.every((v, i) => b[i] === v);
  return is(0x89, 0x50, 0x4e, 0x47) || is(0xff, 0xd8, 0xff) || is(0x47, 0x49, 0x46) || is(0x42, 0x4d) || is(0x49, 0x49, 0x2a, 0x00) || is(0x4d, 0x4d, 0x00, 0x2a) || (is(0x52, 0x49, 0x46, 0x46) && b[8] === 0x57) || is(0x3c, 0x73, 0x76, 0x67) || is(0x3c, 0x3f, 0x78, 0x6d);
}

/** Ask for an address and fetch the picture there; null when the person closes the dialog. */
export function askPictureUrl(): Promise<PictureResult | null> {
  return new Promise((resolve) => {
    let done = false;
    const controller = new AbortController();
    const input = h("input", { type: "url", placeholder: "https://example.com/picture.png", "aria-label": t("Picture address"), spellcheck: "false", autocomplete: "off" });
    const status = h("p", { class: "help pu-status", role: "status" });
    const ok = h("button", { class: "btn primary", type: "button" }, t("Insert"));
    const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
    const finish = (v: PictureResult | null) => {
      if (done) return;
      done = true;
      controller.abort();
      close();
      resolve(v);
    };
    const go = async () => {
      if (!input.value.trim()) return;
      ok.disabled = true;
      input.disabled = true;
      status.classList.remove("error");
      status.textContent = t("Fetching the picture…");
      try {
        finish(await fetchPicture(input.value, controller.signal));
      } catch (e) {
        if (done) return;
        status.classList.add("error");
        status.textContent = (e as Error).message;
        ok.disabled = false;
        input.disabled = false;
        input.focus();
      }
    };
    const dialog = h(
      "div",
      { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Picture from a URL"), style: "width:min(var(--dialog-m),100%)" },
      h("div", { class: "dialog-head" }, h("h2", {}, t("Picture from a URL"))),
      h(
        "div",
        { class: "dialog-body" },
        h("div", { class: "field" }, h("label", {}, t("Picture address")), input, h("span", { class: "help" }, t("Your browser fetches this address directly, so the site it belongs to sees the request. The picture is then saved in the document."))),
        status,
      ),
      h("div", { class: "dialog-actions" }, cancel, ok),
    );
    const close = modal(dialog, () => finish(null));
    ok.addEventListener("click", () => void go());
    cancel.addEventListener("click", () => finish(null));
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") void go();
    });
    setTimeout(() => input.focus());
  });
}
