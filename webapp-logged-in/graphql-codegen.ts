import type { CodegenConfig } from '@graphql-codegen/cli';

const config: CodegenConfig = {
  schema: '../graphql-generation/generated/fbkl-schema.graphql',
  documents: ['src/**/!(*.d).{ts,tsx,graphql}'],
  ignoreNoDocuments: true,
  generates: {
    './src/generated/': {
      preset: 'client',
      presetConfig: {
        fragmentMasking: false,
      },
    },
    // The client preset only emits enums as string-literal union types. This
    // companion output generates the same enums as runtime const objects so
    // they can be referenced by variant (e.g. `ContractKind.Rookie`). The
    // union types are identical, so the two are fully interchangeable.
    './src/generated/enums.ts': {
      plugins: ['typescript'],
      config: {
        onlyEnums: true,
        enumsAsConst: true,
      },
    },
  },
};

export default config;
