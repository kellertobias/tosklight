// Keep every published page linked to the Impressum and the Datenschutzerklärung, and keep the
// assembled site free of third-party requests. Pages the repository renders itself carry the links
// in their own footers; pages produced by other tools (manual renderer, Storybook, CodeSafari,
// semantic-test catalogue) get a small footer injected after assembly.

import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join, posix, relative, sep } from "node:path";
import { DATENSCHUTZ_PATH, IMPRESSUM_PATH, legalLinks } from "./pages.mjs";

export const LEGAL_FOOTER_MARKER = "data-tosklight-legal";

// Hosts a published page may load resources from. Empty on purpose: everything is self-hosted,
// so a new entry here also needs a matching disclosure in the Datenschutzerklärung.
export const ALLOWED_RESOURCE_HOSTS = Object.freeze([]);

// Well-known third-party request targets. Any mention outside the CodeSafari source payload (which
// displays repository files as text and never requests them) fails the check, even as a string.
export const FORBIDDEN_HOSTS = Object.freeze([
	"fonts.googleapis.com",
	"fonts.gstatic.com",
	"ajax.googleapis.com",
	"www.googletagmanager.com",
	"www.google-analytics.com",
	"google-analytics.com",
	"stats.g.doubleclick.net",
	"connect.facebook.net",
	"cdn.jsdelivr.net",
	"unpkg.com",
	"cdnjs.cloudflare.com",
	"use.typekit.net",
	"use.fontawesome.com",
	"kit.fontawesome.com",
	"plausible.io",
	"static.cloudflareinsights.com",
	"www.youtube.com/embed",
	"www.youtube-nocookie.com",
	"player.vimeo.com",
	"maps.googleapis.com",
	"www.gravatar.com",
]);

// Paths whose files are data shown as text, not code or markup the browser acts on.
const TEXT_PAYLOAD_PREFIXES = ["safari/data/"];

const toPosix = (path) => path.split(sep).join("/");

export function listSiteFiles(siteDir, extensions) {
	const files = [];
	const walk = (dir) => {
		for (const name of readdirSync(dir).sort()) {
			const path = join(dir, name);
			if (statSync(path).isDirectory()) walk(path);
			else if (extensions.some((extension) => name.endsWith(extension))) {
				files.push(toPosix(relative(siteDir, path)));
			}
		}
	};
	walk(siteDir);
	return files;
}

const rootPrefix = (page) => "../".repeat(page.split("/").length - 1);

// A fixed chip rather than a page footer: Storybook, CodeSafari and the manual are full-height
// applications whose own layout would hide anything appended below them. Inside the Storybook
// manager the preview frame hides its copy, so the operator sees one chip, not two.
export function legalFooter(page) {
	const framed = page.endsWith("storybook/iframe.html")
		? `<script>if (window.self !== window.top) document.currentScript.parentElement.hidden = true;</script>`
		: "";
	return (
		`<footer ${LEGAL_FOOTER_MARKER} style="position:fixed;right:.5rem;bottom:.5rem;z-index:2147483647;` +
		`padding:.2rem .55rem;border-radius:.4rem;background:rgba(7,9,13,.82);color:#c9d1dc;` +
		`font:12px/1.4 system-ui,sans-serif">` +
		`<style>[${LEGAL_FOOTER_MARKER}] a{color:inherit;text-decoration:underline}</style>` +
		`${legalLinks(rootPrefix(page))}${framed}</footer>`
	);
}

