import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { artifactPaths } from "../artifact-paths.mjs";
import { IMPRINT_CONTACT_ENV, IMPRINT_CONTACT_KEYS, ImprintContactError, parseImprintContact } from "./contact.mjs";
import { checkSite, injectLegalLinks } from "./links.mjs";
import { renderDatenschutz, renderImpressum } from "./pages.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(HERE, "cli.mjs");
const EXAMPLE_RAW = readFileSync(resolve(HERE, "fixtures/imprint-contact.example.json"), "utf8");
const EXAMPLE = JSON.parse(EXAMPLE_RAW);

const decodeEntities = (html) =>
	html.replace(/&#x([0-9a-f]+);/giu, (_, hex) => String.fromCodePoint(Number.parseInt(hex, 16)));

function temporarySite() {
	mkdirSync(artifactPaths.tmp, { recursive: true });
	return mkdtempSync(join(artifactPaths.tmp, "site-legal-"));
}

function writeFile(root, path, content) {
	mkdirSync(dirname(join(root, path)), { recursive: true });
	writeFileSync(join(root, path), content);
}

test("the example contact validates and every documented key is required", () => {
	const { contact, unknownKeys } = parseImprintContact(EXAMPLE_RAW);
	assert.deepEqual(Object.keys(contact), IMPRINT_CONTACT_KEYS);
	assert.deepEqual(unknownKeys, []);
	for (const key of IMPRINT_CONTACT_KEYS) {
		for (const broken of [undefined, "", "   ", 42]) {
			const candidate = { ...EXAMPLE, [key]: broken };
			if (broken === undefined) delete candidate[key];
			assert.throws(
				() => parseImprintContact(JSON.stringify(candidate)),
				(error) => error instanceof ImprintContactError && error.message.includes(key),
			);
		}
	}
});

test("validation errors name keys but never echo values", () => {
	const secretLooking = { ...EXAMPLE, email: "not-an-address-SECRETVALUE", phone: "" };
	for (const raw of [undefined, "", "{not json SECRETVALUE", "[]", "null", JSON.stringify(secretLooking)]) {
		assert.throws(
			() => parseImprintContact(raw),
			(error) => error instanceof ImprintContactError && !error.message.includes("SECRETVALUE") && error.message.includes(IMPRINT_CONTACT_ENV),
		);
	}
	const { unknownKeys } = parseImprintContact(JSON.stringify({ ...EXAMPLE, vatId: "x" }));
	assert.deepEqual(unknownKeys, ["vatId"]);
});

test("the Impressum renders every contact field with the required statements", () => {
	const { contact } = parseImprintContact(EXAMPLE_RAW);
	const html = renderImpressum(contact);
	const visible = decodeEntities(html);
	for (const key of IMPRINT_CONTACT_KEYS) assert.ok(visible.includes(EXAMPLE[key].replace(/&/gu, "&amp;")), key);
	assert.match(html, /Angaben gemäß § 5 DDG/u);
	assert.doesNotMatch(html, /TMG|Telemediengesetz/u);
	assert.match(html, /§ 18 Abs\. 2 MStV/u);
	assert.match(html, /nicht bereit und nicht verpflichtet/u);
	assert.doesNotMatch(html, /ec\.europa\.eu\/consumers\/odr|Umsatzsteuer|Handelsregister|Kammer/u);
	assert.match(html, /German version prevails/u);
	// The address is obfuscated in the markup but stays a working mailto link.
	assert.ok(!html.includes(EXAMPLE.email));
	assert.ok(visible.includes(`href="mailto:${EXAMPLE.email}"`));
	assert.match(html, /href="tel:\+490000000000"/u);
	assert.match(html, /href="\.\.\/impressum\/"/u);
	assert.match(html, /href="\.\.\/datenschutz\/"/u);
});

test("the Datenschutzerklärung covers controller, GitHub Pages and the data-subject rights", () => {
	const { contact } = parseImprintContact(EXAMPLE_RAW);
	const html = renderDatenschutz(contact);
	assert.ok(html.includes(contact.name));
	for (const expected of [
		/GitHub, Inc\., 88 Colin P\. Kelly Jr\. Street, San Francisco, CA 94107, USA/u,
		/IP-Adresse/u,
		/Art\. 6 Abs\. 1 lit\. f DSGVO/u,
		/EU-US Data Privacy Framework/u,
		/Standardvertragsklauseln/u,
		/href="https:\/\/docs\.github\.com\/en\/site-policy\/privacy-policies\/github-general-privacy-statement"/u,
		/keine Cookies/u,
		/Art\. 15 DSGVO/u,
		/Art\. 21 DSGVO/u,
		/Der Hessische Beauftragte für Datenschutz und Informationsfreiheit/u,
		/Art\. 22 DSGVO/u,
		/per E-Mail/u,
		/German version prevails/u,
	]) {
		assert.match(html, expected);
	}
});

test("inject links every page and check rejects third-party requests", () => {
	const site = temporarySite();
	try {
		const { contact } = parseImprintContact(EXAMPLE_RAW);
		writeFile(site, "impressum/index.html", renderImpressum(contact));
		writeFile(site, "datenschutz/index.html", renderDatenschutz(contact));
		writeFile(site, "index.html", `<html><body><a href="impressum/">I</a><a href="datenschutz/index.html">D</a></body></html>`);
		writeFile(site, "manual/index.html", `<html><body><main>Manual</main></body></html>`);
		writeFile(site, "storybook/iframe.html", `<html><body><div id="root"></div></body></html>`);
		writeFile(site, "site.css", "body{color:red}");
		writeFile(site, "safari/data/source.js", `const text = "https://fonts.googleapis.com/css";`);

		assert.deepEqual(checkSite(site), [
			"manual/index.html: no link to both impressum/ and datenschutz/",
			"storybook/iframe.html: no link to both impressum/ and datenschutz/",
		]);
		assert.deepEqual(injectLegalLinks(site), ["manual/index.html", "storybook/iframe.html"]);
		const manual = readFileSync(join(site, "manual/index.html"), "utf8");
		assert.match(manual, /href="\.\.\/impressum\/">Impressum<\/a> · <a href="\.\.\/datenschutz\/">Datenschutz<\/a><\/footer><\/body>/u);
		assert.match(readFileSync(join(site, "storybook/iframe.html"), "utf8"), /window\.self !== window\.top/u);
		assert.deepEqual(injectLegalLinks(site), [], "injection is idempotent");
		assert.deepEqual(checkSite(site), []);

		writeFile(site, "fonts.html", `<html><head><link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter"></head><body><a href="impressum/">I</a><a href="datenschutz/">D</a></body></html>`);
		writeFile(site, "site.css", `@import url("https://cdn.example.net/x.css"); body{background:url(//img.example.net/a.png)}`);
		writeFile(site, "app.js", `fetch("https://api.example.net/track"); const docs = "https://example.org/docs";`);
		const problems = checkSite(site);
		assert.ok(problems.includes("fonts.html: loads third-party resource https://fonts.googleapis.com/css2?family=Inter"));
		assert.ok(problems.includes("fonts.html: references third-party service fonts.googleapis.com"));
		assert.ok(problems.includes("site.css: loads third-party resource https://cdn.example.net/x.css"));
		assert.ok(problems.includes("site.css: loads third-party resource //img.example.net/a.png"));
		assert.ok(problems.includes("app.js: requests third-party resource https://api.example.net/track"));
		assert.ok(!problems.some((problem) => problem.includes("example.org/docs")), "plain URL strings are not requests");
		assert.ok(!problems.some((problem) => problem.startsWith("safari/data/")), "CodeSafari source text is not requested");
	} finally {
		rmSync(site, { recursive: true, force: true });
	}
});

test("the CLI fails without the secret, writes both pages with it, and never prints values", () => {
	const site = temporarySite();
	try {
		writeFile(site, "index.html", `<html><body>Home</body></html>`);
		const environment = { ...process.env };
		delete environment[IMPRINT_CONTACT_ENV];
		const missing = spawnSync(process.execPath, [CLI, "write", site], { env: environment, encoding: "utf8" });
		assert.equal(missing.status, 1);
		assert.match(missing.stderr, /TOSKLIGHT_IMPRINT_CONTACT is not set or empty/u);

		const invalid = spawnSync(process.execPath, [CLI, "validate"], {
			env: { ...environment, [IMPRINT_CONTACT_ENV]: JSON.stringify({ ...EXAMPLE, city: "" }) },
			encoding: "utf8",
		});
		assert.equal(invalid.status, 1);
		assert.match(invalid.stderr, /missing or has empty values for: city/u);

		const written = spawnSync(process.execPath, [CLI, "write", site], {
			env: { ...environment, [IMPRINT_CONTACT_ENV]: EXAMPLE_RAW },
			encoding: "utf8",
		});
		assert.equal(written.status, 0, written.stderr);
		for (const value of Object.values(EXAMPLE)) {
			assert.ok(!`${written.stdout}${written.stderr}`.includes(value), "output must not contain contact values");
		}
		assert.match(readFileSync(join(site, "impressum/index.html"), "utf8"), /Erika Mustermann/u);
		assert.match(readFileSync(join(site, "datenschutz/index.html"), "utf8"), /GitHub Pages/u);
		assert.match(readFileSync(join(site, "imprint/index.html"), "utf8"), /url=\.\.\/impressum\//u);
		assert.match(readFileSync(join(site, "index.html"), "utf8"), /href="impressum\/"/u);
	} finally {
		rmSync(site, { recursive: true, force: true });
	}
});
