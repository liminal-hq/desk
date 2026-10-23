// A small PNG writer for the emoji proof sheets: indexed colour, compressed by a deflate written here (LZ77 with
// the fixed Huffman codes), so the same pixels always give the same bytes whatever zlib the machine has.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

export function crc32(data: Uint8Array): number {
  let c = 0xffffffff;
  for (const b of data) c = (CRC_TABLE[(c ^ b) & 0xff] as number) ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function adler32(data: Uint8Array): number {
  let a = 1;
  let b = 0;
  for (const v of data) {
    a = (a + v) % 65521;
    b = (b + a) % 65521;
  }
  return ((b << 16) | a) >>> 0;
}

/** Writes bits least significant first, as deflate does. */
class Bits {
  out: number[] = [];
  private acc = 0;
  private n = 0;
  put(value: number, count: number): void {
    for (let i = 0; i < count; i++) {
      this.acc |= ((value >>> i) & 1) << this.n;
      if (++this.n === 8) {
        this.out.push(this.acc);
        this.acc = 0;
        this.n = 0;
      }
    }
  }
  /** A Huffman code, which deflate stores most significant bit first. */
  code(value: number, count: number): void {
    for (let i = count - 1; i >= 0; i--) this.put((value >>> i) & 1, 1);
  }
  flush(): number[] {
    if (this.n) this.out.push(this.acc);
    this.acc = 0;
    this.n = 0;
    return this.out;
  }
}

const LEN_BASE = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DIST_EXTRA = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

function literal(bits: Bits, v: number): void {
  if (v < 144) bits.code(0x30 + v, 8);
  else if (v < 256) bits.code(0x190 + v - 144, 9);
  else if (v < 280) bits.code(v - 256, 7);
  else bits.code(0xc0 + v - 280, 8);
}

/** Raw deflate: one final block with the fixed codes, greedy LZ77 over a 32 KB window. */
export function deflate(data: Uint8Array): Uint8Array {
  const bits = new Bits();
  bits.put(1, 1); // final block
  bits.put(1, 2); // fixed Huffman codes
  const head = new Map<number, number[]>();
  const hash = (i: number): number => ((data[i] as number) << 16) | ((data[i + 1] as number) << 8) | (data[i + 2] as number);
  const remember = (i: number): void => {
    if (i + 2 >= data.length) return;
    const h = hash(i);
    const list = head.get(h);
    if (list) {
      list.push(i);
      if (list.length > 64) list.shift();
    } else head.set(h, [i]);
  };
  let i = 0;
  while (i < data.length) {
    let bestLen = 0;
    let bestDist = 0;
    if (i + 2 < data.length) {
      const list = head.get(hash(i)) ?? [];
      for (let k = list.length - 1; k >= 0; k--) {
        const j = list[k] as number;
        if (i - j > 32768) break;
        let len = 0;
        while (len < 258 && i + len < data.length && data[j + len] === data[i + len]) len++;
        if (len > bestLen) {
          bestLen = len;
          bestDist = i - j;
          if (len === 258) break;
        }
      }
    }
    if (bestLen >= 3) {
      let c = LEN_BASE.length - 1;
      while ((LEN_BASE[c] as number) > bestLen) c--;
      literal(bits, 257 + c);
      bits.put(bestLen - (LEN_BASE[c] as number), LEN_EXTRA[c] as number);
      let d = DIST_BASE.length - 1;
      while ((DIST_BASE[d] as number) > bestDist) d--;
      bits.code(d, 5);
      bits.put(bestDist - (DIST_BASE[d] as number), DIST_EXTRA[d] as number);
      for (let k = 0; k < bestLen; k++) remember(i + k);
      i += bestLen;
    } else {
      literal(bits, data[i] as number);
      remember(i);
      i++;
    }
  }
  literal(bits, 256);
  return Uint8Array.from(bits.flush());
}

/** A zlib stream: header, raw deflate and the Adler-32 of the data. */
export function zlib(data: Uint8Array): Uint8Array {
  const body = deflate(data);
  const a = adler32(data);
  return Uint8Array.from([0x78, 0x01, ...body, a >>> 24, (a >>> 16) & 0xff, (a >>> 8) & 0xff, a & 0xff]);
}

function chunk(type: string, data: Uint8Array): number[] {
  const td = Uint8Array.from([...Array.from(type, (c) => c.charCodeAt(0)), ...data]);
  const crc = crc32(td);
  const n = data.length;
  return [n >>> 24, (n >>> 16) & 0xff, (n >>> 8) & 0xff, n & 0xff, ...td, crc >>> 24, (crc >>> 16) & 0xff, (crc >>> 8) & 0xff, crc & 0xff];
}

/**
 * An indexed-colour PNG, 8 bits per pixel. `palette` is up to 256 colours as `[r, g, b, a]`; `pixels`
 * is `width × height` palette indices, row by row.
 */
export function encodePng(width: number, height: number, palette: [number, number, number, number][], pixels: Uint8Array): Uint8Array {
  if (pixels.length !== width * height) throw new Error('pixel count does not match the size');
  const raw = new Uint8Array((width + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (width + 1)] = 0; // no filter
    raw.set(pixels.subarray(y * width, (y + 1) * width), y * (width + 1) + 1);
  }
  const ihdr = new Uint8Array(13);
  const dv = new DataView(ihdr.buffer);
  dv.setUint32(0, width);
  dv.setUint32(4, height);
  ihdr.set([8, 3, 0, 0, 0], 8);
  const plte = Uint8Array.from(palette.flatMap(([r, g, b]) => [r, g, b]));
  const trns = Uint8Array.from(palette.map((c) => c[3]));
  return Uint8Array.from([
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
    ...chunk('IHDR', ihdr),
    ...chunk('PLTE', plte),
    ...chunk('tRNS', trns),
    ...chunk('IDAT', zlib(raw)),
    ...chunk('IEND', new Uint8Array(0)),
  ]);
}
