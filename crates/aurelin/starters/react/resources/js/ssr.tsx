import { createInertiaApp } from '@inertiajs/react';
import createServer from '@inertiajs/react/server';
import { renderToString } from 'react-dom/server';
import { resolvePage } from './resolve-page';

createServer((page) => createInertiaApp({
    page,
    render: renderToString,
    resolve: resolvePage,
    title: (title) => (title ? `${title} - Aurelin` : 'Aurelin'),
    setup: ({ App, props }) => <App {...props} />,
}), { host: '127.0.0.1' });
