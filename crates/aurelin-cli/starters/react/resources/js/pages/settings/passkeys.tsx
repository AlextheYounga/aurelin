import { Form, Head, Link, router } from '@inertiajs/react';
import { Passkeys } from '@laravel/passkeys';
import { useState } from 'react';
import type { FormEvent } from 'react';

type Passkey = {
    id: number;
    name: string;
    last_used_at?: number;
    created_at: number;
};

type Props = {
    passkeys: Passkey[];
    csrf_token: string;
};

export default function PasskeySettings({ passkeys, csrf_token }: Props) {
    const [name, setName] = useState('');
    const [registering, setRegistering] = useState(false);
    const [error, setError] = useState<string>();

    const register = async (event: FormEvent) => {
        event.preventDefault();
        setRegistering(true);
        setError(undefined);
        try {
            Passkeys.configure({ fetch: { headers: { 'X-CSRF-TOKEN': csrf_token } } });
            await Passkeys.register({ name });
            router.reload({ only: ['passkeys'] });
            setName('');
        } catch (caught) {
            setError(caught instanceof Error ? caught.message : 'The passkey could not be registered.');
        } finally {
            setRegistering(false);
        }
    };

    return (
        <main>
            <Head title="Passkeys" />
            <Link href="/dashboard">Dashboard</Link>
            <h1>Passkeys</h1>
            <form onSubmit={(event) => void register(event)}>
                <label htmlFor="name">Passkey name</label>
                <input id="name" value={name} onChange={(event) => setName(event.target.value)} required />
                <button type="submit" disabled={registering || !Passkeys.isSupported()}>Add passkey</button>
            </form>
            {error && <p role="alert">{error}</p>}
            <ul>
                {passkeys.map((passkey) => (
                    <li key={passkey.id}>
                        <strong>{passkey.name}</strong>
                        <span>{passkey.last_used_at ? ` Last used ${new Date(passkey.last_used_at * 1000).toLocaleString()}` : ' Never used'}</span>
                        <Form action={`/user/passkeys/${passkey.id}`} method="delete" headers={{ 'x-csrf-token': csrf_token }}>
                            <button type="submit">Delete</button>
                        </Form>
                    </li>
                ))}
            </ul>
        </main>
    );
}
