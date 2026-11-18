import { createInertiaApp } from '@inertiajs/react';

void createInertiaApp({
    title: (title) => (title ? `${title} - Kiln` : 'Kiln'),
    progress: { color: '#b45309' },
});
