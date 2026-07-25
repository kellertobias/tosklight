import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";

const repositoryRoot = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	"../..",
);
const controlUiRequire = createRequire(
	path.join(repositoryRoot, "apps/light-desktop/package.json"),
);

let modules;

export async function loadTypeScriptAst() {
	if (!modules) {
		const syncApi = pathToFileURL(
			controlUiRequire.resolve("typescript/unstable/sync"),
		).href;
		const astApi = pathToFileURL(
			controlUiRequire.resolve("typescript/unstable/ast"),
		).href;
		modules = Promise.all([import(syncApi), import(astApi)]).then(
			([sync, ast]) => ({ ...sync, ...ast }),
		);
	}
	return modules;
}
