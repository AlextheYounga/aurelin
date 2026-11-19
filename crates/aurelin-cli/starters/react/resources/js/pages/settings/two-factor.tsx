import { Form, Head, Link } from '@inertiajs/react';

type Props = {
    enabled: boolean;
    awaiting_confirmation: boolean;
    secret_key?: string;
    qr_code?: string;
    recovery_codes: string[];
    errors: Record<string, string>;
    csrf_token: string;
};

export default function TwoFactorSettings(props: Props) {
    const headers = { 'x-csrf-token': props.csrf_token };

    return (
        <main>
            <Head title="Two-factor authentication" />
            <Link href="/dashboard">Dashboard</Link>
            <h1>Two-factor authentication</h1>
            {!props.enabled && !props.awaiting_confirmation && (
                <Form action="/user/two-factor-authentication" method="post" headers={headers}>
                    <button type="submit">Enable two-factor authentication</button>
                </Form>
            )}
            {props.awaiting_confirmation && (
                <section>
                    <h2>Finish enabling two-factor authentication</h2>
                    <p>Scan this QR code with your authenticator app, then enter the generated code.</p>
                    {props.qr_code && <img src={`data:image/png;base64,${props.qr_code}`} alt="Two-factor authentication QR code" />}
                    {props.secret_key && <p>Setup key: <code>{props.secret_key}</code></p>}
                    <Form action="/user/confirmed-two-factor-authentication" method="post" headers={headers} resetOnSuccess>
                        <label htmlFor="code">Authentication code</label>
                        <input id="code" name="code" required inputMode="numeric" autoComplete="one-time-code" />
                        {props.errors.code && <p role="alert">{props.errors.code}</p>}
                        <button type="submit">Confirm</button>
                    </Form>
                </section>
            )}
            {props.enabled && (
                <section>
                    <h2>Recovery codes</h2>
                    <ul>{props.recovery_codes.map((code) => <li key={code}><code>{code}</code></li>)}</ul>
                    <Form action="/user/two-factor-recovery-codes" method="post" headers={headers}>
                        <button type="submit">Regenerate recovery codes</button>
                    </Form>
                    <Form action="/user/two-factor-authentication" method="delete" headers={headers}>
                        <button type="submit">Disable two-factor authentication</button>
                    </Form>
                </section>
            )}
        </main>
    );
}
