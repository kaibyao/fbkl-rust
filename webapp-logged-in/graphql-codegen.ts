import type { CodegenConfig } from '@graphql-codegen/cli';

const config: CodegenConfig = {
  schema: '../graphql-generation/generated/fbkl-schema.graphql',
  documents: ['app/**/!(*.d).{ts,tsx,graphql}'],
  ignoreNoDocuments: true,
  generates: {
    './generated/': {
      preset: 'client',
    },
  },
};

export default config;
