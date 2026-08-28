#!/usr/bin/env node
/**
 * Run the patch MCP server against a desk.
 *
 * `TOSKLIGHT_URL` says where the desk is listening, and `TOSKLIGHT_DESK_ID` is the alias this
 * server presents as. One alias is one desk: pointing two of these at one desk under one alias
 * would make them the same desk, which is not what a second tool wants.
 */

import { readFile } from "node:fs/promises";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { Architect } from "./architect";
import { Desk } from "./desk";
import { createServer } from "./server";

// Only the settings this server reads. Declaring them keeps the package free of a Node type
// dependency it would otherwise need for one property.
declare const process: { env: Record<string, string | undefined> };

/**
 * Which product to patch.
 *
 * `TOSKLIGHT_TARGET=architect` talks to the editor running on this machine, found through the
 * handle file it writes on startup. Anything else is the desk, which stays the default because it
 * is the one an operator points at a URL.
 */
const backend =
	process.env.TOSKLIGHT_TARGET === "architect"
		? new Architect({ readFile: (path) => readFile(path, "utf8") })
		: new Desk({
				baseUrl: process.env.TOSKLIGHT_URL ?? "http://127.0.0.1:5000",
				deskId: process.env.TOSKLIGHT_DESK_ID ?? "patch-mcp",
			});

await createServer(backend).connect(new StdioServerTransport());
