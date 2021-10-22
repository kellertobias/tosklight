import { Router } from "express";
import { IViewHome } from "/shared/interfaces/ViewHome";

export default (router: Router): void => {
  router.get("/views/home", (req, res) => {
    res.json({
      timestamp: new Date(),
    } as IViewHome);
  });
};
