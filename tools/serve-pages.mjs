#!/usr/bin/env node
// Minimal static file server for previewing the assembled public site locally.
// Dependency-free so `npm run pages:serve` needs nothing beyond Node.

import { createServer } from "node:http";
import { createReadStream, statSync } from "node:fs";
import { extname, join, normalize, resolve } from "node:path";

const root = resolve(process.argv[2] ?? ".");
const port = Number(process.argv[3] ?? 8080);

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".webp": "image/webp",
  ".pdf": "application/pdf",
  ".woff2": "font/woff2",
  ".map": "application/json",
};

const server = createServer((request, response) => {
  // Strip the query string and block path traversal outside the served root.
  const requestPath = decodeURIComponent((request.url ?? "/").split("?")[0]);
  let target = normalize(join(root, requestPath));
  if (!target.startsWith(root)) {
    response.writeHead(403).end("Forbidden");
    return;
  }

  try {
    if (statSync(target).isDirectory()) target = join(target, "index.html");
  } catch {
    response.writeHead(404).end("Not found");
    return;
  }

  try {
    const size = statSync(target).size;
    response.writeHead(200, {
      "content-type": TYPES[extname(target)] ?? "application/octet-stream",
      "content-length": size,
    });
    createReadStream(target).pipe(response);
  } catch {
    response.writeHead(404).end("Not found");
  }
});

server.listen(port, "127.0.0.1", () => {
  console.log(`Serving ${root} at http://127.0.0.1:${port}/`);
});
