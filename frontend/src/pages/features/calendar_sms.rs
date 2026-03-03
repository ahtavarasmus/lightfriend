use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(CalendarSms)]
pub fn calendar_sms() -> Html {
    use_seo(SeoMeta {
        title: "Google Calendar on Dumbphone – Calendar Reminders via SMS | Lightfriend",
        description: "Access Google Calendar from any dumbphone via SMS. Get event reminders, check your schedule, and manage appointments from Light Phone, Nokia, or any basic phone.",
        canonical: "https://lightfriend.ai/features/calendar-sms",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"Google Calendar on Dumbphone"}</h1>
                <p class="feature-subtitle">{"Get calendar reminders and check your schedule via SMS — no smartphone needed"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"How It Works"}</h2>
                    <p>{"Staying on top of your schedule shouldn't require a smartphone. With Lightfriend, you can access Google Calendar via SMS from any phone — get reminders, check upcoming events, and never miss an appointment."}</p>
                    <p>{"Lightfriend connects to your Google Calendar and sends SMS reminders before events. Text Lightfriend to ask about your schedule, and get instant replies with your upcoming events."}</p>
                </section>
                <section>
                    <h2>{"Features"}</h2>
                    <ul>
                        <li>{"Automatic SMS reminders before events"}</li>
                        <li>{"Check your daily and weekly schedule via text"}</li>
                        <li>{"Event details including time, location, and attendees"}</li>
                        <li>{"Morning daily agenda summary"}</li>
                        <li>{"Multiple calendar support"}</li>
                        <li>{"Customizable reminder timing"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Compatible Phones"}</h2>
                    <p>{"Works with Light Phone 2 & 3, Nokia flip phones, any basic phone, any flip phone — any phone that can send SMS."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Get Calendar on Your Dumbphone"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing →"}</a>
                </section>
            </div>
        </div>
    }
}
