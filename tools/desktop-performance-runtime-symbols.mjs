// Private provisioning evidence only; matching symbols do not prove stack completeness.
import { createHash } from "node:crypto";
import { constants } from "node:fs";
import { lstat, open, realpath } from "node:fs/promises";
import path from "node:path";
import {
	boundedCommand,
	privateDirectory,
	verifyIdentity,
} from "./desktop-performance-headless-stacks-recorder.mjs";

const MAP_CAP = 1024 * 1024;
const ELF_CAP = 256 * 1024 * 1024;
const OUTPUT_CAP = 65536;
const RUNTIME =
	/^(?:(?:libc|libm|libpthread|libdl|librt)\.so(?:\.\d+)*|ld-linux(?:-[a-z0-9_-]+)?\.so(?:\.\d+)*)$/u;
const require_ = (ok, message) => {
	if (!ok) throw new Error(`Runtime symbols unavailable: ${message}`);
};
const fingerprint = (s) =>
	`${s.dev}:${s.ino}:${s.size}:${s.mode}:${s.uid}:${s.mtimeMs}:${s.ctimeMs}`;
const immutable = (s) =>
	s.isFile() && !s.isSymbolicLink?.() && s.uid === 0 && !(s.mode & 0o022);
const cleanPath = (p) =>
	path.isAbsolute(p) && path.normalize(p) === p && !/[\x00-\x20\\]/u.test(p);

async function boundedRead(io, filename, cap, elf = false) {
	const file = await io.open(
		filename,
		constants.O_RDONLY | constants.O_NOFOLLOW,
	);
	try {
		const before = await file.stat();
		if (elf)
			require_(
				immutable(before) && before.size <= cap,
				"runtime/debug ownership or size",
			);
		const chunks = [];
		let length = 0;
		while (length <= cap) {
			const buffer = Buffer.alloc(Math.min(65536, cap + 1 - length));
			const { bytesRead } = await file.read(buffer, 0, buffer.length, null);
			if (!bytesRead) break;
			length += bytesRead;
			require_(length <= cap, "read cap");
			chunks.push(buffer.subarray(0, bytesRead));
		}
		const bytes = Buffer.concat(chunks);
		const after = await file.stat();
		if (elf) {
			require_(
				bytes.subarray(0, 4).equals(Buffer.from([127, 69, 76, 70])),
				"runtime/debug ELF",
			);
			require_(
				fingerprint(before) === fingerprint(after),
				"ELF changed while reading",
			);
		}
		return { bytes, metadata: after };
	} finally {
		await file.close();
	}
}

function mappings(bytes) {
	const text = bytes.toString("utf8");
	require_(
		Buffer.from(text).equals(bytes) && text.endsWith("\n"),
		"malformed maps encoding",
	);
	const found = new Map(),
		ranges = new Set();
	for (const line of text.trimEnd().split("\n")) {
		const match =
			/^([0-9a-f]+)-([0-9a-f]+) ([r-][w-][x-][ps]) ([0-9a-f]+) ([0-9a-f]+):([0-9a-f]+) (\d+)(?:\s+(.+))?$/u.exec(
				line,
			);
		require_(
			match && BigInt(`0x${match[1]}`) < BigInt(`0x${match[2]}`),
			"malformed map",
		);
		const range = `${match[1]}-${match[2]}`;
		require_(!ranges.has(range), "duplicate map range");
		ranges.add(range);
		const filename = match[8];
		if (!filename) continue;
		const withoutDeleted = filename.replace(/ \(deleted\)$/u, "");
		if (!RUNTIME.test(path.basename(withoutDeleted))) continue;
		require_(cleanPath(filename), "deleted or unsafe runtime path");
		const identity = {
			inode: BigInt(match[7]),
			major: BigInt(`0x${match[5]}`),
			minor: BigInt(`0x${match[6]}`),
		};
		require_(identity.inode > 0n, "runtime inode absent");
		const previous = found.get(filename);
		require_(
			!previous ||
				Object.keys(identity).every((k) => identity[k] === previous[k]),
			"conflicting runtime mappings",
		);
		found.set(filename, identity);
	}
	require_(found.size > 0 && found.size <= 32, "runtime count");
	require_(
		[...found.keys()].some((p) =>
			/^libc\.so(?:\.\d+)*$/u.test(path.basename(p)),
		) && [...found.keys()].some((p) => path.basename(p).startsWith("ld-linux")),
		"libc/loader mapping absent",
	);
	return found;
}

