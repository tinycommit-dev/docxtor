// The documents the harness measures, built the same way every time (no downloads, no randomness that changes).
//
//   series(n)          n paragraphs of ordinary prose with headings and a list now and then (the slope series: 100, 1,000, 5,000, 20,000)
//   novel(words)       a novel: paragraphs of about 65 words with short lines of dialogue between them, some italics and bold,
//                      a chapter heading about every 4,000 words, a page break before each chapter, and a few footnotes
//   NOVELS             "War and Peace" (580,000 words) and "Proust" (1,300,000 words)
//
// Each returns the .docx bytes (made by the engine, so the file is one Docxtor saved) and what was put in.
import { deflateSync } from "node:zlib";
import { Engine } from "../../packages/core/src/index.ts";

export interface Built {
  name: string;
  bytes: Uint8Array;
  paragraphs: number;
  words: number;
  chapters: number;
  footnotes: number;
  chars: number;
}

/** A small deterministic random number generator (mulberry32). */
export function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** 800 pronounceable made-up words, always the same ones. */
function vocabulary(): string[] {
  const r = rng(7);
  const on = ["b", "br", "c", "d", "f", "g", "gl", "h", "j", "k", "l", "m", "n", "p", "pr", "r", "s", "st", "t", "tr", "v", "w", "z", "th", "sh", "ch"];
  const nu = ["a", "e", "i", "o", "u", "ai", "ea", "ou", "io"];
  const co = ["", "n", "r", "s", "t", "l", "m", "nd", "st", "rt"];
  const pick = <T,>(a: T[]) => a[Math.floor(r() * a.length)];
  const out = new Set<string>();
  while (out.size < 800) out.add(pick(on) + pick(nu) + pick(co) + (r() < 0.45 ? pick(on) + pick(nu) + pick(co) : ""));
  return [...out];
}

/** Word counts ("the" and its like) occur more often, as in prose: the first 40 words are picked about half the time. */
function sentenceWords(r: () => number, v: string[], n: number): string[] {
  const w: string[] = [];
  for (let i = 0; i < n; i++) w.push(r() < 0.5 ? v[Math.floor(r() * 40)] : v[Math.floor(r() * v.length)]);
  return w;
}

function paragraphText(r: () => number, v: string[], dialogue: boolean): { text: string; words: number } {
  const n = dialogue ? 5 + Math.floor(r() * 16) : 45 + Math.floor(r() * 76);
  const w = sentenceWords(r, v, n);
  // Sentences of 6 to 20 words.
  let out = "";
  let i = 0;
  while (i < n) {
    const len = Math.min(n - i, 6 + Math.floor(r() * 15));
    const s = w.slice(i, i + len);
    s[0] = s[0][0].toUpperCase() + s[0].slice(1);
    out += (out ? " " : "") + s.join(" ") + (r() < 0.12 ? "?" : ".");
    i += len;
  }
  // Italics now and then (one phrase in four paragraphs), bold rarely (one in fifteen).
  const words = out.split(" ");
  if (!dialogue && r() < 0.25) {
    const at = 3 + Math.floor(r() * (words.length - 8));
    const k = 1 + Math.floor(r() * 3);
    words[at] = "*" + words[at];
    words[at + k - 1] = words[at + k - 1].replace(/[.?]$/, "") + "*";
  }
  if (!dialogue && r() < 0.07) {
    const at = 2 + Math.floor(r() * (words.length - 6));
    words[at] = "**" + words[at] + "**";
  }
  let text = words.join(" ");
  if (dialogue) text = "“" + text.replace(/\.$/, ",") + "” said " + v[Math.floor(r() * 40)] + ".";
  return { text, words: n };
}

function build(name: string, md: string[], heads: number[], noteAt: number[], paragraphs: number, words: number): Built {
  const e = Engine.blank(name);
  const first = e.renderAll().order[0];
  e.apply([
    { op: "insert_markdown", before: first, markdown: md.join("\n\n") },
    { op: "delete_blocks", ids: [first] },
  ] as never);
  const order = e.renderAll().order;
  if (heads.length) e.apply({ op: "format_paragraphs", ids: heads.map((i) => order[i]), format: { pageBreakBefore: true } } as never);
  if (noteAt.length) e.apply(noteAt.map((i) => ({ op: "insert_footnote", at: { p: order[i], o: 12 }, text: `Note ${i}: see the earlier chapter.` })) as never);
  const bytes = e.save();
  e.dispose();
  return { name, bytes, paragraphs, words, chapters: heads.length, footnotes: noteAt.length, chars: md.join("").length };
}

/** `n` paragraphs, as packages/core/test/bench-typing.mts has always built them. */
export function series(n: number): Built {
  const md = Array.from({ length: n }, (_, i) => (i % 50 === 0 ? `## Heading ${i}` : i % 7 === 0 ? `- item **${i}** of the list` : `Paragraph ${i} with *some* words to lay out and a little more text.`));
  return build(`series-${n}`, md, [], [], n, n * 12);
}

