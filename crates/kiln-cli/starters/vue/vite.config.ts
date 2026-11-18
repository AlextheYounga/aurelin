import inertia from '@inertiajs/vite';
import vue from '@vitejs/plugin-vue';
import { defineConfig } from 'vite';

export default defineConfig({
    base: '/build/',
    plugins: [inertia(), vue()],
    build: {
        manifest: true,
        rollupOptions: { input: 'resources/js/app.ts' },
    },
});
