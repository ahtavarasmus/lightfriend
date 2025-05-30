use yew::prelude::*;
use yew_router::prelude::*;
use crate::Route;



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
                    <li>{"Access tokens used to access the integrations you setup"}</li>
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
                    <li>{"Access tokens and other sensitive data are encrypted at rest"}</li>
                    <li>{"Segregated user data access"}</li>
                </ul>
            </section>

            <section>
                <h2>{"4. Your Data Rights"}</h2>
                <p>{"You have the right to:"}</p>
                <ul>
                    <li>{"Access your personal data"}</li>
                    <li>{"Modify your phone number, email, nickname, profile information and service connections"}</li>
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
                <h2>{"6. Google Calendar Integration and OAuth"}</h2>
                <p>{"Lightfriend integrates with Google Calendar to allow you to manage your calendar via SMS or voice call commands. To provide this functionality, we use Google OAuth with the following details:"}</p>
                <ul>
                    <li>{"Data We Access: We request access to your Google Calendar data only when you authorize it through OAuth."}</li>
                    <li>{"Tokens We Store: Upon your authorization, we store an access token and a refresh token in our secure database. These tokens enable Lightfriend to access your Google Calendar on your behalf when you use our SMS or voice call features. We encrypt these tokens to protect your data."}</li>
                    <li>{"What We Don't Store: We do not store any additional Google account data, such as your calendar events, email, or personal details—only the encrypted tokens necessary for calendar access are retained."}</li>
                    <li>{"Usage: The stored tokens are used exclusively to authenticate and access your Google Calendar when you request actions (e.g., checking or updating your schedule). Lightfriend does not retain user data obtained through Workspace APIs to develop, improve, or train generalized AI and/or ML models."}</li>
                    <li>{"Sharing: We do not share these tokens or your Google user data with third parties, except as required by law or to facilitate the Google API services you've authorized."}</li>
                </ul>
            </section>

            <section>
                <h2>{"7. WhatsApp Integration"}</h2>
                <p>{"Lightfriend integrates with WhatsApp to allow you to send and receive WhatsApp messages via SMS or voice calls. Due to the technical requirements of providing this service, we must process and temporarily store certain WhatsApp data. Here's how we handle your WhatsApp data:"}</p>
                <ul>
                    <li>{"Connection Data: When you connect your WhatsApp account, we store necessary authentication data to maintain the connection between our service and your WhatsApp account."}</li>
                    <li>{"Message Encryption in Transit: All WhatsApp messages are protected with end-to-end encryption during transmission between WhatsApp's API and our servers. The decryption process occurs securely on our servers using securely stored keys, which is necessary for converting messages to SMS and voice calls."}</li>
                    <li>{"Message Processing: To enable the conversion of WhatsApp messages to SMS and voice calls, messages must be processed and temporarily stored on our secure servers. This storage is a technical necessity to ensure reliable message delivery and conversion between different communication formats."}</li>
                    <li>{"Message Storage Duration: Messages are retained only for the minimum time necessary to ensure reliable delivery and system functionality. We automatically remove messages based on our retention policy."}</li>
                    <li>{"Security Measures: Access to message data is strictly limited to only essential system processes required for service operation. Our servers are secured with industry-standard practices and access controls."}</li>
                    <li>{"Data Retention: WhatsApp connection data is retained until you disconnect the service or delete your account."}</li>
                    <li>{"Third-Party Access: We do not share your WhatsApp data with any third parties except as required by law."}</li>
                </ul>
                <p>{"Important Disclaimers:"}</p>
                <ul>
                    <li>{"Message Content Responsibility: Users are solely responsible for the content of messages sent through our service. Lightfriend acts solely as a technical intermediary for message delivery and does not monitor, edit, or take responsibility for user-generated content."}</li>
                    <li>{"Technical Requirements: The processing and temporary storage of messages is a technical requirement necessary to provide the service. By using our WhatsApp integration, you acknowledge and accept these technical requirements."}</li>
                    <li>{"Service Limitations: Due to the technical nature of the WhatsApp bridge, we cannot guarantee the privacy of messages beyond our implemented security measures. Users should consider this when deciding what information to share through the service."}</li>
                </ul>
            </section>

            <section>
                <h2>{"8. Google Tasks Integration and OAuth"}</h2>
                <p>{"Lightfriend integrates with Google Tasks to allow you to manage your tasks via SMS or voice call commands. To provide this functionality, we use Google OAuth with the following details:"}</p>
                <ul>
                    <li>{"Data We Access: We request access to your Google Tasks data only when you authorize it through OAuth."}</li>
                    <li>{"Tokens We Store: Upon your authorization, we store an access token and a refresh token in our secure database. These tokens enable Lightfriend to access your Google Tasks on your behalf when you use our SMS or voice call features. We encrypt these tokens to protect your data."}</li>
                    <li>{"What We Don't Store: We do not store any additional Google account data, such as your calendar events, email, or personal details—only the encrypted tokens necessary for tasks access are retained."}</li>
                    <li>{"Usage: The stored tokens are used exclusively to authenticate and access your Google Tasks when you request actions (e.g., checking or sending your tasks messages). Lightfriend does not retain user data obtained through Workspace APIs to develop, improve, or train generalized AI and/or ML models."}</li>
                    <li>{"Sharing: We do not share these tokens or your Google user data with third parties, except as required by law or to facilitate the Google API services you've authorized."}</li>
                </ul>
            </section>

            <section>
                <h2>{"8. Data Retention"}</h2>
                <p>{"We retain your data until:"}</p>
                <ul>
                    <li>{"You request account deletion"}</li>
                    <li>{"All outstanding payments are settled"}</li>
                    <li>{"Legal retention requirements are met"}</li>
                </ul>
            </section>

            <section>
                <h2>{"9. Contact Information"}</h2>
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
                <h3>{"Prepaid Credits and No Refunds Policy"}</h3>
                <ul>
                    <li>{"The Service operates on a prepaid credit model where you purchase credits in advance to use for calling and texting features."}</li>
                    <li>{"Usage is deducted from your credit balance based on your actual consumption of the Service's features."}</li>
                    <li>{"You can optionally enable automatic top-up to add more credits to your account when your balance runs low, ensuring uninterrupted service."}</li>
                    <li>{"Due to the nature of our service and its operational costs, we do not offer refunds. Credits purchased are non-refundable and can only be used for services consumed."}</li>
                    <li>{"Detailed usage information and credit history can be accessed through your account profile billing section."}</li>
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
