#!/usr/bin/env node
// Legal pages of the public site.
//
//   node tools/site-legal/cli.mjs validate            check TOSKLIGHT_IMPRINT_CONTACT only
//   node tools/site-legal/cli.mjs write <site-dir>    render Impressum + Datenschutz, link every page
//   node tools/site-legal/cli.mjs inject <site-dir>   link every page (no contact data needed)
//   node tools/site-legal/cli.mjs check <site-dir>    fail on a missing link or third-party request
//
// Output never contains contact values: only key names, page paths and counts.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { IMPRINT_CONTACT_ENV, ImprintContactError, readImprintContactFromEnvironment } from "./contact.mjs";
import { checkSite, injectLegalLinks } from "./links.mjs";
import {
	DATENSCHUTZ_PATH,
	IMPRESSUM_PATH,
	renderDatenschutz,
	renderImpressum,
	renderImprintRedirect,
} from "./pages.mjs";

function readContact() {
	const { contact, unknownKeys } = readImprintContactFromEnvironment();
	if (unknownKeys.length) {
		console.warn(`warning: ${IMPRINT_CONTACT_ENV} has unused keys: ${unknownKeys.join(", ")}`);
	}
	return contact;
}

function writePage(siteDir, path, html) {
	mkdirSync(join(siteDir, path), { recursive: true });
	writeFileSync(join(siteDir, path, "index.html"), html);
}

function requireSiteDir(siteDir) {
	if (!siteDir) {
		console.error("usage: node tools/site-legal/cli.mjs <validate|write|inject|check> [site-dir]");
		process.exit(2);
	}
	return siteDir;
}

function inject(siteDir) {
	const injected = injectLegalLinks(siteDir);
	console.log(`Linked Impressum and Datenschutz from ${injected.length} generated page(s).`);
}

function check(siteDir) {
	const problems = checkSite(siteDir);
	if (problems.length) {
		for (const problem of problems) console.error(`error: ${problem}`);
		console.error(`error: the public site failed its legal-page check (${problems.length} problem(s)).`);
		process.exit(1);
	}
	console.log("Every page links Impressum and Datenschutz; no third-party requests found.");
}

const [, , command, siteDir] = process.argv;
try {
	switch (command) {
		case "validate":
			readContact();
			console.log(`${IMPRINT_CONTACT_ENV} is valid.`);
			break;
		case "write": {
			const dir = requireSiteDir(siteDir);
			const contact = readContact();
			writePage(dir, IMPRESSUM_PATH, renderImpressum(contact));
			writePage(dir, DATENSCHUTZ_PATH, renderDatenschutz(contact));
			writePage(dir, "imprint/", renderImprintRedirect());
			inject(dir);
			check(dir);
			break;
		}
		case "inject":
			inject(requireSiteDir(siteDir));
			break;
		case "check":
			check(requireSiteDir(siteDir));
			break;
		default:
			requireSiteDir(undefined);
	}
} catch (error) {
	if (error instanceof ImprintContactError) {
		console.error(`error: ${error.message}`);
		process.exit(1);
	}
	throw error;
}
