// 5x5 Gaussian blur on a 3840x2160 RGB image.
// In-memory input + output checksum; see workload README.md for rationale.

const std = @import("std");

const W: usize = 3840;
const H: usize = 2160;
const N: usize = W * H * 3;
const SEED: u32 = 0x9E3779B9;

const KERNEL: [5][5]i32 = .{
    .{ 1, 4, 7, 4, 1 },
    .{ 4, 16, 26, 16, 4 },
    .{ 7, 26, 41, 26, 7 },
    .{ 4, 16, 26, 16, 4 },
    .{ 1, 4, 7, 4, 1 },
};
const KSUM: i32 = 273;

fn generate_input(alloc: std.mem.Allocator) ![]u8 {
    const pixels = try alloc.alloc(u8, N);
    var y: usize = 0;
    while (y < H) : (y += 1) {
        const row_base: u8 = @intCast((y * 255) / H);
        const off0 = y * W * 3;
        var x: usize = 0;
        while (x < W) : (x += 1) {
            const off = off0 + x * 3;
            pixels[off] = @intCast((x * 255) / W);
            pixels[off + 1] = row_base;
            pixels[off + 2] = @intCast(((x + y) * 255) / (W + H));
        }
    }
    var s: u32 = SEED;
    var i: usize = 0;
    while (i + 4 <= N) {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        pixels[i] = pixels[i] +% @as(u8, @truncate(s));
        pixels[i + 1] = pixels[i + 1] +% @as(u8, @truncate(s >> 8));
        pixels[i + 2] = pixels[i + 2] +% @as(u8, @truncate(s >> 16));
        pixels[i + 3] = pixels[i + 3] +% @as(u8, @truncate(s >> 24));
        i += 4;
    }
    while (i < N) {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        pixels[i] = pixels[i] +% @as(u8, @truncate(s));
        i += 1;
    }
    return pixels;
}

inline fn clamp_idx(v: isize, lo: isize, hi: isize) usize {
    if (v < lo) return @intCast(lo);
    if (v > hi) return @intCast(hi);
    return @intCast(v);
}

inline fn clamp_u8(v: i32) u8 {
    if (v < 0) return 0;
    if (v > 255) return 255;
    return @intCast(v);
}

fn blur(alloc: std.mem.Allocator, src: []const u8, w: usize, h: usize) ![]u8 {
    const dst = try alloc.alloc(u8, src.len);
    const wi: isize = @intCast(w);
    const hi: isize = @intCast(h);
    var y: usize = 0;
    while (y < h) : (y += 1) {
        var x: usize = 0;
        while (x < w) : (x += 1) {
            var r_acc: i32 = 0;
            var g_acc: i32 = 0;
            var b_acc: i32 = 0;
            var ky: isize = -2;
            while (ky <= 2) : (ky += 1) {
                const yy = clamp_idx(@as(isize, @intCast(y)) + ky, 0, hi - 1);
                const row = yy * w;
                var kx: isize = -2;
                while (kx <= 2) : (kx += 1) {
                    const xx = clamp_idx(@as(isize, @intCast(x)) + kx, 0, wi - 1);
                    const idx = (row + xx) * 3;
                    const k: i32 = KERNEL[@as(usize, @intCast(ky + 2))][@as(usize, @intCast(kx + 2))];
                    r_acc += @as(i32, src[idx]) * k;
                    g_acc += @as(i32, src[idx + 1]) * k;
                    b_acc += @as(i32, src[idx + 2]) * k;
                }
            }
            const out_idx = (y * w + x) * 3;
            dst[out_idx] = clamp_u8(@divTrunc(r_acc, KSUM));
            dst[out_idx + 1] = clamp_u8(@divTrunc(g_acc, KSUM));
            dst[out_idx + 2] = clamp_u8(@divTrunc(b_acc, KSUM));
        }
    }
    return dst;
}

fn fnv1a32(bytes: []const u8) u32 {
    var h: u32 = 0x811c9dc5;
    for (bytes) |b| {
        h ^= @as(u32, b);
        h *%= 0x01000193;
    }
    return h;
}

pub fn main() !u8 {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const alloc = gpa.allocator();

    const src = try generate_input(alloc);
    defer alloc.free(src);
    const dst = try blur(alloc, src, W, H);
    defer alloc.free(dst);
    const hash = fnv1a32(dst);

    var stdout_buf: [128]u8 = undefined;
    const out = try std.fmt.bufPrint(&stdout_buf, "checksum={x:0>8} dims={d}x{d} bytes={d}\n", .{ hash, W, H, dst.len });
    _ = try std.fs.File.stdout().writeAll(out);
    return 0;
}
