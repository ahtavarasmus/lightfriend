use yew::prelude::*;
use yew_router::prelude::*;
use crate::Route;
use serde_json::json;



use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn openCheckout(items: JsValue);
}

#[function_component(CheckoutButton)]
fn checkout_button() -> Html {
    let onclick = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        let items = json!({"items": [{
            "priceId": "pri_01jmqk1r39nk4h7bbr10jbatsz",
            "quantity": 1
        }]});
        openCheckout(serde_wasm_bindgen::to_value(&items).unwrap());
    });

    html! {
        <a href="#" {onclick}><b>{"Sign up now"}</b></a>
    }
}


#[function_component(Pricing)]
pub fn pricing() -> Html {
    html! {
        <div class="pricing-container">
            <div class="pricing-header">
                <h1>{"Simple, Usage-Based Pricing"}</h1>
                <p>{"Pay only for what you use. No subscriptions, no commitments."}</p>
            </div>

            <div class="pricing-grid">
                <div class="pricing-card main">
                    <div class="card-header">
                        <h3>{"Voice Calls"}</h3>
                        <div class="price">
                            <span class="amount">{"€0.20"}</span>
                            <span class="period">{"/minute"}</span>
                        </div>
                    </div>
                    <ul>
                        <li>{"High-quality AI assistant voice calls"}</li>
                        <li>{"All the tools available"}</li>
                        <li>{"No logging, fully private"}</li>
                        <li>{"24/7 availability"}</li>
                    </ul>
                </div>

                <div class="pricing-card main">
                    <div class="card-header">
                        <h3>{"SMS Messages"}</h3>
                        <div class="price">
                            <span class="amount">{"€0.20"}</span>
                            <span class="period">{"/message"}</span>
                        </div>
                    </div>
                    <ul>
                        <li>{"AI assistant chat responses"}</li>
                        <li>{"All the tools available"}</li>
                        <li>{"Message history, only on your device locally"}</li>
                        <li>{"24/7 availability"}</li>
                    </ul>
                </div>

                <div class="pricing-card features">
                    <div class="card-header">
                        <h3>{"Included Features"}</h3>
                    </div>
                    <ul>
                        <li>{"Smart AI Assistant"}</li>
                        <li>{"Calendar Integration"}</li>
                        <li>{"WhatsApp Integration"}</li>
                        <li>{"Telegram Integration"}</li>
                        <li>{"Email Access"}</li>
                        <li>{"Perplexity Search"}</li>
                        <li>{"24/7 Availability"}</li>
                    </ul>
                </div>
                </div>
                <CheckoutButton />

            <div class="pricing-faq">
                <h2>{"Common Questions"}</h2>
                <div class="faq-grid">
                    <div class="faq-item">
                        <h3>{"How does billing work?"}</h3>
                        <p>{"You'll be billed monthly based on your actual usage of voice calls and SMS messages. No minimum fees or hidden charges."}</p>
                    </div>
                    <div class="faq-item">
                        <h3>{"Are there any hidden fees?"}</h3>
                        <p>{"No hidden fees. You only pay for the calls and messages you make. All features are included at no extra cost."}</p>
                    </div>
                    <div class="faq-item">
                        <h3>{"What about refunds?"}</h3>
                        <p>{"Due to the pay-as-you-go nature of our service, we don't offer refunds. You're only charged for services you actually use."}</p>
                    </div>
                </div>
            </div>

            <div class="legal-links">
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
            </div>
        </div>
    }
}

