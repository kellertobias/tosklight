import bodyParser from "body-parser";
import { Router } from "express";

import getViewDataRoutes from './views'
import getCommandRoutes from './commands'

export function apiRouter(): Router {
  const router = Router();
  router.use(bodyParser.json());
  getViewDataRoutes(router)
  getCommandRoutes(router)
  return router;
}
