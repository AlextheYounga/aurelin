import { createInertiaApp } from '@inertiajs/react';

void createInertiaApp({
    title: (title) => (title ? `${title} - Aurelin` : 'Aurelin'),
    progress: { color: '#b45309' },
});