#[function_component(PrivacyPolicy)]
pub fn privacy_policy() -> Html {
    html! {
        <div class="legal-content privacy-policy">
            <h1>{"Privacy Policy"}</h1>

            <section>
                <h2>{"1. Data Collection and Processing"}</h2>
                <p>{"We collect and process the following personal data:"}</p>
                <ul>
                    <li>{"Phone number (for service access and user identification)"}</li>
                    <li>{"Email address (for account recovery and identification)"}</li>
                    <li>{"User-provided profile information (for AI assistant personalization)"}</li>
                </ul>
            </section>

            <section>
                <h2>{"2. Legal Basis for Processing"}</h2>
                <p>{"We process your data based on:"}</p>
                <ul>
                    <li>{"Contract fulfillment (service provision)"}</li>
                    <li>{"Your explicit consent for AI personalization"}</li>
                    <li>{"Legitimate business interests (billing and security)"}</li>
                </ul>
            </section>

            <section>
                <h2>{"3. Data Security Measures"}</h2>
                <ul>
                    <li>{"Secure server access limited to authorized personnel"}</li>
                    <li>{"Password hashing for secure storage"}</li>
                    <li>{"HTTPS encryption for all data transmission"}</li>
                    <li>{"Protected API endpoints with secret keys"}</li>
                    <li>{"Segregated user data access"}</li>
                </ul>
            </section>

            <section>
                <h2>{"4. Your Data Rights"}</h2>
                <p>{"You have the right to:"}</p>
                <ul>
                    <li>{"Access your personal data"}</li>
                    <li>{"Modify your phone number, email, nickname, and profile information"}</li>
                    <li>{"Request account deletion (subject to outstanding payments)"}</li>
                </ul>
            </section>

            <section>
                <h2>{"5. AI Processing"}</h2>
                <p>{"Our AI assistant processes your provided information to:"}</p>
                <ul>
                    <li>{"Personalize responses based on your profile information"}</li>
                    <li>{"Provide context-aware assistance during calls"}</li>
                    <li>{"Improve service quality"}</li>
                </ul>
            </section>

            <section>
                <h2>{"6. Data Retention"}</h2>
                <p>{"We retain your data until:"}</p>
                <ul>
                    <li>{"You request account deletion"}</li>
                    <li>{"All outstanding payments are settled"}</li>
                    <li>{"Legal retention requirements are met"}</li>
                </ul>
            </section>

            <section>
                <h2>{"7. Contact Information"}</h2>
                <p>{"For privacy-related inquiries or to exercise your data rights, contact:"}</p>
                <p>{"Email: rasmus@ahtava.com"}</p>
                <p>{"Location: Tampere, Finland"}</p>
            </section>
            <div class="legal-links">
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
            </div>
        </div>
    }
}

