import { createInertiaApp } from '@inertiajs/vue3';

void createInertiaApp({
    title: (title) => (title ? `${title} - Kiln` : 'Kiln'),
    progress: { color: '#b45309' },
});
