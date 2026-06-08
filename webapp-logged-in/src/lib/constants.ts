// API requests are issued same-origin and proxied to the Rust backend
// (Vite dev server proxy in development; reverse-proxied in production), so the
// browser attaches the `fbkl_id` auth cookie automatically. Keep paths relative.
export const BASE_API_URL = '';
