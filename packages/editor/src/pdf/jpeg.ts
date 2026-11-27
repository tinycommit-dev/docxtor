// Reading just enough of a JPEG file to put it in a PDF as it is (the DCTDecode filter reads JPEG data
// directly): its size, how many colour components it has, and whether its colours are inverted or its
// picture is turned by an EXIF orientation. No DOM.

export interface JpegInfo {
  width: number;
  height: number;
  /** 1 (grey), 3 (RGB) or 4 (CMYK). */
  components: number;
  bits: number;
  /** An Adobe marker says the four components are stored inverted (CMYK files from Photoshop). */
  inverted: boolean;
  /** EXIF orientation, 1 (as stored) to 8. */
  orientation: number;
}

export const isJpeg = (b: Uint8Array): boolean => b.length > 4 && b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff;

/** The facts about a JPEG, or null when the data is not one a PDF viewer can be given as it is. */
export function readJpeg(b: Uint8Array): JpegInfo | null {
  if (!isJpeg(b)) return null;
  let i = 2;
  let adobe = false;
  let orientation = 1;
  while (i + 4 <= b.length) {
    if (b[i] !== 0xff) {
      i++;
      continue;
    }
    const marker = b[i + 1];
    if (marker === 0xff) {
      i++;
      continue;
    }
    // Markers without a length.
    if (marker === 0xd8 || marker === 0x01 || (marker >= 0xd0 && marker <= 0xd7)) {
      i += 2;
      continue;
    }
    const len = (b[i + 2] << 8) | b[i + 3];
    const body = i + 4;
    if (marker === 0xee && len >= 12 && String.fromCharCode(b[body], b[body + 1], b[body + 2], b[body + 3], b[body + 4]) === "Adobe") adobe = true;
    if (marker === 0xe1 && len >= 16 && String.fromCharCode(b[body], b[body + 1], b[body + 2], b[body + 3]) === "Exif") orientation = exifOrientation(b, body + 6, i + 2 + len);
    // A frame header: SOF0 to SOF15 except the table markers DHT, JPG and DAC.
    if (marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc) {
      const components = b[body + 5];
      if (components !== 1 && components !== 3 && components !== 4) return null;
      // Arithmetic coding (SOF9 and up) is not read by every viewer.
      if (marker >= 0xc9) return null;
      return { bits: b[body], height: (b[body + 1] << 8) | b[body + 2], width: (b[body + 3] << 8) | b[body + 4], components, inverted: adobe && components === 4, orientation };
    }
    i += 2 + len;
  }
  return null;
}

/** The orientation tag from the TIFF data of an EXIF block. */
function exifOrientation(b: Uint8Array, tiff: number, end: number): number {
  if (tiff + 8 > end) return 1;
  const little = b[tiff] === 0x49;
  const u16 = (o: number) => (little ? b[o] | (b[o + 1] << 8) : (b[o] << 8) | b[o + 1]);
  const u32 = (o: number) => (little ? (b[o] | (b[o + 1] << 8) | (b[o + 2] << 16) | (b[o + 3] << 24)) >>> 0 : ((b[o] << 24) | (b[o + 1] << 16) | (b[o + 2] << 8) | b[o + 3]) >>> 0);
  const ifd = tiff + u32(tiff + 4);
  if (ifd + 2 > end) return 1;
  const n = u16(ifd);
  for (let k = 0; k < n; k++) {
    const e = ifd + 2 + k * 12;
    if (e + 12 > end) break;
    if (u16(e) === 0x0112) return u16(e + 8) || 1;
  }
  return 1;
}
