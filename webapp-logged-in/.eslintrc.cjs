const path = require('path');

module.exports = {
  ignorePatterns: ['generated/**/*'],
  plugins: ['mui-path-imports'],
  rules: {
    'mui-path-imports/mui-path-imports': 'error',
  },
  settings: {
    'import/resolver': {
      typescript: {
        alwaysTryTypes: true, // always try to resolve types under `<root>@types` directory even it doesn't contain any source code, like `@types/unist`
        project: path.resolve(__dirname, './tsconfig.json'),
      },
    },
  },
};
