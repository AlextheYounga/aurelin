import inertia from '@inertiajs/vite';
import vue from '@vitejs/plugin-vue';
import { defineConfig } from 'vite';

export default defineConfig(({ command }) => ({
    base: command === 'build' ? '/build/' : '',
    plugins: [inertia({ ssr: false }), vue()],
    server: { port: 5173, strictPort: true, hmr: { port: 5173 } },
    build: {
        manifest: true,
        rollupOptions: { input: 'index.html' },
    },
}));
