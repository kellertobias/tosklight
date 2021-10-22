import { Router } from "express";

export default (router: Router): void => {
    router.get("/command", (req, res) => {
        res.json({});
    });

    router.get("/scene/:sceneName/go", (req, res) => {
        const sceneName = req.params.sceneName;
        res.json({sceneName});
    });
};
