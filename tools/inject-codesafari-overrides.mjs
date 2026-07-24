import { readFile, writeFile } from "node:fs/promises";

const [, , indexPath] = process.argv;
if (!indexPath) {
  throw new Error("usage: node tools/inject-codesafari-overrides.mjs <index.html>");
}

const marker = '<link rel="stylesheet" href="./codesafari-overrides.css">';
const source = await readFile(indexPath, "utf8");
if (source.includes(marker)) {
  process.exit(0);
}
if (!source.includes("</head>")) {
  throw new Error(`CodeSafari export has no </head>: ${indexPath}`);
}

await writeFile(indexPath, source.replace("</head>", `    ${marker}\n  </head>`));
