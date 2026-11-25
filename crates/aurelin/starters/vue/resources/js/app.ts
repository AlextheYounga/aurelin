import { createInertiaApp } from '@inertiajs/vue3';
import { createApp, createSSRApp, h } from 'vue';
import { initializeAppearance } from './appearance';
import { resolvePage } from './resolve-page';

initializeAppearance();

void createInertiaApp({
    title: (title) => (title ? `${title} - Aurelin` : 'Aurelin'),
    progress: { color: '#b45309' },
    resolve: resolvePage,
    setup({ el, App, props, plugin }) {
        const create = el.hasAttribute('data-server-rendered') ? createSSRApp : createApp;
        create({ render: () => h(App, props) }).use(plugin).mount(el);
    },
});
