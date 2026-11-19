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
    errors: Record<string, string>;
    status?: string;
    csrf_token: string;
    passwordRules: string;
    canManagePasskeys: boolean;
    passkeys: Passkey[];
    canManageTwoFactor: boolean;
    twoFactorEnabled: boolean;
    requiresConfirmation: boolean;
    twoFactorAwaitingConfirmation: boolean;
    twoFactorSecretKey?: string;
    twoFactorQrCode?: string;
    twoFactorRecoveryCodes: string[];
};

export default function SecuritySettings({
    errors,
    status,
    csrf_token,
    passwordRules,
    canManagePasskeys,
    passkeys,
    canManageTwoFactor,
    twoFactorEnabled,
    twoFactorAwaitingConfirmation,
    twoFactorSecretKey,
    twoFactorQrCode,
    twoFactorRecoveryCodes,
}: Props) {
    const headers = { 'x-csrf-token': csrf_token };
    const [passkeyName, setPasskeyName] = useState('');
    const [registeringPasskey, setRegisteringPasskey] = useState(false);
    const [passkeyError, setPasskeyError] = useState<string>();

    const registerPasskey = async (event: FormEvent) => {
        event.preventDefault();
        setRegisteringPasskey(true);
        setPasskeyError(undefined);
        try {
            Passkeys.configure({ fetch: { headers: { 'X-CSRF-TOKEN': csrf_token } } });
            await Passkeys.register({ name: passkeyName });
            router.reload({ only: ['passkeys'] });
            setPasskeyName('');
        } catch (caught) {
            setPasskeyError(caught instanceof Error ? caught.message : 'The passkey could not be registered.');
        } finally {
            setRegisteringPasskey(false);
        }
    };

    return (
        <main>
            <Head title="Security settings" />
            <nav>
                <Link href="/dashboard">Dashboard</Link>
                <Link href="/settings/profile">Profile</Link>
                <Link href="/settings/security">Security</Link>
                <Link href="/settings/appearance">Appearance</Link>
            </nav>
            <h1>Security settings</h1>
            {status === 'password-updated' && <p>Password updated.</p>}
            <Form action="/settings/password" method="put" headers={headers} resetOnSuccess>
                {({ processing }) => (
                    <>
                        <label htmlFor="current_password">Current password</label>
                        <input id="current_password" name="current_password" type="password" required autoComplete="current-password" />
                        {errors.current_password && <p role="alert">{errors.current_password}</p>}

                        <label htmlFor="password">New password</label>
                        <input id="password" name="password" type="password" required autoComplete="new-password" passwordrules={passwordRules} />
                        {errors.password && <p role="alert">{errors.password}</p>}

                        <label htmlFor="password_confirmation">Confirm password</label>
                        <input id="password_confirmation" name="password_confirmation" type="password" required autoComplete="new-password" passwordrules={passwordRules} />
                        <button type="submit" disabled={processing}>Save password</button>
                    </>
                )}
            </Form>
            {canManageTwoFactor && (
                <section>
                    <h2>Two-factor authentication</h2>
                    {!twoFactorEnabled && !twoFactorAwaitingConfirmation && (
                        <Form action="/user/two-factor-authentication" method="post" headers={headers}>
                            <button type="submit">Enable two-factor authentication</button>
                        </Form>
                    )}
                    {twoFactorAwaitingConfirmation && (
                        <>
                            <p>Scan this QR code with your authenticator app, then enter the generated code.</p>
                            {twoFactorQrCode && <img src={`data:image/png;base64,${twoFactorQrCode}`} alt="Two-factor authentication QR code" />}
                            {twoFactorSecretKey && <p>Setup key: <code>{twoFactorSecretKey}</code></p>}
                            <Form action="/user/confirmed-two-factor-authentication" method="post" headers={headers} resetOnSuccess>
                                <label htmlFor="code">Authentication code</label>
                                <input id="code" name="code" required inputMode="numeric" autoComplete="one-time-code" />
                                {errors.code && <p role="alert">{errors.code}</p>}
                                <button type="submit">Confirm</button>
                            </Form>
                        </>
                    )}
                    {twoFactorEnabled && (
                        <>
                            <h3>Recovery codes</h3>
                            <ul>{twoFactorRecoveryCodes.map((code) => <li key={code}><code>{code}</code></li>)}</ul>
                            <Form action="/user/two-factor-recovery-codes" method="post" headers={headers}>
                                <button type="submit">Regenerate recovery codes</button>
                            </Form>
                            <Form action="/user/two-factor-authentication" method="delete" headers={headers}>
                                <button type="submit">Disable two-factor authentication</button>
                            </Form>
                        </>
                    )}
                </section>
            )}
            {canManagePasskeys && (
                <section>
                    <h2>Passkeys</h2>
                    <form onSubmit={(event) => void registerPasskey(event)}>
                        <label htmlFor="passkey-name">Passkey name</label>
                        <input id="passkey-name" value={passkeyName} onChange={(event) => setPasskeyName(event.target.value)} required />
                        <button type="submit" disabled={registeringPasskey || !Passkeys.isSupported()}>Add passkey</button>
                    </form>
                    {passkeyError && <p role="alert">{passkeyError}</p>}
                    <ul>
                        {passkeys.map((passkey) => (
                            <li key={passkey.id}>
                                <strong>{passkey.name}</strong>
                                <span>{passkey.last_used_at ? ` Last used ${new Date(passkey.last_used_at * 1000).toLocaleString()}` : ' Never used'}</span>
                                <Form action={`/user/passkeys/${passkey.id}`} method="delete" headers={headers}>
                                    <button type="submit">Delete</button>
                                </Form>
                            </li>
                        ))}
                    </ul>
                </section>
            )}
        </main>
    );
}
