import { createInertiaApp } from '@inertiajs/react';
import { createRoot, hydrateRoot } from 'react-dom/client';
import { initializeAppearance } from './appearance';
import { resolvePage } from './resolve-page';

initializeAppearance();

void createInertiaApp({
    title: (title) => (title ? `${title} - Aurelin` : 'Aurelin'),
    progress: { color: '#b45309' },
    resolve: resolvePage,
    setup({ el, App, props }) {
        const app = <App {...props} />;
        if (el.hasAttribute('data-server-rendered')) {
            hydrateRoot(el, app);
        } else {
            createRoot(el).render(app);
        }
    },
});
