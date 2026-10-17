import assert from "node:assert/strict";
import test from "node:test";
import { verifyRuntimeSymbols } from "./desktop-performance-runtime-symbols.mjs";

const LIBC = "/lib/x86_64-linux-gnu/libc.so.6";
const LOADER = "/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2";
const id = "0123456789abcdef0123456789abcdef01234567";
const debugPath = `/usr/lib/debug/.build-id/${id.slice(0, 2)}/${id.slice(2)}.debug`;
const metadata = (ino = 5) => ({
	dev: 2049,
	ino,
	size: 8,
	mode: 0o100644,
	uid: 0,
	mtimeMs: 1,
	ctimeMs: 1,
	isFile: () => true,
	isSymbolicLink: () => false,
});
const line = (start, filename, inode = 5) =>
	`${start}-${(Number.parseInt(start, 16) + 1).toString(16)} r-xp 00000000 08:01 ${inode} ${filename}\n`;
function fixture() {
	const f = {
		maps: line("1000", LIBC) + line("2000", LOADER, 6),
		files: new Map([
			[LIBC, metadata()],
			[LOADER, metadata(6)],
			[debugPath, metadata(7)],
		]),
		aliases: new Map(),
		commands: [],
		verifications: 0,
	};
	f.io = {
		privateDirectory: async () => {},
		verifyIdentity: async () => {
			f.verifications++;
		},
		realpath: async (p) => f.aliases.get(p) ?? p,
		lstat: async (p) => {
			assert.ok(f.files.has(p), `missing ${p}`);
			return f.files.get(p);
		},
		open: async (p, flags) => {
			assert.ok(flags > 0);
			const bytes =
				p === "/proc/12/maps"
					? Buffer.from(f.maps)
					: Buffer.from([127, 69, 76, 70, 0, 0, 0, 0]);
			if (p !== "/proc/12/maps" && !f.files.has(p))
				throw Object.assign(new Error("missing debug"), { code: "ENOENT" });
			let offset = 0;
			return {
				stat: async () => f.files.get(p) ?? metadata(),
				read: async (buffer, start, length) => {
					const count = Math.min(length, bytes.length - offset);
					bytes.copy(buffer, start, offset, offset + count);
					offset += count;
					return { bytesRead: count };
				},
				close: async () => {},
			};
		},
		boundedCommand: async (root, name, command, args, cap) => {
			assert.equal(root, "/private/capture");
			assert.equal(command, "/usr/bin/readelf");
			assert.equal(cap, 65536);
			assert.deepEqual(args.slice(0, -1), [
				"--notes",
				"--sections",
				"--wide",
				"--",
			]);
			f.commands.push(name);
			return `Build ID: ${id}\n  [ 7] .symtab SYMTAB 00000000\n`;
		},
	};
	f.run = () =>
		verifyRuntimeSymbols(
			{ root: "/private/capture", binding: { pid: 12 } },
			f.io,
		);
	return f;
}

test("valid mapped runtime/debug identities produce private receipt and coalesce library segments", async () => {
	const f = fixture();
	f.maps += line("3000", LIBC);
	const result = await f.run();
	assert.equal(result.library_count, 2);
	assert.equal(result.libraries[0].build_id, id);
	assert.match(result.libraries[0].runtime_sha256, /^[0-9a-f]{64}$/u);
	assert.equal(f.commands.length, 4);
	assert.equal(f.verifications, 3);
});

test("runtime symlink aliases resolve to checked canonical immutable ELF", async () => {
	const f = fixture();
	const canonical = "/usr/lib/x86_64-linux-gnu/libc.so.6";
	f.aliases.set(LIBC, canonical);
	f.files.set(canonical, f.files.get(LIBC));
	const result = await f.run();
	assert.equal(result.libraries[0].runtime_path, canonical);
});

test("debug build ID mismatch and missing symtab fail closed", async () => {
	for (const output of [
		`Build ID: ${"ab".repeat(20)}\n [ 2] .symtab SYMTAB\n`,
		`Build ID: ${id}\n`,
	]) {
		const f = fixture(),
			command = f.io.boundedCommand;
		f.io.boundedCommand = (...args) =>
			args[1].endsWith("debug") ? output : command(...args);
		await assert.rejects(f.run(), /debug build ID or symtab/);
	}
});

test("missing debug, debug redirect and unsafe ownership fail closed", async () => {
	const missing = fixture();
	missing.files.delete(debugPath);
	await assert.rejects(missing.run(), /missing debug/);
	const redirected = fixture();
	redirected.aliases.set(debugPath, "/elsewhere/debug");
	await assert.rejects(redirected.run(), /debug path redirect/);
	for (const filename of [LIBC, debugPath]) {
		for (const patch of [
			{ uid: 1000 },
			{ mode: 0o100666 },
			{ isFile: () => false },
			{ isSymbolicLink: () => true },
		]) {
			const f = fixture();
			Object.assign(f.files.get(filename), patch);
			await assert.rejects(f.run(), /ownership/);
		}
	}
});

test("malformed, deleted, traversal, duplicate and inconsistent maps fail closed", async () => {
	for (const maps of [
		"bad\n",
		line("1000", `${LIBC} (deleted)`),
		line("1000", "/lib/../lib/libc.so.6"),
		line("1000", LIBC) + line("1000", LIBC),
		line("1000", LIBC) + line("2000", LIBC, 99),
	]) {
		const f = fixture();
		f.maps = maps;
		await assert.rejects(f.run(), /map|path/);
	}
});

test("missing loader, inode mismatch and readelf ambiguity/caps fail closed", async () => {
	const absent = fixture();
	absent.maps = line("1000", LIBC);
	await assert.rejects(absent.run(), /loader/);
	const inode = fixture();
	inode.files.get(LIBC).ino = 88;
	await assert.rejects(inode.run(), /mapped runtime identity/);
	for (const output of [
		`Build ID: ${id}\nBuild ID: ${id}\n`,
		"x".repeat(65537),
		"Build ID: ../../escape\n",
	]) {
		const f = fixture();
		f.io.boundedCommand = async () => output;
		await assert.rejects(f.run(), /build ID|output cap/);
	}
});

test("identity changes before/after maps or after inspection abort", async () => {
	for (const boundary of [1, 2, 3]) {
		const f = fixture();
		f.io.verifyIdentity = async () => {
			if (++f.verifications === boundary)
				throw new Error("owned identity changed");
		};
		await assert.rejects(f.run(), /owned identity changed/);
	}
});

test("bounded maps read and changed mapped aliases fail closed", async () => {
	const oversized = fixture();
	oversized.maps = "x".repeat(1024 * 1024 + 1);
	await assert.rejects(oversized.run(), /read cap/);
	const changed = fixture();
	let calls = 0;
	changed.io.realpath = async (p) =>
		p === LIBC && ++calls >= 3 ? "/changed" : p;
	await assert.rejects(
		changed.run(),
		/ELF identity changed|mapped path changed/,
	);
});
