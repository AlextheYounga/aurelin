use leptos::form::ActionForm;
use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    components::{A, Route, Router, Routes},
    path,
};

/// A typed server boundary shared by native SSR and the hydrated browser app.
#[server]
pub async fn application_name() -> Result<String, ServerFnError> {
    Ok("Aurelin".to_owned())
}

#[server]
pub async fn login_csrf_token() -> Result<String, ServerFnError> {
    aurelin_framework::view::leptos::csrf_token()
        .await
        .map_err(ServerFnError::new)
}

#[server]
pub async fn login(
    email: String,
    password: String,
    remember: Option<String>,
    csrf_token: String,
) -> Result<(), ServerFnError> {
    use std::net::SocketAddr;

    use aurelin_fortify::axum::{LoginAttempt, LoginOutcome, attempt_login};
    use leptos_axum::{extract, redirect};

    use crate::auth::AuthSession;

    aurelin_framework::view::leptos::validate_csrf_token(&csrf_token)
        .await
        .map_err(ServerFnError::new)?;
    let mut auth: AuthSession = extract().await?;
    let address: aurelin_framework::axum::extract::ConnectInfo<SocketAddr> = extract().await?;
    match attempt_login(
        &mut auth,
        LoginAttempt {
            email,
            password,
            remember: remember.is_some(),
            ip_address: address.0.ip(),
        },
    )
    .await
    .map_err(ServerFnError::new)?
    {
        LoginOutcome::AlreadyAuthenticated | LoginOutcome::Authenticated => {
            redirect("/dashboard");
            Ok(())
        }
        LoginOutcome::TwoFactorChallenge => {
            redirect("/two-factor-challenge");
            Ok(())
        }
        LoginOutcome::InvalidCredentials { .. } => Err(ServerFnError::new(
            "These credentials do not match our records.",
        )),
        LoginOutcome::RateLimited { retry_after } => Err(ServerFnError::new(format!(
            "Too many login attempts. Try again in {} seconds.",
            retry_after.as_secs().max(1)
        ))),
    }
}

#[server]
pub async fn register(
    name: String,
    email: String,
    password: String,
    password_confirmation: String,
    csrf_token: String,
) -> Result<(), ServerFnError> {
    use aurelin_fortify::axum::{RegistrationAttempt, RegistrationOutcome, attempt_registration};
    use leptos_axum::{extract, redirect};

    use crate::auth::AuthSession;

    aurelin_framework::view::leptos::validate_csrf_token(&csrf_token)
        .await
        .map_err(ServerFnError::new)?;
    let mut auth: AuthSession = extract().await?;
    match attempt_registration(
        &mut auth,
        RegistrationAttempt {
            name,
            email,
            password,
            password_confirmation,
        },
    )
    .await
    .map_err(ServerFnError::new)?
    {
        RegistrationOutcome::AlreadyAuthenticated | RegistrationOutcome::Created => {
            redirect("/dashboard");
            Ok(())
        }
        RegistrationOutcome::EmailAlreadyExists => {
            Err(ServerFnError::new("The email has already been taken."))
        }
        RegistrationOutcome::InvalidInput { message, .. } => Err(ServerFnError::new(message)),
    }
}

#[server]
pub async fn logout(csrf_token: String) -> Result<(), ServerFnError> {
    use aurelin_fortify::axum::logout;
    use leptos_axum::{extract, redirect};

    use crate::auth::AuthSession;

    aurelin_framework::view::leptos::validate_csrf_token(&csrf_token)
        .await
        .map_err(ServerFnError::new)?;
    let mut auth: AuthSession = extract().await?;
    logout(&mut auth).await.map_err(ServerFnError::new)?;
    redirect("/");
    Ok(())
}

#[server]
pub async fn dashboard_user() -> Result<String, ServerFnError> {
    use leptos_axum::{extract, redirect};

    use crate::auth::AuthSession;

    let auth: AuthSession = extract().await?;
    let Some(user) = auth.user else {
        redirect("/login");
        return Err(ServerFnError::new("Authentication required."));
    };
    if user.email_verified_at.is_none() {
        redirect("/email/verify");
        return Err(ServerFnError::new("Email verification required."));
    }
    Ok(user.name)
}

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="aurelin" href="/assets/app.css"/>
        <Title text="Aurelin"/>
        <Router>
            <header class="site-header">
                <A href="/">"Aurelin"</A>
                <nav aria-label="Main navigation">
                    <A href="/">"Home"</A>
                    <A href="/login">"Log in"</A>
                    <A href="/register">"Register"</A>
                </nav>
            </header>
            <main>
                <Routes fallback=NotFound>
                    <Route path=path!("") view=Welcome/>
                    <Route path=path!("login") view=Login/>
                    <Route path=path!("register") view=Register/>
                    <Route path=path!("dashboard") view=Dashboard/>
                    <Route path=path!("email/verify") view=VerifyEmail/>
                    <Route path=path!("two-factor-challenge") view=TwoFactorChallenge/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn Welcome() -> impl IntoView {
    let count = RwSignal::new(0);
    let name = Resource::new(|| (), |_| application_name());

    view! {
        <section class="card">
            <p class="eyebrow">"Full-stack Rust"</p>
            <h1>"Welcome to Aurelin"</h1>
            <Suspense fallback=|| view! { <p>"Loading application..."</p> }>
                {move || {
                    Suspend::new(async move {
                        match name.await {
                            Ok(name) => view! {
                                <p>{name} " rendered this page on the server and hydrated it with WebAssembly."</p>
                            }.into_any(),
                            Err(_) => view! { <p>"The application name is unavailable."</p> }.into_any(),
                        }
                    })
                }}
            </Suspense>
            <button type="button" on:click=move |_| *count.write() += 1>
                "Hydrated clicks: " {count}
            </button>
        </section>
    }
}

