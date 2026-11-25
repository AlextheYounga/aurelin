import { createInertiaApp } from '@inertiajs/vue3';
import createServer from '@inertiajs/vue3/server';
import { createSSRApp, h } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { resolvePage } from './resolve-page';

createServer((page) => createInertiaApp({
    page,
    render: renderToString,
    resolve: resolvePage,
    title: (title) => (title ? `${title} - Aurelin` : 'Aurelin'),
    setup: ({ App, props, plugin }) => createSSRApp({ render: () => h(App, props) }).use(plugin),
}), { host: '127.0.0.1' });
