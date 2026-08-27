/**
 * The ToskLight patch MCP server.
 *
 * Exposes the patch capabilities as MCP tools over stdio, against a running desk. What it can do
 * is deliberately what an operator can do through the desk's own API — this server holds no
 * private path into a show.
 */

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import {
	CallToolRequestSchema,
	ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { Desk, DeskError, type DeskOptions } from "./desk";
import { tools } from "./tools";

export function createServer(options: DeskOptions): Server {
	const desk = new Desk(options);
	const server = new Server(
		{ name: "tosklight-patch", version: "0.1.0" },
		{ capabilities: { tools: {} } },
	);

	server.setRequestHandler(ListToolsRequestSchema, async () => ({
		tools: tools.map((tool) => ({
			name: tool.name,
			description: tool.description,
			inputSchema: tool.inputSchema,
		})),
	}));

	server.setRequestHandler(CallToolRequestSchema, async (request) => {
		const tool = tools.find((candidate) => candidate.name === request.params.name);
		if (!tool) {
			return {
				isError: true,
				content: [
					{ type: "text", text: `no such tool: ${request.params.name}` },
				],
			};
		}
		try {
			const result = await tool.run(desk, request.params.arguments ?? {});
			return {
				content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
			};
		} catch (error) {
			// A refused edit is ordinary — a stale revision, a fixture that is not there — and the
			// caller can act on the desk's own words. Only the message crosses, never a stack.
			const message =
				error instanceof DeskError || error instanceof Error
					? error.message
					: String(error);
			return { isError: true, content: [{ type: "text", text: message }] };
		}
	});

	return server;
}
