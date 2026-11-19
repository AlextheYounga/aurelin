import { Form, Head, Link } from '@inertiajs/react';
import { Passkeys } from '@laravel/passkeys';
import { useState } from 'react';

type Props = {
    csrf_token: string;
    errors: Record<string, string>;
    status?: string;
};

export default function Login({ csrf_token, errors, status }: Props) {
    const [passkeyError, setPasskeyError] = useState<string>();
    const [verifying, setVerifying] = useState(false);

    const verifyPasskey = async () => {
        setVerifying(true);
        setPasskeyError(undefined);
        try {
            Passkeys.configure({ fetch: { headers: { 'X-CSRF-TOKEN': csrf_token } } });
            const response = await Passkeys.verify();
            window.location.href = response.redirect ?? '/dashboard';
        } catch (error) {
            setPasskeyError(error instanceof Error ? error.message : 'The passkey could not be verified.');
            setVerifying(false);
        }
    };

    return (
        <main>
            <Head title="Log in" />
            <h1>Log in to your account</h1>
            {status && <p>{status}</p>}
            <Form action="/login" method="post" headers={{ 'x-csrf-token': csrf_token }} resetOnSuccess={['password']}>
                {({ processing }) => (
                    <>
                        <label htmlFor="email">Email address</label>
                        <input id="email" name="email" type="email" required autoFocus autoComplete="email webauthn" />
                        {errors.email && <p role="alert">{errors.email}</p>}

                        <label htmlFor="password">Password</label>
                        <Link href="/forgot-password">Forgot your password?</Link>
                        <input id="password" name="password" type="password" required autoComplete="current-password" />
                        {errors.password && <p role="alert">{errors.password}</p>}

                        <button type="submit" disabled={processing}>Log in</button>
                    </>
                )}
            </Form>
            {Passkeys.isSupported() && (
                <button type="button" disabled={verifying} onClick={() => void verifyPasskey()}>
                    Log in with a passkey
                </button>
            )}
            {passkeyError && <p role="alert">{passkeyError}</p>}
            <p>Don't have an account? <Link href="/register">Sign up</Link></p>
        </main>
    );
}
