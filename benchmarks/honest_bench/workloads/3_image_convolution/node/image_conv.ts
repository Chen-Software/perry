// 5x5 Gaussian blur on a 3840x2160 RGB image.
// In-memory input + output checksum. Same algorithm as Rust/Zig/Perry.

const W = 3840;
const H = 2160;
const N = W * H * 3;
const SEED = 0x9E3779B9;

const KERNEL = [
  [1, 4, 7, 4, 1],
  [4, 16, 26, 16, 4],
  [7, 26, 41, 26, 7],
  [4, 16, 26, 16, 4],
  [1, 4, 7, 4, 1],
];
const KSUM = 273;

function generateInput(): Uint8Array {
  const pixels = new Uint8Array(N);
  for (let y = 0; y < H; y++) {
    const rowBase = ((y * 255) / H) | 0;
    const off0 = y * W * 3;
    for (let x = 0; x < W; x++) {
      const off = off0 + x * 3;
      pixels[off] = ((x * 255) / W) | 0;
      pixels[off + 1] = rowBase;
      pixels[off + 2] = (((x + y) * 255) / (W + H)) | 0;
    }
  }
  let s = SEED >>> 0;
  let i = 0;
  while (i + 4 <= N) {
    s = (s ^ ((s << 13) >>> 0)) >>> 0;
    s = (s ^ (s >>> 17)) >>> 0;
    s = (s ^ ((s << 5) >>> 0)) >>> 0;
    pixels[i] = (pixels[i] + (s & 0xff)) & 0xff;
    pixels[i + 1] = (pixels[i + 1] + ((s >>> 8) & 0xff)) & 0xff;
    pixels[i + 2] = (pixels[i + 2] + ((s >>> 16) & 0xff)) & 0xff;
    pixels[i + 3] = (pixels[i + 3] + ((s >>> 24) & 0xff)) & 0xff;
    i += 4;
  }
  while (i < N) {
    s = (s ^ ((s << 13) >>> 0)) >>> 0;
    s = (s ^ (s >>> 17)) >>> 0;
    s = (s ^ ((s << 5) >>> 0)) >>> 0;
    pixels[i] = (pixels[i] + (s & 0xff)) & 0xff;
    i += 1;
  }
  return pixels;
}

function blur(src: Uint8Array, w: number, h: number): Uint8Array {
  const dst = new Uint8Array(src.length);
  const wi = w - 1;
  const hi = h - 1;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      let rAcc = 0, gAcc = 0, bAcc = 0;
      for (let ky = -2; ky <= 2; ky++) {
        const yy = Math.max(0, Math.min(y + ky, hi));
        const row = yy * w;
        const krow = KERNEL[ky + 2];
        for (let kx = -2; kx <= 2; kx++) {
          const xx = Math.max(0, Math.min(x + kx, wi));
          const idx = (row + xx) * 3;
          const k = krow[kx + 2];
          rAcc += src[idx] * k;
          gAcc += src[idx + 1] * k;
          bAcc += src[idx + 2] * k;
        }
      }
      const outIdx = (y * w + x) * 3;
      dst[outIdx] = Math.max(0, Math.min(255, (rAcc / KSUM) | 0));
      dst[outIdx + 1] = Math.max(0, Math.min(255, (gAcc / KSUM) | 0));
      dst[outIdx + 2] = Math.max(0, Math.min(255, (bAcc / KSUM) | 0));
    }
  }
  return dst;
}

function fnv1a32(bytes: Uint8Array): number {
  let h = 0x811c9dc5 | 0;
  for (let i = 0; i < bytes.length; i++) {
    h = (h ^ bytes[i]) | 0;
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

const src = generateInput();
const dst = blur(src, W, H);
const hash = fnv1a32(dst);
const hex = hash.toString(16).padStart(8, '0');
console.log(`checksum=${hex} dims=${W}x${H} bytes=${dst.length}`);
