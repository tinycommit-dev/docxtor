// Getting sources from outside: a DOI looked up at doi.org, and the checks that keep an import from adding what the
// document already has. The one place the Citations panel asks another server something is `fetchDoi`.
import { timeLimit, Engine, t, type Source } from "@docxtor/editor";

/** A DOI inside whatever was pasted: `10.1000/xyz123`, `doi:10.1000/xyz123` or a doi.org link. Null when there is none. */
export function findDoi(text: string): string | null {
  const m = /\b10\.\d{4,9}\/[^\s"<>]+/i.exec(text.trim());
  if (!m) return null;
  let doi = m[0];
  try {
    doi = decodeURIComponent(doi);
  } catch {
    // A stray % in the DOI itself: keep it as written.
  }
  // A full stop, comma or bracket after the DOI belongs to the sentence it was pasted from.
  return doi.replace(/[.,;:)\]}]+$/, "");
}

/** The address doi.org answers with the details of a DOI. */
export const doiUrl = (doi: string) => `https://doi.org/${doi.split("/").map(encodeURIComponent).join("/")}`;

/** Ask doi.org for a DOI's details and read them as a source. Throws a message that is fit to show. */
export async function fetchDoi(doi: string, signal?: AbortSignal): Promise<Source> {
  let res: Response;
  try {
    res = await fetch(doiUrl(doi), { signal: timeLimit(15000, signal), headers: { Accept: "application/vnd.citationstyles.csl+json" } });
  } catch (e) {
    if ((e as Error).name === "AbortError") throw e;
    throw new Error(t("Couldn't reach doi.org. Check your connection and try again."));
  }
  if (res.status === 404) throw new Error(t("doi.org has no record of {doi}. Check the DOI and try again.", { doi }));
  if (!res.ok) throw new Error(t("doi.org didn't answer (error {status}). Try again later.", { status: res.status }));
  let found: Source[];
  try {
    found = Engine.parseCslJson(await res.text());
  } catch (e) {
    throw new Error(String((e as Error).message ?? e));
  }
  const source = found[0];
  if (!source) throw new Error(t("doi.org had no details for {doi}.", { doi }));
  return { ...source, doi: source.doi || doi };
}

/** Read BibTeX, or CSL-JSON (what Zotero exports), into sources without tags. Throws the engine's plain message. */
export function readSources(text: string): Source[] {
  const body = text.trim();
  return /^[[{]/.test(body) ? Engine.parseCslJson(body) : Engine.parseBibtex(body);
}

const bare = (s: string) => s.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "");
const doiKey = (s: Source) => (s.doi ? (findDoi(s.doi) ?? s.doi).toLowerCase() : "");
const titleKey = (s: Source) => (s.title ? `${bare(s.title)}|${s.year ?? ""}` : "");

/** Whether `s` is one of `have`: the same DOI, or the same title in the same year. */
export function isDuplicate(have: Source[], s: Source): boolean {
  const doi = doiKey(s);
  const title = titleKey(s);
  return have.some((x) => (doi && doiKey(x) === doi) || (title && titleKey(x) === title));
}
