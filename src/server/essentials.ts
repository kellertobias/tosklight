import express, { Router } from "express";
import path from "path";
import { getManifest } from "./manifest-manager";

import { IS_DEV, WEBPACK_PORT } from "./config";

export function pagesRouter(): Router {
  const router = Router();

  router.get(`/**`, async (_, res) => {
    const manifest = await getManifest();
    res.render("page.ejs", { manifest });
  });

  return router;
}

export function staticsRouter(): Router {
  const router = Router();

  if (IS_DEV) {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const { createProxyMiddleware } = require("http-proxy-middleware");
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    // All the assets are hosted by Webpack on localhost:${config.WEBPACK_PORT} (Webpack-dev-server)
    router.use(
      "/statics",
      createProxyMiddleware({
        target: `http://localhost:${WEBPACK_PORT}/`,
      }),
    );
  } else {
    const staticsPath = path.join(process.cwd(), "dist", "statics");

    // All the assets are in "statics" folder (Done by Webpack during the build phase)
    router.use("/statics", express.static(staticsPath));
  }
  return router;
}
