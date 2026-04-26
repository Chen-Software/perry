// JSON pipeline: read 108MB array, filter active records, add 2 derived fields,
// serialize back, write output. Stays in stdlib (std.json) throughout.

const std = @import("std");

const Addr = struct {
    street: []const u8,
    city: []const u8,
    zip: u32,
};

const InRecord = struct {
    id: u64,
    name: []const u8,
    email: []const u8,
    age: u32,
    country: []const u8,
    tags: [][]const u8,
    score: u32,
    active: bool,
    addr: Addr,
};

const OutRecord = struct {
    id: u64,
    name: []const u8,
    email: []const u8,
    age: u32,
    country: []const u8,
    tags: [][]const u8,
    score: u32,
    active: bool,
    addr: Addr,
    display_name: []const u8,
    age_group: []const u8,
};

fn ageGroup(age: u32) []const u8 {
    if (age < 30) return "young";
    if (age < 50) return "mid";
    return "senior";
}

fn toUpperAlloc(alloc: std.mem.Allocator, s: []const u8) ![]u8 {
    const out = try alloc.alloc(u8, s.len);
    for (s, 0..) |c, i| {
        out[i] = if (c >= 'a' and c <= 'z') c - 32 else c;
    }
    return out;
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

    const args = try std.process.argsAlloc(alloc);
    defer std.process.argsFree(alloc, args);
    if (args.len < 3) {
        std.debug.print("usage: {s} <input.json> <output.json>\n", .{args[0]});
        return 1;
    }

    // Read
    const in_path = args[1];
    const out_path = args[2];
    const file = try std.fs.cwd().openFile(in_path, .{});
    const stat = try file.stat();
    const input = try alloc.alloc(u8, stat.size);
    defer alloc.free(input);
    _ = try file.readAll(input);
    file.close();
    const input_bytes = input.len;

    // Parse
    var parsed = try std.json.parseFromSlice([]InRecord, alloc, input, .{
        .allocate = .alloc_always,
        .ignore_unknown_fields = true,
    });
    defer parsed.deinit();
    const records = parsed.value;
    const records_in = records.len;

    // Filter + map
    var out_list = std.ArrayList(OutRecord){};
    defer out_list.deinit(alloc);
    for (records) |r| {
        if (!r.active) continue;
        const disp = try toUpperAlloc(alloc, r.name);
        try out_list.append(alloc, OutRecord{
            .id = r.id,
            .name = r.name,
            .email = r.email,
            .age = r.age,
            .country = r.country,
            .tags = r.tags,
            .score = r.score,
            .active = r.active,
            .addr = r.addr,
            .display_name = disp,
            .age_group = ageGroup(r.age),
        });
    }
    const records_out = out_list.items.len;

    // Serialize — valueAlloc returns an allocated byte slice with the JSON.
    const serialized = try std.json.Stringify.valueAlloc(alloc, out_list.items, .{});
    defer alloc.free(serialized);
    const output_bytes = serialized.len;

    // Write output
    const ofile = try std.fs.cwd().createFile(out_path, .{});
    try ofile.writeAll(serialized);
    ofile.close();

    const hash = fnv1a32(serialized);

    var stdout_buf: [256]u8 = undefined;
    const line = try std.fmt.bufPrint(
        &stdout_buf,
        "input_bytes={d} records_in={d} records_out={d} output_bytes={d} hash={x:0>8}\n",
        .{ input_bytes, records_in, records_out, output_bytes, hash },
    );
    _ = try std.fs.File.stdout().writeAll(line);
    return 0;
}
