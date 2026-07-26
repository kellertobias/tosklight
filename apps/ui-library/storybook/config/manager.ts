import { addons } from "storybook/manager-api";
import { create } from "storybook/theming";

addons.setConfig({
  theme: create({
    base: "dark",
    brandTitle: "ToskLight UI",
    appBg: "#07090c",
    appContentBg: "#07090c",
    barBg: "#0e1217",
    inputBg: "#141920",
    colorPrimary: "#1bd6ec",
    colorSecondary: "#1bd6ec",
  }),
});
