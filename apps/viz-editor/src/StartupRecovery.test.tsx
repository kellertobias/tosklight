import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { StartupRecovery } from "./StartupRecovery";

const mocks = vi.hoisted(() => ({ snapshot: vi.fn(), open: vi.fn(), create: vi.fn() }));
vi.mock("./document/session", () => ({ documentSession: {startupRecovery: mocks.snapshot} }));
vi.mock("./FileBar", () => ({ chooseShowToOpen: mocks.open, chooseNewShow: mocks.create }));
const refused = {path: "/shows/old.show", reason: "unsupported programming contract", fallback: null, fallbackError: "packaged Demo Show is missing"};
beforeEach(() => {
 vi.clearAllMocks();
 mocks.snapshot.mockResolvedValue(refused);
 mocks.open.mockResolvedValue(null);
 mocks.create.mockResolvedValue(null);
});
it("keeps cancellation and rejected Open visible, then accepts a separate New Show", async () => {
 const accepted = vi.fn();
 render(<StartupRecovery onDocument={accepted} onError={vi.fn()} />);
 const notice = await screen.findByRole("alert");
 expect(notice.textContent).toContain("No fallback show is open.");
 expect(notice.textContent).toContain("packaged Demo Show is missing");
 fireEvent.click(screen.getByRole("button", {name: "Open Show"}));
 await waitFor(() => expect(mocks.open).toHaveBeenCalledOnce());
 expect(accepted).not.toHaveBeenCalled();
 mocks.open.mockRejectedValueOnce(new Error("chosen show is incompatible"));
 await waitFor(() => expect(screen.getByRole("button", {name: "Open Show"})).not.toBeDisabled());
 fireEvent.click(screen.getByRole("button", {name: "Open Show"}));
 await screen.findByText(/chosen show is incompatible/);
 expect(screen.getByRole("alert")).toBeTruthy();
 const newShow = {name: "Separate", path: "/shows/separate.show"};
 mocks.create.mockResolvedValueOnce(newShow);
 fireEvent.click(screen.getByRole("button", {name: "New Show"}));
 await waitFor(() => expect(accepted).toHaveBeenCalledWith(newShow));
 expect(screen.queryByRole("alert")).toBeNull();
});
it("uses a persistent snapshot for a late mount without needing an earlier event", async () => {
 const mounted = render(<StartupRecovery onDocument={vi.fn()} onError={vi.fn()} />);
 await screen.findByRole("alert");
 mounted.unmount();
 render(<StartupRecovery onDocument={vi.fn()} onError={vi.fn()} />);
 expect((await screen.findByRole("alert")).textContent).toContain("/shows/old.show");
 expect(mocks.snapshot).toHaveBeenCalledTimes(2);
});

it("keeps the recovery heading and content clear of actual native window controls", async () => {
 const {execFileSync} = await import("node:child_process");
 const {chromium} = await import("playwright");
 const {mkdtempSync, readFileSync, writeFileSync} = await import("node:fs");
 const {resolve, dirname} = await import("node:path");
 const {artifactPaths} = await import("../../../tools/artifact-paths.mjs");
 const root = resolve(import.meta.dirname, "../../..");
 const evidence = mkdtempSync(resolve(artifactPaths.tmp, "tl728-recovery-geometry-"));
 const source = `import React from 'react';import{createRoot}from'react-dom/client';
 import{ArchitectStartupHeader}from${JSON.stringify(resolve(root,"apps/viz-editor/src/StartupRecovery.tsx"))};
 window.__TAURI_INTERNALS__={invoke:async command=>command==='startup_recovery'?${JSON.stringify(refused)}:false};
 createRoot(document.getElementById('app')).render(<div className="viz-editor"><ArchitectStartupHeader onDocument={()=>{}} onError={()=>{}}/><div className="viz-editor-shell"><main>Show workspace</main></div></div>);`;
 const bundle = JSON.parse(execFileSync(process.execPath,["-e",`require('esbuild').build(${JSON.stringify({stdin:{contents:source,loader:"tsx",resolveDir:root},bundle:true,write:false,platform:"browser",jsx:"automatic",outfile:resolve(evidence,"fixture.js"),logLevel:"silent"})}).then(result=>process.stdout.write(JSON.stringify({outputFiles:result.outputFiles.map(file=>({path:file.path,text:file.text}))})))`],{cwd:root,encoding:"utf8",maxBuffer:32*1024*1024})) as {outputFiles:{path:string;text:string}[]};
 const styles = (path: string): string => readFileSync(path,"utf8").replace(/@import\s+"([^"]+)";/g,(_,name:string)=>styles(resolve(dirname(path),name)));
 const browser = await chromium.launch({headless:true});
 try {
  const page = await browser.newPage({viewport:{width:1440,height:900}});
  await page.setContent(`<style>${styles(resolve(root,"apps/ui-library/src/styles.css"))}${styles(resolve(root,"apps/viz-editor/src/styles.css"))}#app{height:100vh}</style><div id="app"></div>`);
  for(const file of bundle.outputFiles.filter(file=>file.path.endsWith(".css")))await page.addStyleTag({content:file.text});
  for(const file of bundle.outputFiles.filter(file=>file.path.endsWith(".js")))await page.addScriptTag({content:file.text});
  await page.getByRole("alert").waitFor();
  const measured = await page.evaluate(()=>{
   const controls=document.querySelector(".viz-native-window-controls")!.getBoundingClientRect();
   const notice=document.querySelector(".viz-startup-recovery")!;
   const boxes=[...notice.querySelectorAll("strong,p,button")].map(element=>element.getBoundingClientRect().toJSON());
   return {controls:controls.toJSON(),boxes,notice:notice.getBoundingClientRect().toJSON(),workspace:document.querySelector(".viz-editor-shell")!.getBoundingClientRect().toJSON()};
  });
  writeFileSync(resolve(evidence,"rects.json"),JSON.stringify(measured,null,2));
  await page.screenshot({path:resolve(evidence,"recovery.png")});
  for(const box of measured.boxes){
   const overlaps=box.left<measured.controls.right&&box.right>measured.controls.left&&box.top<measured.controls.bottom&&box.bottom>measured.controls.top;
   expect(overlaps,"native controls must not obscure recovery text or actions").toBe(false);
  }
  expect(measured.workspace.top).toBeGreaterThanOrEqual(measured.notice.bottom);
  expect(measured.workspace.bottom).toBeLessThanOrEqual(900);
  expect(measured.workspace.height).toBeGreaterThan(400);
 }finally{await browser.close();}
},15000);
