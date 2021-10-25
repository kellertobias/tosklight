const custom = require('../webpack.config.js')
module.exports = {
  "stories": [
    "../src/client/**/*.stories.tsx",
    "../src/client/*.stories.tsx",
  ],
  "addons": [
    "@storybook/addon-links",
    "@storybook/addon-essentials",
    'storybook-addon-outline',
    'storybook-dark-mode'
  ],
  "core": {
    "builder": "webpack5"
  },
  webpackFinal: (config) => {
    return {
      ...config,
      module: {
         ...config.module,
         rules: custom.module.rules
      }
    };
  },
}