// @ts-check

/**
 * @type {import('next').NextConfig}
 */
const nextConfig = {
  async rewrites() {
    return [
      {
        source: '/api/:path*',
        destination: 'http://localhost:9001/api/:path*',
      },
    ];
  },
};

export default nextConfig;
