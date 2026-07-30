import { randomBytes } from "node:crypto";
import { connect } from "node:net";

export async function startSlowVisualizationClient(baseUrl, token) {
	const socket = await openRawVisualizationSocket(baseUrl, token);
	socket.write(
		maskedWebSocketTextFrame({
			type: "subscribe",
			lanes: ["normal", "preload"],
			max_rate_hz: 10,
		}),
	);
	socket.pause();
	socket.on("error", () => undefined);
	let resynchronizePreload = false;
	const resynchronize = setInterval(() => {
		if (socket.destroyed) return;
		resynchronizePreload = !resynchronizePreload;
		socket.write(
			maskedWebSocketTextFrame({
				type: "resynchronize",
				lane: resynchronizePreload ? "preload" : "normal",
			}),
		);
	}, 100);
	resynchronize.unref?.();
	return {
		async close() {
			clearInterval(resynchronize);
			socket.resume();
			if (socket.closed) return;
			const closed = new Promise((resolve) => socket.once("close", resolve));
			socket.destroy();
			socket.unref();
			await closed;
		},
	};
}

async function openRawVisualizationSocket(baseUrl, token) {
	const url = new URL(baseUrl);
	if (url.protocol !== "http:")
		throw new Error("Raw slow-client proof currently requires local HTTP");
	const socket = connect({
		host: url.hostname,
		port: Number(url.port || 80),
	});
	const key = randomBytes(16).toString("base64");
	socket.write(
		[
			"GET /api/v2/visualization/stream HTTP/1.1",
			`Host: ${url.host}`,
			"Connection: Upgrade",
			"Upgrade: websocket",
			"Sec-WebSocket-Version: 13",
			`Sec-WebSocket-Key: ${key}`,
			`Sec-WebSocket-Protocol: light.visualization.v1, light.token.${token}`,
			"",
			"",
		].join("\r\n"),
	);
	await new Promise((resolve, reject) => {
		let response = "";
		const timeout = setTimeout(
			() => reject(new Error("Slow visualization client did not connect")),
			5_000,
		);
		const finish = (error) => {
			clearTimeout(timeout);
			socket.off("data", onData);
			socket.off("error", onError);
			if (error) reject(error);
			else resolve();
		};
		const onError = (reason) => finish(reason);
		const onData = (chunk) => {
			response += chunk.toString("latin1");
			const headersEnd = response.indexOf("\r\n\r\n");
			if (headersEnd < 0) return;
			const status = response.slice(0, response.indexOf("\r\n"));
			if (!status.includes(" 101 "))
				finish(new Error(`Slow visualization handshake failed: ${status}`));
			else finish();
		};
		socket.on("data", onData);
		socket.on("error", onError);
	});
	return socket;
}

function maskedWebSocketTextFrame(value) {
	const payload = Buffer.from(JSON.stringify(value));
	if (payload.length >= 126)
		throw new Error("Slow-client subscription exceeds the small frame encoder");
	const mask = randomBytes(4);
	const frame = Buffer.alloc(2 + mask.length + payload.length);
	frame[0] = 0x81;
	frame[1] = 0x80 | payload.length;
	mask.copy(frame, 2);
	for (let index = 0; index < payload.length; index++)
		frame[6 + index] = (payload[index] ?? 0) ^ (mask[index % 4] ?? 0);
	return frame;
}