/** A novel of about `words` words. */
export function novel(name: string, words: number, seed = 1): Built {
  const r = rng(seed);
  const v = vocabulary();
  const md: string[] = [];
  const heads: number[] = [];
  const noteAt: number[] = [];
  let total = 0;
  let sinceHead = 1e9;
  let chapter = 0;
  let paras = 0;
  while (total < words) {
    if (sinceHead >= 4000) {
      chapter++;
      heads.push(md.length);
      md.push(`# Chapter ${chapter}`);
      sinceHead = 0;
      continue;
    }
    // One line of dialogue in four, or two in a row now and then.
    const p = paragraphText(r, v, r() < 0.25);
    if (paras % 400 === 399) noteAt.push(md.length);
    md.push(p.text);
    paras++;
    total += p.words;
    sinceHead += p.words;
  }
  return build(name, md, heads, noteAt, paras, total);
}

/** A picture of `w` by `h` pixels as a PNG: smooth bands with noise on top, so it does not compress to nothing (about 200 KB at 800 by 500, as a photograph in a thesis would). */
export function png(w: number, h: number, seed: number): Uint8Array {
  const r = rng(seed);
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) {
    const row = y * (w * 3 + 1);
    raw[row] = 0;
    for (let x = 0; x < w; x++) {
      const o = row + 1 + x * 3;
      raw[o] = (x * 255) / w + ((r() * 40) | 0);
      raw[o + 1] = (y * 255) / h + ((r() * 40) | 0);
      raw[o + 2] = ((x + y) * 127) / (w + h) + ((r() * 60) | 0);
    }
  }
  const crcTable = Array.from({ length: 256 }, (_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = (b: Buffer) => {
    let c = 0xffffffff;
    for (const x of b) c = crcTable[(c ^ x) & 255] ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (type: string, data: Buffer) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type), data]);
    const sum = Buffer.alloc(4);
    sum.writeUInt32BE(crc(body));
    return Buffer.concat([len, body, sum]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8;
  ihdr[9] = 2;
  return new Uint8Array(Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]));
}

/** Words a page of the thesis holds, with its headings, tables and pictures (measured: `perf/lib/docs.mts` is rebuilt, and the page count is read in the browser). */
export const THESIS_WORDS_PER_PAGE = 415;

/**
 * A document as a student or a researcher writes one, about `pages` pages long: chapters (Heading 1, a page break before each),
 * sections and subsections (Heading 2 and 3), body paragraphs of 80 to 160 words with italics, bold, links and footnotes, bulleted
 * lists, a block quote now and then, a table of four columns every two sections, and a picture with its caption every three.
 */
