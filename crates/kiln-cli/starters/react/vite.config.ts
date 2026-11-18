import inertia from '@inertiajs/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

export default defineConfig({
    base: '/build/',
    plugins: [inertia(), react()],
    build: {
        manifest: true,
        rollupOptions: { input: 'resources/js/app.tsx' },
    },
});
