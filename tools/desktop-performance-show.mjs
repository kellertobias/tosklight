import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

/** Seed only the fresh benchmark data directory, before its owned process is launched. */
export async function preseedPerformanceShow(sourceFile, dataDirectory) {
	const bytes = await readFile(path.resolve(sourceFile));
	if (!bytes.subarray(0, 16).equals(Buffer.from("SQLite format 3\0", "binary")))
		throw new Error("Performance demo source is not a SQLite show file");
	const directory = path.join(dataDirectory, "shows");
	await mkdir(directory, { recursive: true });
	await writeFile(path.join(directory, "Default Stage Show.show"), bytes, {
		flag: "wx",
	});
	return {
		source_show_file: path.basename(sourceFile),
		source_show_sha256: createHash("sha256").update(bytes).digest("hex"),
		source_show_bytes: bytes.length,
		source_show_loading_mode: "startup-preseed",
		source_show_seed_path: "shows/Default Stage Show.show",
	};
}
