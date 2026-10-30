import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { Desk } from "../apps/patch-mcp/src/desk";
import { createServer } from "../apps/patch-mcp/src/server";
import { expect, test } from "./bench/core/fixtures";
import { loadCanonicalCopy, objects } from "./support/catalog";

type ToolResult = { isError?: boolean; content: Array<{ type: string; text?: string; data?: string; mimeType?: string }> };

test("TL-854 @api › the programming MCP tools select, color, aim and record a Cue on the shared desk command line", async ({ api, bench }) => {
	test.setTimeout(120_000);
	await loadCanonicalCopy(api, bench, "programming-mcp", "default-stage");
	const deskId = api.session!.desk.id;
	const server = createServer(new Desk({ baseUrl: api.baseUrl, deskId }));
	const client = new Client({ name: "tl-854", version: "1.0.0" });
	const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
	await Promise.all([server.connect(serverTransport), client.connect(clientTransport)]);
	const call = async (name: string, args: Record<string, unknown> = {}) =>
		(await client.callTool({ name, arguments: args })) as ToolResult;
	const json = async (name: string, args: Record<string, unknown> = {}) => {
		const result = await call(name, args);
		expect(result.isError, `${name}: ${result.content[0]?.text}`).toBeFalsy();
		return JSON.parse(result.content[0].text!);
	};

	const listed = (await client.listTools()).tools.map((tool) => tool.name);
	expect(listed).toEqual(expect.arrayContaining(["select", "set_values", "record_cue", "snapshot", "run_command"]));

	// Find the fixtures that take colour and position, record them as a Group, then program it.
	await json("select", { fixtures: "1 THRU 999" });
	await json("set_values", { color: { red: 100 }, position: { pan: 10, tilt: 10 } });
	const probe = await json("get_programmer");
	const byFixture = new Map<string, Set<string>>();
	for (const value of probe.values as Array<{ fixture: string; attribute: string }>)
		byFixture.set(value.fixture, (byFixture.get(value.fixture) ?? new Set()).add(value.attribute));
	const movers = [...byFixture].filter(([, attributes]) => attributes.has("color") && attributes.has("position")).map(([fixture]) => fixture);
	expect(movers.length).toBeGreaterThan(0);
	await json("clear", { values: true });
	await json("run_command", { command: `FIXTURE ${movers.join(" + ")} RECORD GROUP 854` });
	const groups: Array<{ group: string; fixtures: string[] }> = await json("list_groups");
	const chosen = groups.find((group) => group.group === "854")?.group;
	expect(chosen).toBe("854");
	await json("clear", { values: true });
	await json("select", { group: chosen });
	await json("set_values", { color: { red: 100, green: 40, blue: 0 }, position: { pan: 30, tilt: 20 } });
	const programmer = await json("get_programmer");
	const attributes = new Set([
		...programmer.values.map((value: { attribute: string }) => value.attribute),
		...Object.values(programmer.group_values as Record<string, Record<string, unknown>>).flatMap((values) => Object.keys(values)),
	]);
	expect([...attributes]).toEqual(expect.arrayContaining(["color", "position"]));
	expect((await call("run_command", { command: "FIXTURE 1 RECORD GROUP 854" })).isError).toBe(true);
	await json("set_values", { intensity: 80 });

	// The line the tools type is the desk's own line: this API session sees its revision move.
	await json("run_command", { command: `GROUP ${chosen} AT 75` });
	const history = await api.request<{ entries?: Array<{ command: string; desk_id: string }> } | Array<{ command: string; desk_id: string }>>("GET", "/api/v2/command-history");
	const entries = Array.isArray(history) ? history : (history.entries ?? []);
	expect(entries.find((entry) => /G(ROUP)? ?854 AT 75/.test(entry.command))?.desk_id).toBe(deskId);

	const recorded = await json("record_cue", { cue_list: 854, cue: "1", fade_seconds: 3 });
	expect(recorded.outcome).toBe("accepted");
	const list = (await objects<any>(api, "cue_list")).find((item) => item.body.pool_number === 854)!;
	expect(list.body.cues).toHaveLength(1);
	expect(list.body.cues[0].fade_millis).toBe(3000);
	const stored = [...list.body.cues[0].changes, ...(list.body.cues[0].group_changes ?? [])].map((change: { attribute: string }) => change.attribute);
	expect(stored).toEqual(expect.arrayContaining(["color", "position", "intensity"]));

	// Overwriting or deleting needs an explicit confirm.
	const refused = await call("record_cue", { cue_list: 854, cue: "1" });
	expect(refused.isError).toBe(true);
	expect(refused.content[0].text).toContain("confirm: true");
	expect((await call("update_cue", { cue_list: 854, cue: "1" })).isError).toBe(true);
	expect((await call("run_command", { command: "DELETE CUELIST 854" })).isError).toBe(true);
	expect((await objects<any>(api, "cue_list")).some((item) => item.body.pool_number === 854)).toBe(true);
	await json("set_values", { intensity: 40 });
	await json("update_cue", { cue_list: 854, cue: "1", mode: "update", confirm: true });

	const cues = await json("list_cue_lists", { cue_list: 854, include_values: true });
	expect(cues[0].cues[0]).toMatchObject({ cue: "1", fade_seconds: 3 });
	expect(cues[0].cues[0].values.length).toBeGreaterThan(0);

	const snapshot = await call("snapshot");
	expect(snapshot.isError).toBeFalsy();
	const image = snapshot.content.find((part) => part.type === "image")!;
	expect(image.mimeType).toBe("image/png");
	if (process.env.SNAPSHOT_OUT) (await import("node:fs")).writeFileSync(process.env.SNAPSHOT_OUT, Buffer.from(image.data!, "base64"));
	expect(Buffer.from(image.data!, "base64").subarray(1, 4).toString()).toBe("PNG");
	expect(snapshot.content.find((part) => part.type === "text")!.text).toContain("intensity_percent");

	const summary = await json("get_show_summary");
	expect(summary.cue_lists).toBeGreaterThan(0);
	await client.close();
});
