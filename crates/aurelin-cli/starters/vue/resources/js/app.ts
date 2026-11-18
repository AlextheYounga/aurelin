import { createInertiaApp } from '@inertiajs/vue3';

void createInertiaApp({
    title: (title) => (title ? `${title} - Aurelin` : 'Aurelin'),
    progress: { color: '#b45309' },
});
