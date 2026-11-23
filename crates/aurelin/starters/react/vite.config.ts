import inertia from '@inertiajs/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

export default defineConfig(({ command }) => ({
    base: command === 'build' ? '/build/' : '',
    plugins: [inertia({ ssr: false }), react()],
    server: { port: 5173, strictPort: true, hmr: { port: 5173 } },
    build: {
        manifest: true,
        rollupOptions: { input: 'index.html' },
    },
}));
