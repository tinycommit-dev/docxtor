// Small TIFF, EMF and WMF files for the browser tests, written byte by byte. Each is 100 x 50 units (a TIFF,
// 4 x 2 pixels): red on the left, green on the right.

class Bytes {
  b: number[] = [];
  u8(...v: number[]) {
    this.b.push(...v);
    return this;
  }
  u16(v: number) {
    return this.u8(v & 255, (v >> 8) & 255);
  }
  u32(v: number) {
    return this.u8(v & 255, (v >>> 8) & 255, (v >>> 16) & 255, (v >>> 24) & 255);
  }
  i32(v: number) {
    return this.u32(v >>> 0);
  }
  at(i: number, v: number) {
    this.b[i] = v & 255;
    this.b[i + 1] = (v >> 8) & 255;
    this.b[i + 2] = (v >> 16) & 255;
    this.b[i + 3] = (v >>> 24) & 255;
  }
  get bytes() {
    return new Uint8Array(this.b);
  }
}

/** COLORREF: 0x00BBGGRR. */
const ref = (r: number, g: number, b: number) => r | (g << 8) | (b << 16);

/** An uncompressed RGB TIFF of 4 x 2 pixels: the left half red, the right half green. */
export function tiff(): Uint8Array {
  const t = new Bytes().u8(0x49, 0x49, 42, 0).u32(8);
  const entries: [number, number, number, number][] = [
    [256, 3, 1, 4],
    [257, 3, 1, 2],
    [258, 3, 3, 122],
    [259, 3, 1, 1],
    [262, 3, 1, 2],
    [273, 4, 1, 128],
    [277, 3, 1, 3],
    [278, 3, 1, 2],
    [279, 4, 1, 24],
  ];
  t.u16(entries.length);
  for (const [tag, type, count, value] of entries) t.u16(tag).u16(type).u32(count).u32(value);
  t.u32(0).u16(8).u16(8).u16(8);
  for (let y = 0; y < 2; y++) for (let x = 0; x < 4; x++) t.u8(...(x < 2 ? [220, 30, 30] : [30, 170, 60]));
  return t.bytes;
}

/** An EMF: a red rectangle over the left half and a green one over the right, no outline. */
export function emf(): Uint8Array {
  const e = new Bytes();
  e.u32(1).u32(88);
  for (const n of [0, 0, 99, 49, 0, 0, 2646, 1323]) e.i32(n);
  e.u32(0x464d4520).u32(0x10000).u32(0).u32(0).u16(0).u16(0).u32(0).u32(0).u32(0);
  for (const n of [1024, 768, 320, 240]) e.i32(n);
  const brush = (handle: number, color: number) => e.u32(39).u32(24).u32(handle).u32(0).u32(color).u32(0);
  const select = (handle: number) => e.u32(37).u32(12).u32(handle);
  const rect = (l: number, t: number, r: number, b: number) => {
    e.u32(43).u32(24);
    for (const n of [l, t, r, b]) e.i32(n);
  };
  select(0x80000008); // the stock null pen
  brush(1, ref(220, 30, 30));
  select(1);
  rect(0, 0, 50, 50);
  brush(2, ref(30, 170, 60));
  select(2);
  rect(50, 0, 100, 50);
  e.u32(14).u32(20).u32(0).u32(16).u32(20);
  e.at(48, e.b.length);
  return e.bytes;
}

/** A WMF with the placeable header: the same two rectangles, written with the arguments in WMF's order (bottom, right, top, left). */
export function wmf(): Uint8Array {
  const w = new Bytes();
  w.u32(0x9ac6cdd7).u16(0).u16(0).u16(0).u16(100).u16(50).u16(96).u32(0).u16(0);
  w.u16(1).u16(9).u16(0x300);
  const sizeAt = w.b.length;
  w.u32(0).u16(3).u32(0).u16(0);
  const record = (fn: number, ...params: number[]) => {
    w.u32(3 + params.length).u16(fn);
    for (const p of params) w.u16(p);
  };
  record(0x020b, 0, 0); // window origin (y, x)
  record(0x020c, 50, 100); // window extent (y, x)
  record(0x02fa, 5, 0, 0, 0, 0); // a null pen
  record(0x012d, 0);
  record(0x02fc, 0, 220 | (30 << 8), 30, 0); // a red brush
  record(0x012d, 1);
  record(0x041b, 50, 50, 0, 0); // bottom, right, top, left
  record(0x02fc, 0, 30 | (170 << 8), 60, 0); // a green brush
  record(0x012d, 2);
  record(0x041b, 50, 100, 0, 50);
  record(0);
  w.at(sizeAt, (w.b.length - 22) / 2);
  return w.bytes;
}
