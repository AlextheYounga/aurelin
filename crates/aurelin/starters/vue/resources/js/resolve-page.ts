import type { DefineComponent } from 'vue';

const pages = import.meta.glob<{ default: DefineComponent }>('./pages/**/*.vue');

export function resolvePage(name: string) {
    const page = pages[`./pages/${name}.vue`];
    if (!page) throw new Error(`Unknown Inertia page: ${name}`);
    return page().then((module) => module.default);
}
