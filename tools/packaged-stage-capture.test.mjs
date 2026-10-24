import { deflateSync } from "node:zlib";
import { mkdir, mkdtemp, writeFile, rm } from "node:fs/promises";
import path from "node:path";
import { artifactPaths } from "./artifact-paths.mjs";
import test from "node:test";
import assert from "node:assert/strict";
import { captureRequested, inspectNativePng, evaluateCapture } from "./packaged-stage-capture.mjs";

test("capture is opt-in and refuses user desk, external display, phases and nonfixed modes", () => {
    assert.equal(captureRequested({}, null, null, "default-stage"), false);
    const env = { LIGHT_STAGE_PACKAGED_BENCH_CAPTURE: "live-ultra", LIGHT_STAGE_PACKAGED_BENCH_DISPLAY: "internal" };
    assert.equal(captureRequested(env, { layout: "dual" }, 5160, "canonical-demo"), true);
    for (const [config, baseline, port, profile] of [
        [env, null, 5160, "canonical-demo"], [env, { phases: "static-camera-hidden" }, 5160, "canonical-demo"],
        [env, {}, 5000, "canonical-demo"], [{ ...env, LIGHT_STAGE_PACKAGED_BENCH_DISPLAY: "external" }, {}, 5160, "canonical-demo"],
        [{ ...env, LIGHT_STAGE_PACKAGED_BENCH_STAGE: "0" }, {}, 5160, "canonical-demo"],
        [env, {}, 5160, "supported-scale"],
    ]) assert.throws(() => captureRequested(config, baseline, port, profile));
});

test("incomplete UI evidence cannot pass capture verification or performance acceptance", async () => {
    const result = await evaluateCapture({ passed: true, failures: [], ui: { buttonClicked: true } }, "/unused", {});
    assert.equal(result.captureVerified, false);
    assert.equal(result.acceptanceGateEnforced, false);
    assert.equal(result.baselineMeasurementReady, false);
    assert.equal(result.image, null);
});

test("PNG inspection rejects empty, truncated and non-PNG output", () => {
    for (const bytes of [Buffer.alloc(0), Buffer.from("not PNG"), Buffer.from([137,80,78,71,13,10,26,10])])
        assert.throws(() => inspectNativePng(bytes));
});

function chunk(type, data) {
    const bytes = Buffer.alloc(data.length + 12);
    bytes.writeUInt32BE(data.length); bytes.write(type, 4); data.copy(bytes, 8);
    let crc = 0xffffffff;
    for (const byte of bytes.subarray(4, -4)) {
        crc ^= byte;
        for (let bit = 0; bit < 8; bit++) crc = crc & 1 ? 0xedb88320 ^ (crc >>> 1) : crc >>> 1;
    }
    bytes.writeUInt32BE((crc ^ 0xffffffff) >>> 0, bytes.length - 4);
    return bytes;
}
function nativePng() {
    const header = Buffer.alloc(13);
    header.writeUInt32BE(3840); header.writeUInt32BE(2160, 4); header[8] = 8; header[9] = 6;
    const pixels = Buffer.alloc((3840 * 4 + 1) * 2160);
    return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]), chunk("IHDR", header),
        chunk("IDAT", deflateSync(pixels)), chunk("IEND", Buffer.alloc(0))]);
}

test("real PNG decode and CRC inspection are tied to the exact owned saved path and job", async () => {
    await mkdir(artifactPaths.tmp, { recursive: true });
    const directory = await mkdtemp(path.join(artifactPaths.tmp, "packaged-capture-test-"));
    try {
        const bytes = nativePng(), destination = path.join(directory, "Stage-owned-job.png");
        await writeFile(destination, bytes);
        const record = { passed: true, failures: [], ready: { kind: "stage-ready" },
            request: { paneId: "live", requestId: "request" },
            ui: { buttonClicked: true, chooserClicked: true, savedVisible: true },
            status: { paneId: "live", requestId: "request", jobId: "owned-job", engine: "ultra",
                followPreload: false, phase: { saved: { path: destination } } } };
        const result = await evaluateCapture(record, directory, {});
        assert.equal(result.captureVerified, true);
        assert.equal(result.image.width, 3840); assert.equal(result.image.height, 2160);
        assert.match(result.image.sha256, /^[0-9a-f]{64}$/);
        const bad = Buffer.from(bytes); bad[29] ^= 1;
        assert.throws(() => inspectNativePng(bad), /CRC/);
        assert.equal((await evaluateCapture({ ...record, status: { ...record.status, jobId: "foreign" } }, directory, {})).captureVerified, false);
    } finally { await rm(directory, { recursive: true, force: true }); }
});
