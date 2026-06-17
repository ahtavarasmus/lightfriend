use crate::utils::api::Api;
use futures::future::{select, Either};
use gloo_timers::future::TimeoutFuture;
use yew::prelude::*;

/// Simple page shown after guest checkout completes
/// Shows "check email" immediately; if the user is already logged in, a
/// background auth check redirects them back home.
#[function_component(SubscriptionSuccess)]
pub fn subscription_success() -> Html {
    use_effect_with_deps(
        move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                let auth_check = Api::get("/api/auth/status").send();
                let timeout = TimeoutFuture::new(2_000);
                if let Either::Left((Ok(response), _)) =
                    select(Box::pin(auth_check), Box::pin(timeout)).await
                {
                    if response.ok() {
                        // User is logged in - redirect to home
                        if let Some(window) = web_sys::window() {
                            let _ = window.location().set_href("/?subscription=success");
                        }
                    }
                }
            });
            || ()
        },
        (),
    );

    html! {
        <div class="auth-page-shell">
            <div class="hero-background"></div>
            <div class="auth-success-card">
                <h1 style="
                    font-size: 1.9rem;
                    margin-bottom: 1rem;
                    font-weight: 750;
                ">
                    {"Welcome to Lightfriend!"}
                </h1>
                <p style="
                    font-size: 1.1rem;
                    line-height: 1.6;
                    margin-bottom: 1.5rem;
                ">
                    {"Your subscription is being set up. Check your email for a link to create your password and get started."}
                </p>
                <div class="auth-success-note">
                    <p style="
                        font-size: 0.95rem;
                        margin: 0;
                    ">
                        {"Check your inbox and spam folder for an email from Lightfriend."}
                    </p>
                </div>
                <p style="
                    color: rgba(255, 255, 255, 0.78);
                    font-size: 0.9rem;
                ">
                    {"Already have an account? "}
                    <a href="/login" style="text-decoration: none;">{"Log in here"}</a>
                </p>
            </div>
        </div>
    }
}