function hrefs(html) {
	return [...html.matchAll(/<a\b[^>]*?\shref\s*=\s*(["'])(.*?)\1/gisu)].map((match) => match[2]);
}

function resolvesTo(page, href, target) {
	if (/^[a-z][a-z0-9+.-]*:|^\/\//iu.test(href) || href.startsWith("/")) return false;
	const path = href.split(/[?#]/u)[0];
	const resolved = posix.normalize(posix.join(posix.dirname(page), path)).replace(/\/$/u, "");
	const wanted = target.replace(/\/$/u, "");
	return resolved === wanted || resolved === `${wanted}/index.html`;
}

export function hasLegalLinks(page, html) {
	const links = hrefs(html);
	return [IMPRESSUM_PATH, DATENSCHUTZ_PATH].every((target) =>
		links.some((href) => resolvesTo(page, href, target)),
	);
}

/** Add the footer to every HTML page that does not already link both legal pages. */
export function injectLegalLinks(siteDir) {
	const injected = [];
	for (const page of listSiteFiles(siteDir, [".html"])) {
		const path = join(siteDir, page);
		const html = readFileSync(path, "utf8");
		if (hasLegalLinks(page, html)) continue;
		const footer = legalFooter(page);
		const closing = html.search(/<\/body>/iu);
		writeFileSync(
			path,
			closing === -1 ? `${html}${footer}` : `${html.slice(0, closing)}${footer}${html.slice(closing)}`,
		);
		injected.push(page);
	}
	return injected;
}

const isAbsolute = (url) => /^(?:[a-z][a-z0-9+.-]*:)?\/\//iu.test(url.trim());
const hostOf = (url) => {
	try {
		return new URL(url.trim().replace(/^\/\//u, "https://")).host;
	} catch {
		return url;
	}
};
const allowedResource = (url) => ALLOWED_RESOURCE_HOSTS.includes(hostOf(url));

function cssRequests(css) {
	return [
		...[...css.matchAll(/@import\s+(?:url\()?\s*["']?([^"')\s;]+)/giu)].map((match) => match[1]),
		...[...css.matchAll(/url\(\s*["']?([^"')]+)["']?\s*\)/giu)].map((match) => match[1]),
	];
}

function htmlRequests(html) {
	const urls = [];
	for (const [tag] of html.matchAll(/<(?:script|link|img|iframe|frame|source|video|audio|track|embed|object|input|image|use)\b[^>]*>/gisu)) {
		for (const match of tag.matchAll(/\s(?:src|href|poster|data|xlink:href)\s*=\s*(["'])(.*?)\1/gisu)) {
			// A plain <link rel="canonical"> or hyperlink is not fetched, but every other <link> is.
			if (/^<link\b/iu.test(tag) && /\srel\s*=\s*["']?(?:canonical|alternate)\b/iu.test(tag)) continue;
			urls.push(match[2]);
		}
		for (const match of tag.matchAll(/\ssrcset\s*=\s*(["'])(.*?)\1/gisu)) {
			urls.push(...match[2].split(",").map((candidate) => candidate.trim().split(/\s+/u)[0]));
		}
	}
	for (const match of html.matchAll(/<meta\b[^>]*http-equiv\s*=\s*["']?refresh[^>]*content\s*=\s*["'][^"']*url=([^"']+)/gisu)) {
		urls.push(match[1]);
	}
	for (const match of html.matchAll(/<style\b[^>]*>(.*?)<\/style>/gisu)) urls.push(...cssRequests(match[1]));
	for (const match of html.matchAll(/\sstyle\s*=\s*(["'])(.*?)\1/gisu)) urls.push(...cssRequests(match[2]));
	return urls;
}

// Literal network calls in scripts. A URL string inside a script is usually a documentation link
// or an error message, so only the shapes that make the browser fetch are treated as requests.
const SCRIPT_REQUEST =
	/(?:\bfetch|\bimportScripts|\bimport|\bnew\s+(?:WebSocket|EventSource|Worker|SharedWorker))\s*\(\s*["'`]((?:https?|wss?):\/\/[^"'`]+)|\.(?:src|href)\s*=\s*["'`]((?:https?:)?\/\/[^"'`]+)|\bsrc\s*:\s*["'`]((?:https?:)?\/\/[^"'`]+)/gu;

function scriptRequests(script) {
	return [...script.matchAll(SCRIPT_REQUEST)].map((match) => match[1] ?? match[2] ?? match[3]);
}

/**
 * Check an assembled site. Returns a list of human-readable problems; an empty list passes.
 */
export function checkSite(siteDir) {
	const problems = [];
	for (const target of [IMPRESSUM_PATH, DATENSCHUTZ_PATH]) {
		if (!existsSync(join(siteDir, target, "index.html"))) problems.push(`missing page ${target}index.html`);
	}
	const pages = listSiteFiles(siteDir, [".html", ".htm"]);
	if (!pages.length) problems.push("the site has no HTML pages");
	for (const page of pages) {
		const html = readFileSync(join(siteDir, page), "utf8");
		if (!hasLegalLinks(page, html)) problems.push(`${page}: no link to both ${IMPRESSUM_PATH} and ${DATENSCHUTZ_PATH}`);
		for (const url of htmlRequests(html)) {
			if (isAbsolute(url) && !allowedResource(url)) problems.push(`${page}: loads third-party resource ${url}`);
		}
	}
	for (const file of listSiteFiles(siteDir, [".css"])) {
		for (const url of cssRequests(readFileSync(join(siteDir, file), "utf8"))) {
			if (isAbsolute(url) && !allowedResource(url)) problems.push(`${file}: loads third-party resource ${url}`);
		}
	}
	for (const file of listSiteFiles(siteDir, [".js", ".mjs", ".cjs"])) {
		if (TEXT_PAYLOAD_PREFIXES.some((prefix) => file.startsWith(prefix))) continue;
		for (const url of scriptRequests(readFileSync(join(siteDir, file), "utf8"))) {
			if (!allowedResource(url)) problems.push(`${file}: requests third-party resource ${url}`);
		}
	}
	for (const file of listSiteFiles(siteDir, [".html", ".htm", ".css", ".js", ".mjs", ".cjs", ".json", ".svg", ".webmanifest"])) {
		if (TEXT_PAYLOAD_PREFIXES.some((prefix) => file.startsWith(prefix))) continue;
		const text = readFileSync(join(siteDir, file), "utf8");
		for (const host of FORBIDDEN_HOSTS) {
			if (text.includes(`//${host}`)) problems.push(`${file}: references third-party service ${host}`);
		}
	}
	return problems;
}
