// 5x5 Gaussian blur on a 3840x2160 RGB image.
//
// Input is generated in-memory (deterministic xorshift32) so the three
// language implementations can compare convolution-loop performance
// independent of their binary-file I/O stories. Output is a 64-bit FNV-1a
// checksum of the blurred pixel bytes printed to stdout.

use std::env;
use std::process::ExitCode;

const W: usize = 3840;
const H: usize = 2160;
const N: usize = W * H * 3;
const SEED: u32 = 0x9E3779B9;

// sigma=1.0 Gaussian, sum = 273
const KERNEL: [[i32; 5]; 5] = [
    [1,  4,  7,  4, 1],
    [4, 16, 26, 16, 4],
    [7, 26, 41, 26, 7],
    [4, 16, 26, 16, 4],
    [1,  4,  7,  4, 1],
];
const KSUM: i32 = 273;

fn generate_input() -> Vec<u8> {
    // Matches scripts/gen_image.py: gradient + xorshift32 noise, summed mod 256.
    let mut pixels = vec![0u8; N];
    for y in 0..H {
        let row_base = ((y * 255) / H) as u8;
        let off0 = y * W * 3;
        for x in 0..W {
            let off = off0 + x * 3;
            pixels[off]     = ((x * 255) / W) as u8;
            pixels[off + 1] = row_base;
            pixels[off + 2] = (((x + y) * 255) / (W + H)) as u8;
        }
    }
    let mut s: u32 = SEED;
    let mut i = 0;
    while i + 4 <= N {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        pixels[i]     = pixels[i].wrapping_add(s as u8);
        pixels[i + 1] = pixels[i + 1].wrapping_add((s >> 8) as u8);
        pixels[i + 2] = pixels[i + 2].wrapping_add((s >> 16) as u8);
        pixels[i + 3] = pixels[i + 3].wrapping_add((s >> 24) as u8);
        i += 4;
    }
    while i < N {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        pixels[i] = pixels[i].wrapping_add(s as u8);
        i += 1;
    }
    pixels
}

#[inline(always)]
fn clamp(v: isize, lo: isize, hi: isize) -> usize {
    if v < lo { lo as usize } else if v > hi { hi as usize } else { v as usize }
}

fn blur(src: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut dst = vec![0u8; src.len()];
    let wi = w as isize;
    let hi = h as isize;
    for y in 0..h {
        for x in 0..w {
            let mut r_acc: i32 = 0;
            let mut g_acc: i32 = 0;
            let mut b_acc: i32 = 0;
            for ky in -2..=2isize {
                let yy = clamp(y as isize + ky, 0, hi - 1);
                let row = yy * w;
                for kx in -2..=2isize {
                    let xx = clamp(x as isize + kx, 0, wi - 1);
                    let idx = (row + xx) * 3;
                    let k = KERNEL[(ky + 2) as usize][(kx + 2) as usize];
                    r_acc += src[idx]     as i32 * k;
                    g_acc += src[idx + 1] as i32 * k;
                    b_acc += src[idx + 2] as i32 * k;
                }
            }
            let out_idx = (y * w + x) * 3;
            dst[out_idx]     = (r_acc / KSUM).clamp(0, 255) as u8;
            dst[out_idx + 1] = (g_acc / KSUM).clamp(0, 255) as u8;
            dst[out_idx + 2] = (b_acc / KSUM).clamp(0, 255) as u8;
        }
    }
    dst
}

fn fnv1a32(bytes: &[u8]) -> u32 {
    // 32-bit variant. Perry's 64-bit BigInt arithmetic has a live bug
    // (bitwise ops wrap signed → hash collapses to 0), so we keep hashes in
    // the 32-bit integer space where all three languages agree bit-exactly.
    let mut h: u32 = 0x811c9dc5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}

fn main() -> ExitCode {
    // Accept dims override for quick sanity checks: `image_conv <w> <h>`
    let args: Vec<String> = env::args().collect();
    let (w, h) = if args.len() >= 3 {
        (args[1].parse::<usize>().unwrap_or(W), args[2].parse::<usize>().unwrap_or(H))
    } else {
        (W, H)
    };
    // Fast-path: precomputed constants assume default dims. For arbitrary dims,
    // regenerate and adjust.
    let src = if w == W && h == H {
        generate_input()
    } else {
        let mut px = vec![0u8; w * h * 3];
        let mut s: u32 = SEED;
        let mut i = 0;
        while i < px.len() {
            s ^= s << 13; s ^= s >> 17; s ^= s << 5;
            px[i] = s as u8;
            i += 1;
        }
        px
    };
    let out = blur(&src, w, h);
    let hash = fnv1a32(&out);
    println!("checksum={:08x} dims={}x{} bytes={}", hash, w, h, out.len());
    ExitCode::from(0)
}
