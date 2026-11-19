import { Form, Head } from '@inertiajs/react';
import { useState } from 'react';

type Props = {
    csrf_token: string;
    errors: Record<string, string>;
};

export default function TwoFactorChallenge({ csrf_token, errors }: Props) {
    const [useRecoveryCode, setUseRecoveryCode] = useState(false);

    return (
        <main>
            <Head title="Two-factor authentication" />
            <h1>Two-factor authentication</h1>
            <p>
                {useRecoveryCode
                    ? 'Enter one of your emergency recovery codes.'
                    : 'Enter the authentication code from your authenticator app.'}
            </p>
            <Form action="/two-factor-challenge" method="post" headers={{ 'x-csrf-token': csrf_token }} resetOnSuccess>
                {({ processing }) => (
                    <>
                        {useRecoveryCode ? (
                            <>
                                <label htmlFor="recovery_code">Recovery code</label>
                                <input id="recovery_code" name="recovery_code" required autoFocus autoComplete="one-time-code" />
                            </>
                        ) : (
                            <>
                                <label htmlFor="code">Authentication code</label>
                                <input id="code" name="code" inputMode="numeric" required autoFocus autoComplete="one-time-code" />
                            </>
                        )}
                        {errors.code && <p role="alert">{errors.code}</p>}
                        <button type="submit" disabled={processing}>Continue</button>
                    </>
                )}
            </Form>
            <button type="button" onClick={() => setUseRecoveryCode((current) => !current)}>
                {useRecoveryCode ? 'Use an authentication code' : 'Use a recovery code'}
            </button>
        </main>
    );
}