export function thesis(name: string, pages: number, seed = 3, leave: { pictures?: boolean; tables?: boolean } = {}): Built {
  const r = rng(seed);
  const v = vocabulary();
  const words = pages * THESIS_WORDS_PER_PAGE;
  const md: string[] = [];
  const heads: string[] = [];
  const figures: string[] = [];
  const notes: string[] = [];
  let total = 0;
  let paras = 0;
  let chapter = 0;
  let section = 0;
  let figure = 0;
  const title = () => sentenceWords(r, v, 2 + Math.floor(r() * 3)).map((w) => w[0].toUpperCase() + w.slice(1)).join(" ");
  const para = () => {
    const p = paragraphText(r, v, false);
    total += p.words;
    paras++;
    return p.text;
  };
  const pictures = [png(800, 500, 11), png(640, 480, 12), png(900, 400, 13), png(700, 700, 14)];
  while (total < words) {
    chapter++;
    section = 0;
    const h1 = `Chapter ${chapter}. ${title()}`;
    heads.push(h1);
    md.push(`# ${h1}`);
    let chapterWords = 0;
    while (chapterWords < 7000 && total < words) {
      section++;
      md.push(`## ${chapter}.${section} ${title()}`);
      const n = 4 + Math.floor(r() * 4);
      for (let i = 0; i < n; i++) {
        const w0 = total;
        if (i === 2 && r() < 0.5) md.push(`### ${chapter}.${section}.1 ${title()}`);
        md.push(para());
        if (paras % 8 === 7) notes.push(md[md.length - 1].slice(0, 40));
        if (i === 1 && r() < 0.4) {
          md.push(`- ${sentenceWords(r, v, 9).join(" ")}\n- ${sentenceWords(r, v, 7).join(" ")}\n- ${sentenceWords(r, v, 11).join(" ")}`);
          total += 27;
        }
        if (i === 3 && r() < 0.25) {
          md.push(`> ${para()}`);
        }
        chapterWords += total - w0;
      }
      if (section % 2 === 0 && !leave.tables) {
        const cell = () => sentenceWords(r, v, 1 + Math.floor(r() * 2)).join(" ");
        const rows = 4 + Math.floor(r() * 4);
        md.push([`| ${cell()} | ${cell()} | ${cell()} | ${cell()} |`, `|---|---|---|---|`, ...Array.from({ length: rows }, () => `| ${cell()} | ${(r() * 1000).toFixed(1)} | ${(r() * 100).toFixed(2)} | ${cell()} |`)].join("\n"));
        total += rows * 8;
        chapterWords += rows * 8;
      }
      if (section % 3 === 0 && !leave.pictures) {
        figure++;
        const caption = `Figure ${figure}. ${sentenceWords(r, v, 8).join(" ")}.`;
        figures.push(caption);
        md.push(caption);
        total += 130;
        chapterWords += 130;
      }
    }
  }
  const e = Engine.blank(name);
  const first = e.renderAll().order[0];
  e.apply([
    { op: "insert_markdown", before: first, markdown: md.join("\n\n") },
    { op: "delete_blocks", ids: [first] },
  ] as never);
  const all = e.renderAll();
  const idOf = new Map<string, number>();
  for (const b of all.blocks as unknown as { id: number }[]) {
    const s = JSON.stringify(b);
    const m = /"(Chapter \d+\.[^"]*|Figure \d+\.[^"]*)"/.exec(s);
    if (m) idOf.set(m[1].replace(/\.$/, ""), b.id);
  }
  const headIds = heads.map((h) => idOf.get(h)).filter((x): x is number => x !== undefined);
  if (headIds.length) e.apply({ op: "format_paragraphs", ids: headIds, format: { pageBreakBefore: true } } as never);
  const media = pictures.map((b) => e.addMedia("png", b));
  const dims = [[360, 225], [320, 240], [380, 169], [280, 280]];
  const figIds = figures.map((f) => idOf.get(f.replace(/\.$/, ""))).map((id, i) => ({ id, i }));
  const ops: unknown[] = [];
  for (const { id, i } of figIds) if (id !== undefined) ops.push({ op: "insert_image", at: { p: id, o: 0 }, media: media[i % media.length], width: dims[i % 4][0], height: dims[i % 4][1], alt: `Figure ${i + 1}` });
  if (ops.length) e.apply(ops as never);
  // A footnote in every eighth body paragraph (plain paragraphs of at least 60 characters, no heading).
  const plain = (all.blocks as unknown as { t: string; id: number; style?: string; len: number }[]).filter((b) => b.t === "p" && !b.style && b.len > 60);
  const noteOps = plain.filter((_, i) => i % 8 === 7).map((b, i) => ({ op: "insert_footnote", at: { p: b.id, o: 20 }, text: `Note ${i + 1}: see the earlier chapter and the sources listed there.` }));
  if (noteOps.length) e.apply(noteOps as never);
  const bytes = e.save();
  e.dispose();
  return { name, bytes, paragraphs: paras, words: total, chapters: heads.length, footnotes: noteOps.length, chars: md.join("").length };
}

export const NOVELS = {
  "war-and-peace": { title: "War and Peace", words: 580_000 },
  proust: { title: "Proust", words: 1_300_000 },
} as const;

/** One of the named documents: `series-1000`, `war-and-peace`, `proust`. */
export function named(name: string): Built {
  const m = /^series-(\d+)$/.exec(name);
  if (m) return series(Number(m[1]));
  const t = /^thesis-(\d+)(-nopic|-notab|-plain)?$/.exec(name);
  if (t) return thesis(name, Number(t[1]), 3, { pictures: t[2] === "-nopic" || t[2] === "-plain", tables: t[2] === "-notab" || t[2] === "-plain" });
  const n = NOVELS[name as keyof typeof NOVELS];
  if (!n) throw new Error(`no document named ${name}`);
  return novel(name, n.words, name === "proust" ? 2 : 1);
}

/** The document as a file in `dir`, made once (the cache is keyed by the generator's own text, so a change to it rebuilds). */
export async function cached(name: string, dir: string): Promise<{ file: string; built: Omit<Built, "bytes"> }> {
  const { createHash } = await import("node:crypto");
  const fs = await import("node:fs");
  const path = await import("node:path");
  fs.mkdirSync(dir, { recursive: true });
  const key = createHash("sha1").update(fs.readFileSync(new URL(import.meta.url))).digest("hex").slice(0, 8);
  const file = path.join(dir, `${name}-${key}.docx`);
  const meta = file + ".json";
  if (!fs.existsSync(file) || !fs.existsSync(meta)) {
    const b = named(name);
    fs.writeFileSync(file, b.bytes);
    const { bytes: _b, ...rest } = b;
    fs.writeFileSync(meta, JSON.stringify(rest));
  }
  return { file, built: JSON.parse(fs.readFileSync(meta, "utf8")) };
}
