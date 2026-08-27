#!/usr/bin/env node
/**
 * Run the patch MCP server against a desk.
 *
 * `TOSKLIGHT_URL` says where the desk is listening, and `TOSKLIGHT_DESK_ID` is the alias this
 * server presents as. One alias is one desk: pointing two of these at one desk under one alias
 * would make them the same desk, which is not what a second tool wants.
 */

import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { createServer } from "./server";

// Only the two settings this server reads. Declaring them keeps the package free of a Node type
// dependency it would otherwise need for one property.
declare const process: { env: Record<string, string | undefined> };

const server = createServer({
	baseUrl: process.env.TOSKLIGHT_URL ?? "http://127.0.0.1:5000",
	deskId: process.env.TOSKLIGHT_DESK_ID ?? "patch-mcp",
});

await server.connect(new StdioServerTransport());
