use yew::prelude::*;
use yew_router::prelude::*;
use crate::Route;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(PrivacyPolicy)]
pub fn privacy_policy() -> Html {
    use_seo(SeoMeta {
        title: "Privacy Policy \u{2013} Lightfriend",
        description: "How Lightfriend protects your data. AES-256 encryption, no data selling, GDPR compliant. Learn about our privacy practices for the AI dumbphone assistant.",
        canonical: "https://lightfriend.ai/privacy",
        og_type: "website",
    });
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
                    <li>{"Location coordinates (latitude/longitude) for calculating sunrise/sunset times"}</li>
                    <li>{"AI-generated attention items (attention summaries, suggested actions, priority levels)"}</li>
                    <li>{"MCP server configuration data (server URLs, authentication tokens)"}</li>
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
                    <li>{"Automatically generate attention items that flag messages and events needing your attention"}</li>
                    <li>{"Attention items include AI-generated summaries, suggested actions, reasoning, and contextual data"}</li>
                    <li>{"Attention items are stored until dismissed, actioned, or expired"}</li>
                </ul>
            </section>

            <section>
                <h2>{"6. Messaging Platform Integrations (WhatsApp, Signal, Telegram)"}</h2>
                <p>{"Lightfriend integrates with various messaging platforms to allow you to send and receive messages via SMS or voice calls. This section applies to all messaging integrations including WhatsApp, Signal, and Telegram."}</p>
                <ul>
                    <li>{"Connection Data: We store encrypted authentication data to maintain connections to your messaging accounts."}</li>
                    <li>{"Message Processing: Messages are processed and temporarily stored on our servers to enable format conversion between platforms. This is a technical necessity for service operation."}</li>
                    <li>{"Data Retention: Connection data is retained until you disconnect the service or delete your account. Messages are retained only as long as necessary for delivery."}</li>
                    <li>{"Third-Party Access: We do not share your messaging data with third parties except as required by law."}</li>
                    <li>{"Room Identifiers: We store messaging platform room identifiers linked to your contact profiles to route messages to the correct contacts."}</li>
                </ul>
                <p>{"Important Disclaimers:"}</p>
                <ul>
                    <li>{"Users are solely responsible for the content of messages sent through our service. Lightfriend acts only as a technical intermediary."}</li>
                    <li>{"We cannot guarantee privacy beyond our implemented security measures. Users should consider this when deciding what information to share."}</li>
                    <li>{"We are not responsible for any actions taken by the underlying messaging platforms."}</li>
                </ul>
            </section>

            <section>
                <h2>{"8. YouTube Integration and OAuth"}</h2>
                <p>{"Lightfriend provides an intentional YouTube viewing experience through our web dashboard, designed for users who want to access YouTube content without algorithmic recommendations, infinite scroll, or autoplay. This feature uses YouTube API Services."}</p>
                <ul>
                    <li><strong>{"YouTube API Services: "}</strong>{"This application uses YouTube API Services. By using the YouTube features, you are also agreeing to be bound by the "}<a href="https://www.youtube.com/t/terms" target="_blank" rel="noopener noreferrer">{"YouTube Terms of Service"}</a>{"."}</li>
                    <li><strong>{"Google Privacy Policy: "}</strong>{"Google's Privacy Policy applies to your use of YouTube through our service. Please review the "}<a href="http://www.google.com/policies/privacy" target="_blank" rel="noopener noreferrer">{"Google Privacy Policy"}</a>{"."}</li>
                    <li><strong>{"Data We Access (Read-Only): "}</strong>{"With your authorization via the youtube.readonly scope, we access: your YouTube subscriptions list to display recent videos from channels you follow, video metadata (titles, thumbnails, descriptions, view counts), and the ability to search YouTube on your behalf."}</li>
                    <li><strong>{"Data We Access (Extended Permissions): "}</strong>{"If you choose to enable extended permissions via the youtube.force-ssl scope, we can additionally: subscribe/unsubscribe from channels on your behalf, read and post comments, and like/dislike videos. These actions are only performed when you explicitly request them through our interface."}</li>
                    <li><strong>{"Tokens We Store: "}</strong>{"Upon your authorization, we store an encrypted access token and refresh token in our secure database. These tokens enable Lightfriend to access YouTube on your behalf when you use our dashboard."}</li>
                    <li><strong>{"What We Don't Store: "}</strong>{"We do not store your YouTube videos, watch history, search history, or any content beyond the encrypted authentication tokens necessary for the integration."}</li>
                    <li><strong>{"Usage: "}</strong>{"The stored tokens are used exclusively to authenticate with YouTube API when you actively use the YouTube features in our dashboard. We do not access your YouTube data in the background or use it to train AI models."}</li>
                    <li><strong>{"Sharing: "}</strong>{"We do not share your YouTube data or tokens with third parties, except as required by law."}</li>
                    <li><strong>{"AI-Assisted Access: "}</strong>{"Our AI assistant can search YouTube and access your subscription feed on your behalf when processing your requests, using your stored OAuth tokens."}</li>
                    <li><strong>{"Revoking Access: "}</strong>{"You can disconnect YouTube from Lightfriend at any time through your account settings. You can also revoke Lightfriend's access to your Google account at any time by visiting your "}<a href="https://security.google.com/settings/security/permissions" target="_blank" rel="noopener noreferrer">{"Google Security Settings"}</a>{". Upon revocation through Lightfriend, your tokens are deleted and revoked immediately. Upon revocation through Google Security Settings, we will delete your tokens within 30 days."}</li>
                    <li><strong>{"Contact: "}</strong>{"For questions about our YouTube integration privacy practices, contact rasmus@ahtava.com."}</li>
                </ul>
            </section>

            <section>
                <h2>{"9. Vehicle Integration (Tesla)"}</h2>
                <p>{"Lightfriend offers integration with Tesla vehicles to allow remote vehicle control via SMS or voice commands. BY USING THIS FEATURE, YOU ACKNOWLEDGE AND ACCEPT THE FOLLOWING:"}</p>
                <ul>
                    <li>{"We store encrypted OAuth tokens to access your Tesla account on your behalf."}</li>
                    <li>{"Commands you send may control real vehicle functions including but not limited to: unlocking doors, starting climate control, opening trunks, and other vehicle operations."}</li>
                    <li>{"THIS IS AN EXPERIMENTAL SERVICE. You use it entirely at your own risk."}</li>
                    <li>{"You are solely responsible for ensuring it is safe and appropriate to send any vehicle command."}</li>
                    <li>{"Lightfriend does not verify the safety, appropriateness, or consequences of any command."}</li>
                    <li>{"Lightfriend accepts NO LIABILITY whatsoever for any accidents, damage, injury, theft, loss, or any other consequences resulting from vehicle commands."}</li>
                    <li>{"By using this integration, you explicitly waive any and all claims against Lightfriend related to vehicle control."}</li>
                </ul>
            </section>

            <section>
                <h2>{"10. Email Integration (IMAP)"}</h2>
                <p>{"Lightfriend can connect to your email account to monitor and notify you of important messages."}</p>
                <ul>
                    <li>{"We store encrypted credentials (server, port, password) to access your email."}</li>
                    <li>{"Email content is accessed by our AI to judge importance and send notifications."}</li>
                    <li>{"You are responsible for the security of your email account credentials."}</li>
                    <li>{"We do not share your email data with third parties except as required by law."}</li>
                </ul>
            </section>

            <section>
                <h2>{"11. Other Third-Party Services"}</h2>
                <p>{"Lightfriend may integrate with additional third-party services not explicitly listed above. For all such integrations:"}</p>
                <ul>
                    <li>{"We store only the credentials necessary for access, encrypted at rest."}</li>
                    <li>{"You connect these services at your own risk."}</li>
                    <li>{"You are responsible for your accounts with these third-party services."}</li>
                    <li>{"We are not liable for any issues arising from third-party service failures or actions."}</li>
                    <li>{"We may modify or discontinue integrations at any time."}</li>
                </ul>
            </section>

            <section>
                <h2>{"12. MCP Server Integration"}</h2>
                <p>{"Lightfriend allows you to configure custom third-party MCP (Model Context Protocol) servers to extend your AI assistant's capabilities."}</p>
                <ul>
                    <li>{"We store encrypted MCP server URLs and authentication tokens that you provide."}</li>
                    <li>{"When the AI assistant uses MCP tools, your queries and related data may be sent to these external servers."}</li>
                    <li>{"You are responsible for the third-party MCP servers you choose to connect."}</li>
                    <li>{"Lightfriend is not liable for data handling, security practices, or any actions taken by user-configured MCP servers."}</li>
                    <li>{"You can remove MCP server configurations at any time through your account settings."}</li>
                </ul>
            </section>

            <section>
                <h2>{"13. Data Retention"}</h2>
                <p>{"We retain your data until:"}</p>
                <ul>
                    <li>{"You request account deletion"}</li>
                    <li>{"All outstanding payments are settled"}</li>
                    <li>{"Legal retention requirements are met"}</li>
                </ul>
            </section>

            <section>
                <h2>{"14. Contact Information"}</h2>
                <p>{"For privacy-related inquiries or to exercise your data rights, contact:"}</p>
                <p>{"Email: rasmus@ahtava.com"}</p>
                <p>{"Location: Tampere, Finland"}</p>
            </section>
            <div class="legal-links">
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Trustless}>{"Trustless"}</Link<Route>>
            </div>
        </div>
    }
}

