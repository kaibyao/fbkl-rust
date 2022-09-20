// eslint-disable-next-line @typescript-eslint/no-var-requires
const path = require("path");

module.exports = {
  root: true,
  env: {
    browser: true,
    es2021: true,
    node: true,
  },
  extends: [
    "eslint:recommended",
    "plugin:react/recommended",
    "plugin:react/jsx-runtime",
    "plugin:react-hooks/recommended",
    "plugin:@typescript-eslint/recommended",
    "plugin:import/recommended",
    "plugin:import/typescript",
    "prettier",
  ],
  parser: "@typescript-eslint/parser",
  parserOptions: {
    ecmaFeatures: {
      jsx: true,
    },
    ecmaVersion: 13,
    sourceType: "module",
  },
  plugins: [
    "react",
    "@typescript-eslint",
    "graphql",
    "import",
    "sort-imports-es6-autofix",
  ],
  rules: {
    "@typescript-eslint/no-explicit-any": "error",
    // "graphql/named-operations": ["warn", { env: "apollo" }],
    // "graphql/required-fields": [
    //   "warn",
    //   { env: "apollo", requiredFields: ["id"] },
    // ],
    // "graphql/template-strings": ["error", { env: "apollo" }],
    "sort-imports-es6-autofix/sort-imports-es6": [
      "warn",
      {
        ignoreCase: false,
        ignoreMemberSort: false,
        memberSyntaxSortOrder: ["none", "all", "multiple", "single"],
      },
    ],
  },
  settings: {
    "import/extensions": [".js", ".jsx", ".tsx", ".ts"],
    "import/parsers": {
      "@typescript-eslint/parser": [".ts", ".tsx"],
    },
    "import/resolver": {
      typescript: {
        alwaysTryTypes: true, // always try to resolve types under `<root>@types` directory even it doesn't contain any source code, like `@types/unist`
        project: path.resolve(__dirname, "./tsconfig.json"),
      },
    },
    react: {
      version: "detect",
    },
  },
};
