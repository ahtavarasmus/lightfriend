use yew::prelude::*;
use crate::utils::api::Api;

/// Simple page shown after guest checkout completes
/// If user is logged in, redirects to home. Otherwise shows "check email" message.
#[function_component(SubscriptionSuccess)]
pub fn subscription_success() -> Html {
    let checking = use_state(|| true);
    let checking_clone = checking.clone();

    // Check if user is logged in via API
    use_effect_with_deps(move |_| {
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(response) = Api::get("/api/auth/status").send().await {
                if response.ok() {
                    // User is logged in - redirect to home
                    if let Some(window) = web_sys::window() {
                        let _ = window.location().set_href("/?subscription=success");
                    }
                    return;
                }
            }
            // Not logged in - show the page
            checking_clone.set(false);
        });
        || ()
    }, ());

    // Show loading while checking auth
    if *checking {
        return html! {
            <div style="min-height: 100vh; display: flex; align-items: center; justify-content: center; background: linear-gradient(135deg, #0a0a0a 0%, #1a1a2e 100%);">
                <p style="color: rgba(255, 255, 255, 0.6);">{"Loading..."}</p>
            </div>
        };
    }

    // Not logged in - show the check email page
    html! {
        <div style="min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 2rem; background: linear-gradient(135deg, #0a0a0a 0%, #1a1a2e 100%);">
            <div style="
                background: rgba(30, 30, 30, 0.9);
                border: 1px solid rgba(30, 144, 255, 0.3);
                border-radius: 16px;
                padding: 3rem;
                max-width: 500px;
                text-align: center;
                box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
            ">
                <div style="font-size: 4rem; margin-bottom: 1rem;">{"🎉"}</div>
                <h1 style="
                    color: #ffffff;
                    font-size: 1.8rem;
                    margin-bottom: 1rem;
                    font-weight: 600;
                ">
                    {"Welcome to Lightfriend!"}
                </h1>
                <p style="
                    color: rgba(255, 255, 255, 0.8);
                    font-size: 1.1rem;
                    line-height: 1.6;
                    margin-bottom: 1.5rem;
                ">
                    {"Your subscription is being set up. Check your email for a link to create your password and get started."}
                </p>
                <div style="
                    background: rgba(30, 144, 255, 0.1);
                    border: 1px solid rgba(30, 144, 255, 0.3);
                    border-radius: 8px;
                    padding: 1rem;
                    margin-bottom: 1.5rem;
                ">
                    <p style="
                        color: rgba(255, 255, 255, 0.9);
                        font-size: 0.95rem;
                        margin: 0;
                    ">
                        {"📧 Check your inbox (and spam folder) for an email from Lightfriend."}
                    </p>
                </div>
                <p style="
                    color: rgba(255, 255, 255, 0.5);
                    font-size: 0.9rem;
                ">
                    {"Already have an account? "}
                    <a href="/login" style="color: #1e90ff; text-decoration: none;">{"Log in here"}</a>
                </p>
            </div>
        </div>
    }
}