#[function_component(TermsAndConditions)]
pub fn terms_and_conditions() -> Html {
    use_seo(SeoMeta {
        title: "Terms and Conditions \u{2013} Lightfriend",
        description: "Terms and conditions for using Lightfriend AI assistant service. Service terms, data handling, and user agreements.",
        canonical: "https://lightfriend.ai/terms",
        og_type: "website",
    });
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
                <h2>{"4. Billing, Payments, and Refund Policy"}</h2>
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
                <h2>{"5. Intellectual Property"}</h2>
                <p>{"All content provided on the Service, including text, graphics, logos, and software, is the property of lightfriend or its content suppliers and is protected by intellectual property laws."}</p>
            </section>

            <section>
                <h2>{"6. Termination"}</h2>
                <p>{"We reserve the right to suspend or terminate your access to the Service at our discretion, without notice, for conduct that we believe violates these Terms or is harmful to other users."}</p>
            </section>

            <section>
                <h2>{"7. Limitation of Liability"}</h2>
                <p>{"THE SERVICE IS PROVIDED \"AS IS\" AND \"AS AVAILABLE\" WITHOUT WARRANTIES OF ANY KIND, EXPRESS OR IMPLIED. TO THE FULLEST EXTENT PERMITTED BY LAW:"}</p>
                <ul>
                    <li>{"Lightfriend makes no warranties regarding accuracy, reliability, or availability of the Service."}</li>
                    <li>{"Lightfriend shall not be liable for any direct, indirect, incidental, special, consequential, or punitive damages arising from your use of or inability to use the Service."}</li>
                    <li>{"Lightfriend is not responsible for any actions taken by third-party services or integrations."}</li>
                    <li>{"You assume all risk associated with your use of the Service and any connected third-party services."}</li>
                    <li>{"In no event shall Lightfriend's total liability exceed the amount you paid for the Service in the preceding 12 months."}</li>
                </ul>
            </section>

            <section>
                <h2>{"8. Third-Party Integrations"}</h2>
                <p>{"The Service allows you to connect various third-party accounts and services. By using these integrations:"}</p>
                <ul>
                    <li>{"You are solely responsible for your accounts with third-party services."}</li>
                    <li>{"Lightfriend is not liable for any failures, outages, or issues with third-party services."}</li>
                    <li>{"You must comply with the terms of service of each third-party platform you connect."}</li>
                    <li>{"We may modify or discontinue any integration at any time without notice."}</li>
                    <li>{"Third-party services may change their APIs or terms, which may affect functionality."}</li>
                </ul>
            </section>

            <section>
                <h2>{"9. Vehicle Control Services (Tesla Integration)"}</h2>
                <p>{"THIS IS AN EXPERIMENTAL SERVICE. BY USING THE TESLA INTEGRATION, YOU EXPRESSLY ACKNOWLEDGE AND AGREE:"}</p>
                <ul>
                    <li>{"Commands sent through Lightfriend may control real vehicle functions including unlocking, climate control, and other operations."}</li>
                    <li>{"YOU ACCEPT FULL AND SOLE RESPONSIBILITY for all vehicle commands you send and their consequences."}</li>
                    <li>{"You must verify that conditions are safe before sending any vehicle command."}</li>
                    <li>{"Lightfriend does not verify the safety, appropriateness, or timing of any command."}</li>
                    <li>{"LIGHTFRIEND ACCEPTS NO LIABILITY WHATSOEVER for any accidents, vehicle damage, personal injury, death, theft, property damage, or any other consequences resulting from vehicle commands."}</li>
                    <li>{"By using this integration, you EXPLICITLY WAIVE any and all claims against Lightfriend related to vehicle control."}</li>
                    <li>{"You agree to indemnify and hold Lightfriend harmless from any claims arising from your use of vehicle control features."}</li>
                </ul>
            </section>

            <section>
                <h2>{"10. YouTube Integration"}</h2>
                <p>{"Lightfriend provides an intentional YouTube viewing experience through our web dashboard, designed for users who prefer to access YouTube without algorithmic recommendations, infinite scroll, or autoplay. By using the YouTube integration:"}</p>
                <ul>
                    <li>{"You agree to be bound by the "}<a href="https://www.youtube.com/t/terms" target="_blank" rel="noopener noreferrer">{"YouTube Terms of Service"}</a>{"."}</li>
                    <li>{"You authorize Lightfriend to access your YouTube account data as described in our Privacy Policy."}</li>
                    <li>{"If you enable extended permissions, any actions taken through our YouTube features (subscribing, commenting, rating) are performed on your behalf at your explicit request, and you accept full responsibility for them."}</li>
                    <li>{"Lightfriend is not responsible for any content on YouTube or actions taken by YouTube/Google."}</li>
                    <li>{"You may revoke access at any time through your Lightfriend account settings or your "}<a href="https://security.google.com/settings/security/permissions" target="_blank" rel="noopener noreferrer">{"Google Security Settings"}</a>{"."}</li>
                </ul>
            </section>

            <section>
                <h2>{"11. Messaging Services"}</h2>
                <p>{"The Service integrates with messaging platforms including WhatsApp, Signal, and Telegram. By using these features:"}</p>
                <ul>
                    <li>{"You are solely responsible for all message content sent through the Service."}</li>
                    <li>{"Lightfriend acts only as a technical intermediary and does not monitor or control message content."}</li>
                    <li>{"We are not liable for message delivery failures or third-party platform actions."}</li>
                    <li>{"You must comply with the terms of service of each messaging platform."}</li>
                </ul>
            </section>

            <section>
                <h2>{"12. MCP Server Integration"}</h2>
                <p>{"The Service allows you to configure custom third-party MCP (Model Context Protocol) servers. By using this feature:"}</p>
                <ul>
                    <li>{"You assume full responsibility for any third-party MCP servers you configure."}</li>
                    <li>{"Data sent to MCP servers as part of AI processing is transmitted at your own risk."}</li>
                    <li>{"Lightfriend is not liable for the behavior, data handling, availability, or security of any third-party MCP server."}</li>
                    <li>{"You must ensure that your use of third-party MCP servers complies with applicable laws and the terms of those services."}</li>
                </ul>
            </section>

            <section>
                <h2>{"13. Indemnification"}</h2>
                <p>{"You agree to indemnify, defend, and hold harmless Lightfriend, its owners, employees, and affiliates from any claims, damages, losses, or expenses (including legal fees) arising from:"}</p>
                <ul>
                    <li>{"Your use of the Service or any connected integrations."}</li>
                    <li>{"Your violation of these Terms."}</li>
                    <li>{"Your violation of any third-party rights."}</li>
                    <li>{"Any actions taken through your account or connected services."}</li>
                </ul>
            </section>

            <section>
                <h2>{"14. Changes to Terms"}</h2>
                <p>{"We may update these Terms from time to time. Continued use of the Service after any such changes constitutes your acceptance of the new Terms."}</p>
            </section>

            <section>
                <h2>{"15. Governing Law"}</h2>
                <p>{"These Terms are governed by and construed in accordance with the laws of Finland. Any disputes shall be resolved in the courts of Tampere, Finland."}</p>
            </section>

            <section>
                <h2>{"16. Data Protection and Privacy"}</h2>
                <p>{"Your privacy and personal data are protected under our Privacy Policy, which forms an integral part of these Terms. By using the Service, you acknowledge that you have read and understood our Privacy Policy and consent to the collection and processing of your personal data as described therein."}</p>
            </section>

            <section>
                <h2>{"17. Contact Us"}</h2>
                <p>
                    {"For questions or concerns regarding these Terms, please contact us at "}
                    <a href="mailto:rasmus@ahtava.com">{"rasmus@ahtava.com"}</a>
                </p>
            </section>
            <div class="legal-links">
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Trustless}>{"Trustless"}</Link<Route>>
            </div>
        </div>

    }
}
