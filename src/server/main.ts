import express, { Router } from "express";
import path from "path";
import { apiRouter } from "./routes";

import * as config from "./config";
import { pagesRouter, staticsRouter } from "./essentials";

import supervisor from './engine'

console.log(`*******************************************`);
console.log(`NODE_ENV: ${process.env.NODE_ENV}`);
console.log(`config: ${JSON.stringify(config, null, 2)}`);
console.log(`*******************************************`);

supervisor.start()
const app = express();
app.set("view engine", "ejs");

app.use("/assets", express.static(path.join(process.cwd(), "assets")));
app.use('/api', apiRouter());
app.use(staticsRouter());
app.use(pagesRouter());

app.listen(config.SERVER_PORT, () => {
  console.log(`App listening on port ${config.SERVER_PORT}!`);
});

