import { Form, Head } from '@inertiajs/react';

type Props = {
    csrf_token: string;
    errors: Record<string, string>;
};

export default function ConfirmPassword({ csrf_token, errors }: Props) {
    return (
        <main>
            <Head title="Confirm password" />
            <h1>Confirm your password</h1>
            <p>This is a secure area. Confirm your password before continuing.</p>
            <Form action="/user/confirm-password" method="post" headers={{ 'x-csrf-token': csrf_token }} resetOnSuccess={['password']}>
                {({ processing }) => (
                    <>
                        <label htmlFor="password">Password</label>
                        <input id="password" name="password" type="password" required autoFocus autoComplete="current-password" />
                        {errors.password && <p role="alert">{errors.password}</p>}
                        <button type="submit" disabled={processing}>Confirm password</button>
                    </>
                )}
            </Form>
        </main>
    );
}
