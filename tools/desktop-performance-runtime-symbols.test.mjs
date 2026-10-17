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

// Ubuntu libc6_2.35-0ubuntu3.15 ships this compatibility stub with its own symtab/CFI,
// and libc6-dbg has no separate .build-id debug member for this exact build ID.
const PTHREAD = "/lib/x86_64-linux-gnu/libpthread.so.0";
const PTHREAD_ID = "57dbd4443fd643e796cbf95c88dbc95ddd8167a5";
const STUB_SYMTAB = ".symtab SYMTAB 0000000000000000 002000 000120 18 9 3 8";
const STUB_EH_FRAME =
	".eh_frame PROGBITS 0000000000002038 002038 000038 00 A 0 0 8";
function pthreadFixture(sections = `${STUB_SYMTAB}\n [ 8] ${STUB_EH_FRAME}`) {
	const f = fixture();
	f.maps += line("3000", PTHREAD, 8);
	f.files.set(PTHREAD, metadata(8));
	const original = f.io.boundedCommand;
	f.io.boundedCommand = async (...args) => {
		if (args[3].at(-1) !== PTHREAD) return original(...args);
		f.commands.push(args[1]);
		return `Build ID: ${PTHREAD_ID}\n [ 7] ${sections}\n`;
	};
	return f;
}

test("mapped Ubuntu pthread with its own symtab and eh_frame needs no absent separate debug file", async () => {
	const f = pthreadFixture();
	const result = await f.run();
	assert.equal(result.library_count, 3);
	const stub = result.libraries.find((entry) => entry.runtime_path === PTHREAD);
	assert.equal(stub.build_id, PTHREAD_ID);
	assert.equal(stub.symbol_source, "runtime_elf");
	assert.equal(stub.symbol_path, PTHREAD);
	assert.equal(stub.symbol_sha256, stub.runtime_sha256);
	assert.equal(stub.debug_path, null);
	assert.equal(stub.debug_sha256, null);
	assert(!f.commands.includes("runtime-2-debug"));
	assert.equal(result.libraries[0].symbol_source, "separate_debug");
	assert.equal(result.libraries[0].symbol_path, debugPath);
});

test("runtime fallback requires both symtab and real eh_frame, otherwise exact separate debug remains required", async () => {
	for (const sections of [
		STUB_EH_FRAME,
		STUB_SYMTAB,
		`${STUB_SYMTAB}\n [ 8] ${STUB_EH_FRAME.replace(".eh_frame ", ".eh_frame_hdr ")}`,
		`${STUB_SYMTAB}\n [ 8] ${STUB_EH_FRAME.replace("PROGBITS", "NOBITS")}`,
		`${STUB_SYMTAB}\n [ 8] ${STUB_EH_FRAME.replace("000038", "000000")}`,
		`${STUB_SYMTAB}\n [ 8] ${STUB_EH_FRAME.replace("000038", "10000000")}`,
		`${STUB_SYMTAB}\n [ 8] .eh_frame PROGBITS`,
	]) {
		const f = pthreadFixture(sections);
		await assert.rejects(f.run(), /missing debug/);
	}
});

test("in-object symbols do not bypass mapped identity or immutable ownership checks", async () => {
	for (const patch of [{ uid: 1000 }, { mode: 0o100666 }, { ino: 99 }]) {
		const f = pthreadFixture();
		Object.assign(f.files.get(PTHREAD), patch);
		await assert.rejects(f.run(), /ownership|mapped runtime identity/);
	}
});

function precisionFixture({
	changedNamedNanoseconds = false,
	exactInode = 9007199254740993n,
} = {}) {
	const f = fixture();
	f.maps = line("1000", LIBC, exactInode) + line("2000", LOADER, 6);
	f.files.get(LIBC).ino = exactInode;
	const convert = (value, options, named = false) => {
		if (!options?.bigint) return { ...value, ino: Number(value.ino) };
		const result = { ...value };
		for (const key of [
			"dev",
			"ino",
			"size",
			"mode",
			"uid",
			"mtimeMs",
			"ctimeMs",
		])
			result[key] = BigInt(value[key]);
		result.mtimeNs = 1000001n + (named && changedNamedNanoseconds ? 1n : 0n);
		result.ctimeNs = 1000001n;
		return result;
	};
	const originalOpen = f.io.open;
	f.io.open = async (...args) => {
		const file = await originalOpen(...args);
		const originalStat = file.stat;
		file.stat = async (options) => convert(await originalStat(), options);
		return file;
	};
	f.io.lstat = async (name, options) =>
		convert(f.files.get(name), options, true);
	return f;
}

test("exact bigint stat and named identity preserve odd mapped inodes above Number precision", async () => {
	const f = precisionFixture();
	const receipt = await f.run();
	assert.equal(receipt.library_count, 2);
});

test("named nanosecond identity change is rejected even within the same millisecond", async () => {
	const f = precisionFixture({ changedNamedNanoseconds: true, exactInode: 5n });
	await assert.rejects(f.run(), /ELF identity changed/);
});

test("bigint metadata retains root ownership, write-mode and size guards", async () => {
	for (const patch of [
		{ uid: 1000 },
		{ mode: 0o100666 },
		{ size: 256 * 1024 * 1024 + 1 },
	]) {
		const f = precisionFixture();
		Object.assign(f.files.get(LIBC), patch);
		await assert.rejects(f.run(), /ownership or size/);
	}
});