#[component]
fn Login() -> impl IntoView {
    let csrf_token = Resource::new(|| (), |_| login_csrf_token());
    let login = ServerAction::<Login>::new();
    let result = login.value();

    view! {
        <section class="card" aria-labelledby="login-heading">
            <p class="eyebrow">"Authentication"</p>
            <h1 id="login-heading">"Log in"</h1>
            {move || result.get().and_then(Result::err).map(|error| view! {
                <p role="alert">{error.to_string()}</p>
            })}
            <Suspense fallback=|| view! { <p>"Preparing secure form..."</p> }>
                {move || Suspend::new(async move {
                    match csrf_token.await {
                        Ok(csrf_token) => view! {
                            <ActionForm action=login>
                                <input type="hidden" name="csrf_token" value=csrf_token/>
                                <label for="email">"Email"</label>
                                <input id="email" name="email" type="email" autocomplete="email" required/>
                                <label for="password">"Password"</label>
                                <input id="password" name="password" type="password" autocomplete="current-password" required/>
                                <label>
                                    <input name="remember" type="checkbox" value="true"/>
                                    "Remember me"
                                </label>
                                <button type="submit">"Log in"</button>
                            </ActionForm>
                        }.into_any(),
                        Err(error) => view! {
                            <p role="alert">{"Could not prepare login: "}{error.to_string()}</p>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </section>
    }
}

#[component]
fn Register() -> impl IntoView {
    let csrf_token = Resource::new(|| (), |_| login_csrf_token());
    let register = ServerAction::<Register>::new();
    let result = register.value();

    view! {
        <section class="card" aria-labelledby="register-heading">
            <p class="eyebrow">"Authentication"</p>
            <h1 id="register-heading">"Register"</h1>
            {move || result.get().and_then(Result::err).map(|error| view! {
                <p role="alert">{error.to_string()}</p>
            })}
            <Suspense fallback=|| view! { <p>"Preparing secure form..."</p> }>
                {move || Suspend::new(async move {
                    match csrf_token.await {
                        Ok(csrf_token) => view! {
                            <ActionForm action=register>
                                <input type="hidden" name="csrf_token" value=csrf_token/>
                                <label for="name">"Name"</label>
                                <input id="name" name="name" autocomplete="name" required maxlength="255"/>
                                <label for="register-email">"Email"</label>
                                <input id="register-email" name="email" type="email" autocomplete="email" required maxlength="255"/>
                                <label for="register-password">"Password"</label>
                                <input id="register-password" name="password" type="password" autocomplete="new-password" required minlength="8"/>
                                <label for="password-confirmation">"Confirm password"</label>
                                <input id="password-confirmation" name="password_confirmation" type="password" autocomplete="new-password" required minlength="8"/>
                                <button type="submit">"Register"</button>
                            </ActionForm>
                        }.into_any(),
                        Err(error) => view! {
                            <p role="alert">{"Could not prepare registration: "}{error.to_string()}</p>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </section>
    }
}

#[component]
fn Dashboard() -> impl IntoView {
    let user = Resource::new(|| (), |_| dashboard_user());
    let csrf_token = Resource::new(|| (), |_| login_csrf_token());
    let logout = ServerAction::<Logout>::new();

    view! {
        <section class="card">
            <p class="eyebrow">"Authentication"</p>
            <h1>"Dashboard"</h1>
            <Suspense fallback=|| view! { <p>"Loading account..."</p> }>
                {move || Suspend::new(async move {
                    match user.await {
                        Ok(name) => view! { <p>"Signed in as " {name} "."</p> }.into_any(),
                        Err(error) => view! { <p role="alert">{error.to_string()}</p> }.into_any(),
                    }
                })}
            </Suspense>
            <Suspense fallback=|| ()>
                {move || Suspend::new(async move {
                    csrf_token.await.ok().map(|csrf_token| view! {
                        <ActionForm action=logout>
                            <input type="hidden" name="csrf_token" value=csrf_token/>
                            <button type="submit">"Log out"</button>
                        </ActionForm>
                    })
                })}
            </Suspense>
        </section>
    }
}

#[component]
fn VerifyEmail() -> impl IntoView {
    view! {
        <section class="card">
            <p class="eyebrow">"Authentication"</p>
            <h1>"Verify your email"</h1>
            <p>"Use the verification link sent to your email address before opening the dashboard."</p>
        </section>
    }
}

#[component]
fn TwoFactorChallenge() -> impl IntoView {
    view! {
        <section class="card">
            <p class="eyebrow">"Authentication"</p>
            <h1>"Two-factor challenge"</h1>
            <p>"Two-factor challenge input is the next Fortify adapter slice."</p>
        </section>
    }
}

#[component]
fn NotFound() -> impl IntoView {
    view! {
        <section class="card">
            <h1>"Page not found"</h1>
            <A href="/">"Return home"</A>
        </section>
    }
}
