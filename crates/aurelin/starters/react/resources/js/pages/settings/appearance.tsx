import { Head, Link } from '@inertiajs/react';
import { useEffect, useState } from 'react';
import { type Appearance, storedAppearance, updateAppearance } from '../../appearance';

export default function AppearanceSettings() {
    const [appearance, setAppearance] = useState<Appearance>('system');
    useEffect(() => setAppearance(storedAppearance()), []);
    const update = (value: Appearance) => {
        setAppearance(value);
        updateAppearance(value);
    };

    return (
        <main>
            <Head title="Appearance settings" />
            <nav>
                <Link href="/dashboard">Dashboard</Link>
                <Link href="/settings/profile">Profile</Link>
                <Link href="/settings/security">Security</Link>
                <Link href="/settings/appearance">Appearance</Link>
            </nav>
            <h1>Appearance settings</h1>
            <fieldset>
                <legend>Theme</legend>
                {(['light', 'dark', 'system'] as const).map((value) => (
                    <label key={value}>
                        <input type="radio" name="appearance" value={value} checked={appearance === value} onChange={() => update(value)} />
                        {value[0].toUpperCase() + value.slice(1)}
                    </label>
                ))}
            </fieldset>
        </main>
    );
}
