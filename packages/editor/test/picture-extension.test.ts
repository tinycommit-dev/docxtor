// The file type a picture's bytes call for. These need no browser.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { pictureExtension } from "../src/editor/picture.ts";

const bytes = (...v: number[]) => Uint8Array.from([...v, ...new Array(60).fill(0)]);

test("a WMF is recognised with or without the placeable header", () => {
  // Placeable: the magic number Aldus gave the 22-byte header.
  assert.equal(pictureExtension(bytes(0xd7, 0xcd, 0xc6, 0x9a)), "wmf");
  // Standard, as Word writes a part of its own: memory metafile (1) or disk file (2), header size 9, version 0x0300 or 0x0100.
  assert.equal(pictureExtension(bytes(1, 0, 9, 0, 0, 3)), "wmf");
  assert.equal(pictureExtension(bytes(2, 0, 9, 0, 0, 1)), "wmf");
  // The same shape with another header size or version is not one.
  assert.equal(pictureExtension(bytes(1, 0, 8, 0, 0, 3)), null);
  assert.equal(pictureExtension(bytes(1, 0, 9, 0, 0, 2)), null);
});

test("an EMF is not taken for a WMF, and unknown bytes are refused", () => {
  const emf = new Uint8Array(64);
  emf[0] = 1;
  emf.set([0x20, 0x45, 0x4d, 0x46], 40);
  assert.equal(pictureExtension(emf), "emf");
  assert.equal(pictureExtension(bytes(0, 0, 0, 0)), null);
});
