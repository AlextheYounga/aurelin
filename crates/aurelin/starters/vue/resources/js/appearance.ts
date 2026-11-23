export type Appearance = 'light' | 'dark' | 'system';

export function storedAppearance(): Appearance {
    const stored = localStorage.getItem('appearance');
    return stored === 'light' || stored === 'dark' ? stored : 'system';
}

export function applyAppearance(appearance: Appearance): void {
    const dark = appearance === 'dark'
        || (appearance === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
    document.documentElement.classList.toggle('dark', dark);
    document.documentElement.style.colorScheme = dark ? 'dark' : 'light';
}

export function updateAppearance(appearance: Appearance): void {
    localStorage.setItem('appearance', appearance);
    document.cookie = `appearance=${appearance};path=/;max-age=31536000;SameSite=Lax`;
    applyAppearance(appearance);
}

export function initializeAppearance(): void {
    applyAppearance(storedAppearance());
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
        if (storedAppearance() === 'system') {
            applyAppearance('system');
        }
    });
}
