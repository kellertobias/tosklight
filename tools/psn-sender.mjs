#!/usr/bin/env node
// Send PosiStageNet frames for named trackers, for manual testing of the desk's tracking receiver.
//
// The encoder is the bench's (tests/bench/protocols/psnSender.ts), which is byte-for-byte checked
// against the Rust encoder in crates/shared/psn/src/encode.rs. Node imports it through its
// built-in TypeScript type stripping (Node >= 22.18, or 22.6+ with --experimental-strip-types).
//
// Writes no files.
//
// Examples:
//   node tools/psn-sender.mjs --tracker 1:Presenter:0,0,2 --tracker 2:Guitar:1,0,-1
//   node tools/psn-sender.mjs --host 127.0.0.1 --port 50123 --tracker 1::0,1,2 --circle 2
//   node tools/psn-sender.mjs --tracker 1:Presenter:0,0,0 --walk 4,0,0 --duration 10

import { parseArgs } from "node:util";

const { PSN_DEFAULT_GROUP, PSN_DEFAULT_PORT, PsnSender } = await import(
	"../tests/bench/protocols/psnSender.ts"
);

const usage = `Usage: node tools/psn-sender.mjs [options]

  --host <address>        multicast group or unicast host (default ${PSN_DEFAULT_GROUP})
  --port <port>           UDP port (default ${PSN_DEFAULT_PORT})
  --interface <address>   local interface for multicast transmission
  --ttl <n>               multicast TTL (default 1)
  --tracker id:name:x,y,z one tracker in PSN metres (x right, y up, z depth); repeatable
  --rate <hz>             data frames per second (default 60)
  --duration <seconds>    stop after this long (default: until Ctrl-C)
  --circle <radius>       move every tracker on a horizontal circle of this radius (metres)
  --walk dx,dy,dz         move every tracker linearly by this many metres over --period
  --period <seconds>      period of --circle / --walk (default 4)
  --system <name>         system name in info packets (default "ToskLight PSN sender")
  --once                  send one info packet and one frame, then exit
  -h, --help              this text`;

function fail(message) {
	console.error(`psn-sender: ${message}\n\n${usage}`);
	process.exit(2);
}

function vector(text, label) {
	const parts = text.split(",").map(Number);
	if (parts.length !== 3 || parts.some((value) => !Number.isFinite(value)))
		fail(`${label} must be three numbers x,y,z; got "${text}"`);
	return parts;
}

function positiveNumber(text, label, fallback) {
	if (text === undefined) return fallback;
	const value = Number(text);
	if (!Number.isFinite(value) || value <= 0) fail(`${label} must be a positive number`);
	return value;
}

function parseTracker(text) {
	const [id, name = "", position = "0,0,0"] = text.split(":");
	const number = Number(id);
	if (!Number.isInteger(number) || number < 0 || number > 0xffff)
		fail(`tracker id must be 0..65535; got "${id}"`);
	return { id: number, name, position: vector(position, `tracker ${id} position`) };
}

let values;
try {
	({ values } = parseArgs({
		options: {
			host: { type: "string" },
			port: { type: "string" },
			interface: { type: "string" },
			ttl: { type: "string" },
			tracker: { type: "string", multiple: true },
			rate: { type: "string" },
			duration: { type: "string" },
			circle: { type: "string" },
			walk: { type: "string" },
			period: { type: "string" },
			system: { type: "string" },
			once: { type: "boolean" },
			help: { type: "boolean", short: "h" },
		},
	}));
} catch (error) {
	fail(error.message);
}
if (values.help) {
	console.log(usage);
	process.exit(0);
}

const port = values.port === undefined ? PSN_DEFAULT_PORT : Number(values.port);
if (!Number.isInteger(port) || port < 1 || port > 65535) fail("--port must be 1..65535");
const trackers = (values.tracker ?? ["1:Tracker 1:0,0,0"]).map(parseTracker);
const rate = positiveNumber(values.rate, "--rate", 60);
const period = positiveNumber(values.period, "--period", 4);
const duration = values.duration === undefined ? undefined : positiveNumber(values.duration, "--duration");
const circle = values.circle === undefined ? undefined : positiveNumber(values.circle, "--circle");
const walk = values.walk === undefined ? undefined : vector(values.walk, "--walk");

const sender = await PsnSender.open({
	host: values.host ?? PSN_DEFAULT_GROUP,
	port,
	multicastInterface: values.interface,
	ttl: values.ttl === undefined ? 1 : Number(values.ttl),
	systemName: values.system ?? "ToskLight PSN sender",
});
const names = trackers.map(({ id, name }) => ({ id, name }));
const startedAt = Date.now();

function frame() {
	const phase = ((Date.now() - startedAt) / 1000 / period) % 1;
	return trackers.map(({ id, position }) => {
		let [x, y, z] = position;
		if (circle !== undefined) {
			x += circle * Math.cos(2 * Math.PI * phase);
			z += circle * Math.sin(2 * Math.PI * phase);
		}
		if (walk) {
			x += walk[0] * phase;
			y += walk[1] * phase;
			z += walk[2] * phase;
		}
		return { id, position: [x, y, z], validity: 1 };
	});
}

console.log(
	`psn-sender: ${trackers.length} tracker(s) to ${sender.host}:${sender.port} at ${rate} Hz` +
		`${values.once ? " (one frame)" : duration ? ` for ${duration} s` : " (Ctrl-C to stop)"}`,
);

if (values.once) {
	await sender.sendInfo(names);
	await sender.sendFrame(frame());
	await sender.close();
	process.exit(0);
}

let frames = 0;
await sender.sendInfo(names);
const dataTimer = setInterval(() => {
	frames += 1;
	sender.sendFrame(frame()).catch((error) => console.error(`psn-sender: ${error.message}`));
}, 1000 / rate);
const infoTimer = setInterval(() => {
	sender.sendInfo(names).catch(() => undefined);
}, 1000);

async function stop() {
	clearInterval(dataTimer);
	clearInterval(infoTimer);
	await sender.close();
	console.log(`psn-sender: sent ${frames} frame(s)`);
	process.exit(0);
}
process.on("SIGINT", stop);
process.on("SIGTERM", stop);
if (duration !== undefined) setTimeout(stop, duration * 1000);