function matchesDevice(metadata, mapped) {
	const dev = BigInt(metadata.dev);
	const major = ((dev >> 8n) & 0xfffn) | ((dev >> 32n) & 0xfffff000n);
	const minor = (dev & 0xffn) | ((dev >> 12n) & 0xffffff00n);
	return (
		BigInt(metadata.ino) === mapped.inode &&
		major === mapped.major &&
		minor === mapped.minor
	);
}

export async function verifyRuntimeSymbols({ root, binding }, injected = {}) {
	const io = {
		open,
		lstat,
		realpath,
		boundedCommand,
		privateDirectory,
		verifyIdentity,
		...injected,
	};
	require_(
		Number.isSafeInteger(binding?.pid) && binding.pid > 0,
		"invalid process binding",
	);
	await io.privateDirectory(root);
	await io.verifyIdentity(binding);
	const maps = await boundedRead(io, `/proc/${binding.pid}/maps`, MAP_CAP);
	await io.verifyIdentity(binding);
	const libraries = [];
	for (const [mappedPath, mapped] of mappings(maps.bytes)) {
		const runtime = await io.realpath(mappedPath);
		require_(cleanPath(runtime), "unsafe resolved runtime path");
		const checkNamed = async (filename, metadata) => {
			const named = await io.lstat(filename);
			require_(
				immutable(named) &&
					fingerprint(named) === fingerprint(metadata) &&
					(await io.realpath(filename)) === filename,
				"ELF identity changed",
			);
		};
		const binary = await boundedRead(io, runtime, ELF_CAP, true);
		require_(
			matchesDevice(binary.metadata, mapped),
			"mapped runtime identity mismatch",
		);
		await checkNamed(runtime, binary.metadata);
		const ordinal = libraries.length;
		const inspect = async (filename, name, metadata) => {
			const output = await io.boundedCommand(
				root,
				`runtime-${ordinal}-${name}`,
				"/usr/bin/readelf",
				["--notes", "--sections", "--wide", "--", filename],
				OUTPUT_CAP,
			);
			require_(
				typeof output === "string" && Buffer.byteLength(output) <= OUTPUT_CAP,
				"readelf output cap",
			);
			await checkNamed(filename, metadata);
			const ids = [...output.matchAll(/Build ID:\s*([0-9a-f]+)\s*$/gmu)];
			require_(
				ids.length === 1 &&
					/^[0-9a-f]{16,128}$/u.test(ids[0][1]) &&
					ids[0][1].length % 2 === 0,
				"missing/ambiguous build ID",
			);
			return {
				id: ids[0][1],
				symbols: /\]\s+\.symtab\s+SYMTAB\b/u.test(output),
			};
		};
		const elf = await inspect(runtime, "elf", binary.metadata);
		const debugPath = `/usr/lib/debug/.build-id/${elf.id.slice(0, 2)}/${elf.id.slice(2)}.debug`;
		require_(
			(await io.realpath(debugPath)) === debugPath,
			"debug path redirect",
		);
		const debug = await boundedRead(io, debugPath, ELF_CAP, true);
		const symbols = await inspect(debugPath, "debug", debug.metadata);
		require_(
			elf.id === symbols.id && symbols.symbols,
			"debug build ID or symtab mismatch",
		);
		await checkNamed(runtime, binary.metadata);
		require_(
			(await io.realpath(mappedPath)) === runtime,
			"mapped path changed",
		);
		libraries.push({
			mapped_path: mappedPath,
			runtime_path: runtime,
			debug_path: debugPath,
			build_id: elf.id,
			runtime_sha256: createHash("sha256").update(binary.bytes).digest("hex"),
			debug_sha256: createHash("sha256").update(debug.bytes).digest("hex"),
		});
	}
	await io.verifyIdentity(binding);
	return {
		schema: 1,
		library_count: libraries.length,
		maps_sha256: createHash("sha256").update(maps.bytes).digest("hex"),
		libraries,
	};
}
