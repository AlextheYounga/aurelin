import { createInertiaApp } from '@inertiajs/vue3';
import { initializeAppearance } from './appearance';

initializeAppearance();

void createInertiaApp({
    title: (title) => (title ? `${title} - Aurelin` : 'Aurelin'),
    progress: { color: '#b45309' },
});
