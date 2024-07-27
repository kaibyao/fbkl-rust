// @ts-check

/**
 * @type {import('next').NextConfig}
 */
const nextConfig = {
  async rewrites() {
    return [
      {
        source: '/api',
        destination: 'http://localhost:9001/api',
      },
    ];
  },
};

export default nextConfig;
