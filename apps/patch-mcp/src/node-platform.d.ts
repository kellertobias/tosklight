/**
 * The two pieces of the platform this server touches, declared rather than depended on.
 *
 * Pulling in `@types/node` for one file read would bring the whole platform's surface into a
 * package that otherwise runs on nothing but `fetch`. Declaring exactly what is used keeps the
 * call typed and the dependency list honest — the same reason `main.ts` declares `process`.
 */
declare module "node:fs/promises" {
	export function readFile(path: string, encoding: string): Promise<string>;
}
