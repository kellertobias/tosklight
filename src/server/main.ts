import express, { Router } from "express";
import path from "path";
import { apiRouter } from "./routes";

import { pagesRouter, staticsRouter } from "./essentials";

import supervisor from './engine'

console.log(`*******************************************`);
console.log(`NODE_ENV: ${process.env.NODE_ENV}`);
console.log(`*******************************************`);

supervisor.start()
const app = express();
app.set("view engine", "ejs");

app.use("/assets", express.static(path.join(process.cwd(), "assets")));
app.use('/api', apiRouter());
app.use(staticsRouter());
app.use(pagesRouter());

app.listen(3000, () => {
  console.log(`App listening on port ${3000}!`);
});

