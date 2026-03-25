use crate::utils::seo::{use_seo, SeoMeta};
use crate::Route;
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component(PrivacyPolicy)]
pub fn privacy_policy() -> Html {
    use_seo(SeoMeta {
        title: "Privacy Policy \u{2013} Lightfriend",
        description: "How Lightfriend handles your data. Verifiable privacy through sealed computing, open source code, and cryptographic proofs. Not promises - architecture.",
        canonical: "https://lightfriend.ai/privacy",
        og_type: "website",
    });
    html! {
        <div class="legal-content privacy-policy">
            <h1>{"Privacy Policy"}</h1>

            <section>
                <h2>{"1. Overview"}</h2>
                <p>{"Lightfriend is a verifiable system. Its privacy properties are enforced by architecture, not policy, and can be independently verified by anyone. This document describes how the system is designed to work. It is not a warranty. See Section 14."}</p>
                <p>{"For a plain-language explanation, see "}<Link<Route> to={Route::Trustless}>{"Verifiably Private"}</Link<Route>>{"."}</p>
            </section>

            <section>
                <h2>{"2. What Data Enters the System"}</h2>
                <p>{"When you use Lightfriend, the following data enters the enclave environment:"}</p>
                <ul>
                    <li>{"Phone number and email address (for account access and identification)"}</li>
                    <li>{"Profile information you provide (for AI personalization)"}</li>
                    <li>{"Messages from services you connect (WhatsApp, Signal, Telegram, email, etc.)"}</li>
                    <li>{"Authentication tokens for your connected integrations"}</li>
                    <li>{"Location coordinates if you provide them (for sunrise/sunset times)"}</li>
                    <li>{"AI-generated attention items (summaries, suggested actions)"}</li>
                    <li>{"MCP server configuration data if you set it up"}</li>
                </ul>
                <p>{"All of this data is processed and stored inside a sealed enclave. It is designed to never leave the enclave in plaintext. Data stored outside the enclave is encrypted with keys that exist only inside the enclave. You can verify this."}</p>
            </section>

            <section>
                <h2>{"3. How the System is Designed"}</h2>
                <p>{"Lightfriend runs inside an AWS Nitro Enclave - an isolated environment designed to prevent anyone, including us, from accessing data inside it. The system is designed so that:"}</p>
                <ul>
                    <li>{"The enclave is designed so that no one can log in, inspect memory, or extract data."}</li>
                    <li>{"Encryption keys are designed to exist only inside the enclave."}</li>
                    <li>{"Key management is handled by an independent service (Marlin) that is designed to release keys only to enclaves running approved, publicly auditable code."}</li>
                    <li>{"All source code is open source on GitHub with reproducible builds."}</li>
                </ul>
            </section>

            <section>
                <h2>{"4. Verification"}</h2>
                <p>{"Anyone can verify what code is running by comparing the live enclave's cryptographic attestation against our public builds. Verification tools, API endpoints, and instructions are on our "}<Link<Route> to={Route::Trustless}>{"Verifiably Private"}</Link<Route>>{" page."}</p>
            </section>

            <section>
                <h2>{"5. Third-Party Services"}</h2>
                <p>{"The system depends on third-party infrastructure. We do not control these services and are not responsible for their behavior."}</p>
                <ul>
                    <li><strong>{"AWS Nitro Enclaves: "}</strong>{"Provides the sealed environment. Isolation depends on Amazon's hardware and software."}</li>
                    <li><strong>{"Marlin: "}</strong>{"Key custody. Runs inside its own enclave. Open source."}</li>
                    <li><strong>{"Tinfoil: "}</strong>{"AI inference inside sealed environments with cryptographic attestation."}</li>
                    <li><strong>{"Twilio: "}</strong>{"SMS delivery. Message content is designed to be deleted from Twilio after delivery."}</li>
                    <li><strong>{"ElevenLabs: "}</strong>{"Voice call audio. Does not provide cryptographic privacy guarantees. This is the one area relying on trust. We intend to replace this when a verifiable alternative is available."}</li>
                </ul>
            </section>

            <section>
                <h2>{"6. AI Processing"}</h2>
                <p>{"AI processing happens inside sealed environments (enclave and Tinfoil). AI outputs are automatic and may be inaccurate, incomplete, or inappropriate. You are solely responsible for deciding whether and how to act on them. Lightfriend does not review or endorse AI-generated content."}</p>
            </section>

            <section>
                <h2>{"7. Messaging Integrations"}</h2>
                <p>{"Messages from your connected platforms enter the enclave for processing. Connection data and messages are stored encrypted inside the enclave. You are solely responsible for message content. Lightfriend acts as a technical intermediary."}</p>
            </section>

            <section>
                <h2>{"8. YouTube Integration"}</h2>
                <p>{"Uses YouTube API Services. By using this feature you agree to the "}<a href="https://www.youtube.com/t/terms" target="_blank" rel="noopener noreferrer">{"YouTube Terms of Service"}</a>{" and "}<a href="http://www.google.com/policies/privacy" target="_blank" rel="noopener noreferrer">{"Google Privacy Policy"}</a>{"."}</p>
                <ul>
                    <li>{"We store encrypted OAuth tokens to access YouTube on your behalf."}</li>
                    <li>{"With read-only scope: subscription list, video metadata, search."}</li>
                    <li>{"With extended scope: subscribe/unsubscribe, comments, likes - only when you explicitly request it."}</li>
                    <li>{"We do not store watch history, search history, or video content."}</li>
                    <li>{"Revoke access anytime via account settings or "}<a href="https://security.google.com/settings/security/permissions" target="_blank" rel="noopener noreferrer">{"Google Security Settings"}</a>{"."}</li>
                </ul>
            </section>

            <section>
                <h2>{"9. Vehicle Integration (Tesla)"}</h2>
                <p>{"THIS IS AN EXPERIMENTAL SERVICE. Commands may control real vehicle functions. YOU ACCEPT FULL RESPONSIBILITY for all commands and their consequences. Lightfriend accepts NO LIABILITY for any accidents, damage, injury, theft, or loss resulting from vehicle commands."}</p>
            </section>

            <section>
                <h2>{"10. Email, MCP, and Other Integrations"}</h2>
                <p>{"All integration credentials are stored encrypted inside the enclave. You connect services at your own risk and are responsible for your accounts with third-party services. MCP server data may leave the enclave when sent to your configured external servers. We are not liable for third-party service behavior."}</p>
            </section>

            <section>
                <h2>{"11. Your Rights"}</h2>
                <ul>
                    <li>{"Access and modify your personal data"}</li>
                    <li>{"Request account deletion"}</li>
                    <li>{"Disconnect any integration at any time"}</li>
                    <li>{"Verify the system's privacy properties yourself"}</li>
                </ul>
            </section>

            <section>
                <h2>{"12. Data Retention"}</h2>
                <p>{"Data is retained until you delete your account or legal requirements are met. Stored data is encrypted with keys that exist only inside the enclave."}</p>
            </section>

            <section>
                <h2>{"13. Legal Basis"}</h2>
                <p>{"Data is processed on the basis of contract fulfillment, your consent for AI personalization, and legitimate business interests (billing, security)."}</p>
            </section>

            <section>
                <h2>{"14. Disclaimers"}</h2>
                <p>{"This policy describes system design and intent, not warranties."}</p>
                <ul>
                    <li>{"No system is perfectly secure. There may be vulnerabilities unknown to us."}</li>
                    <li>{"Security depends on third-party infrastructure functioning correctly. We do not control these systems."}</li>
                    <li>{"AI outputs may be wrong or harmful. You use them at your own risk."}</li>
                    <li>{"We built this in good faith. We do not warrant it will prevent all unauthorized access."}</li>
                    <li>{"You can verify the system yourself. Use without verification is at your own discretion."}</li>
                    <li>{"To the fullest extent permitted by law, Lightfriend disclaims all warranties regarding the security, privacy, or integrity of your data."}</li>
                </ul>
            </section>

            <section>
                <h2>{"15. Changes"}</h2>
                <p>{"We may update this policy. Changes are visible here and in our open source repository. Continued use constitutes acceptance."}</p>
            </section>

            <section>
                <h2>{"16. Contact"}</h2>
                <p>{"rasmus@lightfriend.ai - Tampere, Finland"}</p>
            </section>
            <div class="legal-links">
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Trustless}>{"Verifiably Private"}</Link<Route>>
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
                    <li>{"AI outputs are generated automatically by third-party models and may be inaccurate, incomplete, misleading, or inappropriate. They are provided for informational purposes only."}</li>
                    <li>{"You are solely responsible for evaluating, interpreting, and deciding whether to act on any AI-generated content. Lightfriend does not review, verify, or endorse AI outputs."}</li>
                    <li>{"Lightfriend is not liable for any consequences resulting from actions you take based on AI-generated content."}</li>
                    <li>{"We reserve the right to modify, improve, or discontinue any aspect of the AI assistant features."}</li>
                </ul>

                <h3>{"Data Collection and Privacy"}</h3>
                <ul>
                    <li>{"To provide personalized assistance, personal information including your phone number and profile information enters the enclave for processing."}</li>
                    <li>{"You grant us permission to use this information to improve and personalize the AI assistant's responses."}</li>
                    <li>{"The Service is designed to process data inside sealed computing environments (AWS Nitro Enclaves). See our Privacy Policy for details on the architecture and its limitations."}</li>
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
                    <li>{"Lightfriend makes no warranties regarding accuracy, reliability, availability, or security of the Service."}</li>
                    <li>{"Lightfriend shall not be liable for any direct, indirect, incidental, special, consequential, or punitive damages arising from your use of or inability to use the Service."}</li>
                    <li>{"Lightfriend is not responsible for any actions taken by third-party services, integrations, or AI models."}</li>
                    <li>{"The Service's privacy architecture depends on third-party infrastructure (AWS Nitro Enclaves, Marlin, Tinfoil, and others). Lightfriend does not warrant the correct functioning of these systems and is not liable for failures in third-party infrastructure."}</li>
                    <li>{"Descriptions of security architecture on the Service's website and documentation describe intended system design, not warranties. No computing system is perfectly secure."}</li>
                    <li>{"You assume all risk associated with your use of the Service, any connected third-party services, and any actions taken based on AI-generated content."}</li>
                    <li>{"The Service provides verification tools so you can independently assess its properties. Use of the Service without performing verification is at your own discretion and risk."}</li>
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
                <p>{"Our Privacy Policy describes how the system is designed to handle your data. It forms an integral part of these Terms. By using the Service, you acknowledge that you have read and understood our Privacy Policy."}</p>
            </section>

            <section>
                <h2>{"17. Contact Us"}</h2>
                <p>
                    {"For questions or concerns regarding these Terms, please contact us at "}
                    <a href="mailto:rasmus@lightfriend.ai">{"rasmus@lightfriend.ai"}</a>
                </p>
            </section>
            <div class="legal-links">
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Trustless}>{"Verifiably Private"}</Link<Route>>
            </div>
        </div>

    }
}
