import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import type {
	Locator,
	Page,
} from "../../../apps/control-ui/node_modules/@playwright/test/index.js";

const sqlite = promisify(execFile);

export async function runSql(file: string, sql: string): Promise<void> {
	await sqlite("sqlite3", [file, sql]);
}

export async function readSql(file: string, sql: string): Promise<string> {
	return (await sqlite("sqlite3", ["-noheader", file, sql])).stdout.trim();
}

export async function objectRows(
	file: string,
	excludedKind: string,
	excludedId: string,
): Promise<string> {
	const kind = excludedKind.replaceAll("'", "''");
	const id = excludedId.replaceAll("'", "''");
	return readSql(
		file,
		`SELECT group_concat(kind||'|'||id||'|'||revision||'|'||length(body_json), char(10)) FROM (SELECT kind,id,revision,body_json FROM objects WHERE NOT (kind='${kind}' AND id='${id}') ORDER BY kind,id)`,
	);
}

export async function extractFixtureAsset(
	archive: string,
	asset: string,
	destination: string,
): Promise<void> {
	const archivePath = fileURLToPath(
		new URL(`../../../assets/fixture-library/${archive}`, import.meta.url),
	);
	const bytes = await new Promise<Buffer>((resolve, reject) => {
		execFile(
			"unzip",
			["-p", archivePath, asset],
			{ encoding: "buffer", maxBuffer: 2 * 1024 * 1024 },
			(error, stdout) => {
				if (error) reject(error);
				else resolve(Buffer.from(stdout));
			},
		);
	});
	await fs.writeFile(destination, bytes);
}

export async function selectConfinedFile(
	page: Page,
	filename: string,
): Promise<void> {
	const picker = page.getByRole("dialog", { name: "Choose files or folders" });
	await expect(picker).toBeVisible();
	await picker.getByRole("button", { name: `${filename}, file` }).click();
	await picker.getByRole("button", { name: "Select", exact: true }).click();
	await expect(picker).toBeHidden();
}

export async function chooseCustomSelect(
	container: Locator,
	label: string,
	option: string,
): Promise<void> {
	const field = container
		.getByText(label, { selector: "label", exact: true })
		.locator("..");
	await field.locator(".ui-select-trigger").click();
	await container
		.page()
		.getByRole("option", { name: option, exact: true })
		.click();
}