#[function_component(TermsAndConditions)]
pub fn terms_and_conditions() -> Html {
    html! {
        <div class="legal-content terms-and-conditions">
            <h1>{"lightfriend Terms and Conditions"}</h1>
            <p class="company-name">{"Provided by Rasmus Multiverse"}</p>

            <section>
                <h2>{"1. Introduction"}</h2>
                <p>{"These Terms and Conditions (\"Terms\") govern your use of the lightfriend platform (\"Service\"). By accessing or using our Service, you agree to comply with and be bound by these Terms."}</p>
            </section>

            <section>
                <h2>{"2. User Accounts"}</h2>
                <p>{"To access certain features, you must create an account. You are responsible for maintaining the confidentiality of your account information and for all activities that occur under your account. lightfriend never has access to your passwords. Please do not provide your password to anyone claiming to be a representative of lightfriend, or any third party."}</p>
            </section>


            <section>
                <h2>{"3. Acceptable Use"}</h2>
                <p>{"You agree not to use the Service for any unlawful purpose or in any way that could harm the Service or impair anyone else's use of it."}</p>
            </section>

            <section>
                <h2>{"3. Billing, Payments, and Refund Policy"}</h2>
                <h3>{"Usage-Based Billing and No Refunds Policy"}</h3>
                <ul>
                    <li>{"The Service operates on a usage-based billing model where charges are calculated based on your monthly usage of calling and texting features."}</li>
                    <li>{"Usage is measured and billed monthly based on actual consumption of the Service's features."}</li>
                    <li>{"You will be automatically billed on a monthly basis for the services consumed."}</li>
                    <li>{"Due to the nature of our service and its operational costs, we do not offer refunds. You are only charged for the services you actually use."}</li>
                    <li>{"Detailed usage information and billing history can be accessed through your account profile billing section."}</li>
                </ul>

                <h3>{"AI Assistant Services"}</h3>
                <ul>
                    <li>{"The Service provides AI-powered assistance based on the information you provide during registration and subsequent interactions."}</li>
                    <li>{"While we strive to provide accurate and helpful assistance, the Service makes no guarantees about the accuracy or reliability of AI-generated responses."}</li>
                    <li>{"We reserve the right to modify, improve, or discontinue any aspect of the AI assistant features."}</li>
                </ul>

                <h3>{"Data Collection and Privacy"}</h3>
                <ul>
                    <li>{"To provide personalized assistance, we collect and process personal information including your phone number and profile information."}</li>
                    <li>{"You grant us permission to use this information to improve and personalize the AI assistant's responses."}</li>
                    <li>{"We implement appropriate security measures to protect your personal information."}</li>
                </ul>

                <h3>{"Service Modifications"}</h3>
                <ul>
                    <li>{"We may modify the Service's features, pricing structure, or billing methods with reasonable notice."}</li>
                    <li>{"Changes to pricing or fundamental service features will be communicated to users in advance."}</li>
                    <li>{"Continued use of the Service after such changes constitutes acceptance of the modifications."}</li>
                </ul>

                <h3>{"Account Termination and Data"}</h3>
                <ul>
                    <li>{"Upon account termination, you will be billed for any usage up to the termination date."}</li>
                    <li>{"We may retain certain data as required by law or for legitimate business purposes."}</li>
                    <li>{"You can request export or deletion of your personal data in accordance with applicable privacy laws."}</li>
                </ul>
            </section>

            <section>
                <h2>{"4. Intellectual Property"}</h2>
                <p>{"All content provided on the Service, including text, graphics, logos, and software, is the property of lightfriend or its content suppliers and is protected by intellectual property laws."}</p>
            </section>

            <section>
                <h2>{"5. Termination"}</h2>
                <p>{"We reserve the right to suspend or terminate your access to the Service at our discretion, without notice, for conduct that we believe violates these Terms or is harmful to other users."}</p>
            </section>

            <section>
                <h2>{"6. Limitation of Liability"}</h2>
                <p>{"The Service is provided \"as is\" without warranties of any kind. lightfriend will not be liable for any damages arising from the use or inability to use the Service."}</p>
            </section>

            <section>
                <h2>{"7. Changes to Terms"}</h2>
                <p>{"We may update these Terms from time to time. Continued use of the Service after any such changes constitutes your acceptance of the new Terms."}</p>
            </section>

            <section>
                <h2>{"8. Governing Law"}</h2>
                <p>{"These Terms are governed by and construed in accordance with the laws of the jurisdiction in which lightfriend operates, in Tampere, Finland."}</p>
            </section>

            <section>
                <h2>{"9. Service Usage Policy"}</h2>
                <p>{"By using our platform, you agree to use the AI-powered voice and text assistance services responsibly. lightfriend is designed to provide smart tools for dumbphone users, including calendar access, email integration, messaging services, and Perplexity search capabilities. Users are responsible for using these features in accordance with applicable laws and regulations. The service should not be used for any malicious or harmful purposes that could compromise the platform's integrity or other users' experience."}</p>
            </section>
<section>
                <h2>{"Data Protection and Privacy"}</h2>
                <p>{"Your privacy and personal data are protected under our Privacy Policy, which forms an integral part of these Terms. By using the Service, you acknowledge that you have read and understood our Privacy Policy and consent to the collection and processing of your personal data as described therein."}</p>
            </section>

            <section>
                <h2>{"10. Contact Us"}</h2>
                <p>
                    {"For questions or concerns regarding these Terms, please contact us at "}
                    <a href="mailto:rasmus@ahtava.com">{"rasmus@ahtava.com"}</a>
                </p>
            </section>
            <div class="legal-links">
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
            </div>
        </div>

    }
}
