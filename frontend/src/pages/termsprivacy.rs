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
            <p class="last-updated">{"Last updated: April 12, 2026"}</p>

            <section>
                <h2>{"1. Overview"}</h2>
                <p>{"Lightfriend is a verifiable system. Its privacy properties are enforced by architecture, not policy, and can be independently verified by anyone. This document describes how the system is designed to work. It is not a warranty. See Section 16."}</p>
                <p>{"For a plain-language explanation, see "}<Link<Route> to={Route::Trustless}>{"Verifiably Private"}</Link<Route>>{"."}</p>
            </section>

            <section>
                <h2>{"2. Data Controller"}</h2>
                <p>{"The data controller is Rasmus Multiverse (sole proprietorship of Rasmus \u{00C4}ht\u{00E4}v\u{00E4}), Tampere, Finland."}</p>
                <p>{"Contact: "}<a href="mailto:rasmus@lightfriend.ai">{"rasmus@lightfriend.ai"}</a></p>
                <p>{"Given the current scale of operations, we have not appointed a Data Protection Officer. You may direct all data protection inquiries to the contact address above."}</p>
                <p>{"You have the right to lodge a complaint with the Finnish Data Protection Ombudsman ("}<a href="https://tietosuoja.fi/en" target="_blank" rel="noopener noreferrer">{"tietosuoja.fi"}</a>{") or your local EU/EEA supervisory authority."}</p>
            </section>

            <section>
                <h2>{"3. What Data Enters the System"}</h2>
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
                <h2>{"4. How the System is Designed"}</h2>
                <p>{"Lightfriend runs inside an AWS Nitro Enclave - an isolated environment designed to prevent anyone, including us, from accessing data inside it. The system is designed so that:"}</p>
                <ul>
                    <li>{"The enclave is designed so that no one can log in, inspect memory, or extract data."}</li>
                    <li>{"Encryption keys are designed to exist only inside the enclave."}</li>
                    <li>{"Key management is handled by an independent service (Marlin) that is designed to release keys only to enclaves running approved, publicly auditable code."}</li>
                    <li>{"All source code is open source on GitHub with reproducible builds."}</li>
                </ul>
            </section>

            <section>
                <h2>{"5. Verification"}</h2>
                <p>{"Anyone can verify what code is running by comparing the live enclave's cryptographic attestation against our public builds. Verification tools, API endpoints, and instructions are on our "}<Link<Route> to={Route::Trustless}>{"Verifiably Private"}</Link<Route>>{" page."}</p>
            </section>

            <section>
                <h2>{"6. Legal Basis for Processing"}</h2>
                <p>{"Under GDPR Article 6, we process your personal data on the following bases:"}</p>
                <ul>
                    <li><strong>{"Contract performance (Art. 6(1)(b)): "}</strong>{"Account creation and management, message processing via bridges, integration connections, and service delivery."}</li>
                    <li><strong>{"Consent (Art. 6(1)(a)): "}</strong>{"AI personalization based on your profile information. You may withdraw consent at any time by updating your profile or contacting us. Withdrawal does not affect the lawfulness of processing before withdrawal."}</li>
                    <li><strong>{"Legitimate interest (Art. 6(1)(f)): "}</strong>{"Security monitoring, fraud prevention, and service improvement. We have balanced these interests against your rights and determined that processing is proportionate given the privacy-preserving architecture of the system."}</li>
                    <li><strong>{"Legal obligation (Art. 6(1)(c)): "}</strong>{"Billing records retention as required by Finnish accounting law."}</li>
                </ul>
            </section>

            <section>
                <h2>{"7. Third-Party Services and Sub-processors"}</h2>
                <p>{"The system depends on third-party infrastructure. Where required by GDPR Article 28, we maintain Data Processing Agreements with our sub-processors or rely on the processor's standard data protection terms. We do not control these services and are not responsible for their behavior."}</p>
                <ul>
                    <li><strong>{"AWS (Amazon Web Services, Inc. - US): "}</strong>{"Provides the Nitro Enclave sealed environment and hosting infrastructure. Isolation depends on Amazon's hardware and software."}</li>
                    <li><strong>{"Marlin (Marlin Protocol - decentralized): "}</strong>{"Key custody. Runs inside its own enclave. Open source."}</li>
                    <li><strong>{"Tinfoil (Tinfoil, Inc. - US): "}</strong>{"AI inference inside sealed environments with cryptographic attestation."}</li>
                    <li><strong>{"Twilio (Twilio, Inc. - US): "}</strong>{"SMS and voice delivery. Message content is designed to be deleted from Twilio after delivery."}</li>
                    <li><strong>{"Stripe (Stripe, Inc. - US): "}</strong>{"Payment processing. We do not store credit card numbers. All payment data is handled by Stripe in accordance with PCI DSS standards."}</li>
                </ul>
                <p>{"A current list of sub-processors is available upon request at rasmus@lightfriend.ai."}</p>
            </section>

            <section>
                <h2>{"8. International Data Transfers"}</h2>
                <p>{"Some of our sub-processors are based in the United States. Data transferred outside the EU/EEA is protected by the following safeguards:"}</p>
                <ul>
                    <li>{"Where applicable, our sub-processors participate in the EU-US Data Privacy Framework or provide Standard Contractual Clauses (SCCs) approved by the European Commission."}</li>
                    <li>{"As a supplementary measure, data stored outside the enclave is encrypted with keys that exist only inside the enclave, meaning sub-processors cannot access plaintext data."}</li>
                </ul>
            </section>

            <section>
                <h2>{"9. AI Processing and Transparency"}</h2>
                <p>{"Lightfriend uses artificial intelligence systems to process your messages and generate responses, summaries, and suggested actions. In accordance with the EU AI Act:"}</p>
                <ul>
                    <li>{"AI processing is performed by third-party large language models accessed through Tinfoil's sealed inference environment."}</li>
                    <li>{"AI-generated content is produced automatically. It may be inaccurate, incomplete, or inappropriate."}</li>
                    <li>{"No automated decision with legal or similarly significant effects is made without your involvement. All AI outputs are recommendations - you decide whether and how to act on them."}</li>
                    <li>{"Lightfriend does not review or endorse AI-generated content."}</li>
                </ul>
            </section>

            <section>
                <h2>{"10. Messaging Integrations"}</h2>
                <p>{"Messages from your connected platforms enter the enclave for processing. Connection data and messages are stored encrypted inside the enclave. You are solely responsible for message content. Lightfriend acts as a technical intermediary."}</p>
                <p>{"Connections to messaging platforms such as WhatsApp, Signal, and Telegram use open-source bridge software that is not an official client of those platforms. Using these integrations may carry risks including account restrictions by the platform provider. See our Terms and Conditions for full details."}</p>
            </section>

            <section>
                <h2>{"11. YouTube Integration"}</h2>
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
                <h2>{"12. Vehicle Integration (Tesla)"}</h2>
                <p>{"THIS IS AN EXPERIMENTAL SERVICE. Commands may control real vehicle functions. YOU ACCEPT FULL RESPONSIBILITY for all commands and their consequences. Lightfriend accepts NO LIABILITY for any accidents, damage, injury, theft, or loss resulting from vehicle commands."}</p>
            </section>

            <section>
                <h2>{"13. Email, MCP, and Other Integrations"}</h2>
                <p>{"All integration credentials are stored encrypted inside the enclave. You connect services at your own risk and are responsible for your accounts with third-party services. We are not liable for third-party service behavior."}</p>
                <p><strong>{"IMPORTANT: "}</strong>{"When you configure MCP (Model Context Protocol) servers, data processed by those servers leaves the sealed enclave environment and is sent to your configured external servers. The privacy guarantees of the enclave do not extend to data processed by external MCP servers. You assume full responsibility for any MCP servers you configure."}</p>
            </section>

            <section>
                <h2>{"14. Your Rights"}</h2>
                <p>{"Under GDPR, you have the following rights regarding your personal data:"}</p>
                <ul>
                    <li><strong>{"Access: "}</strong>{"Request a copy of the personal data we hold about you."}</li>
                    <li><strong>{"Rectification: "}</strong>{"Request correction of inaccurate personal data."}</li>
                    <li><strong>{"Erasure: "}</strong>{"Request deletion of your personal data. Upon receiving a deletion request, we will erase your personal data within 30 days, except where retention is required by law."}</li>
                    <li><strong>{"Data portability: "}</strong>{"Receive your personal data in a structured, commonly used, machine-readable format."}</li>
                    <li><strong>{"Restriction: "}</strong>{"Request restriction of processing in certain circumstances."}</li>
                    <li><strong>{"Objection: "}</strong>{"Object to processing based on legitimate interests."}</li>
                    <li><strong>{"Withdraw consent: "}</strong>{"Where processing is based on consent, withdraw it at any time."}</li>
                    <li><strong>{"Automated decisions: "}</strong>{"Not be subject to decisions based solely on automated processing that produce legal or similarly significant effects."}</li>
                    <li>{"Disconnect any integration at any time."}</li>
                    <li>{"Verify the system's privacy properties yourself."}</li>
                </ul>
                <p>{"To exercise any of these rights, contact rasmus@lightfriend.ai or use the account settings in the Service. We will respond within 30 days."}</p>
            </section>

            <section>
                <h2>{"15. Data Retention"}</h2>
                <ul>
                    <li><strong>{"Messages and integration data: "}</strong>{"Retained until you disconnect the integration or delete your account."}</li>
                    <li><strong>{"Authentication tokens: "}</strong>{"Deleted when you disconnect an integration."}</li>
                    <li><strong>{"Billing records: "}</strong>{"Retained for 6 years as required by Finnish accounting law (Kirjanpitolaki)."}</li>
                    <li><strong>{"Account data: "}</strong>{"Deleted within 30 days of account deletion, except where retention is required by law."}</li>
                </ul>
                <p>{"All stored data is encrypted with keys that exist only inside the enclave."}</p>
            </section>

            <section>
                <h2>{"16. Disclaimers"}</h2>
                <p>{"Lightfriend is open-source software hosted as a service. This policy describes the system's intended design as implemented in the publicly available source code. It is not a warranty or guarantee by the operator."}</p>
                <ul>
                    <li>{"No system is perfectly secure. There may be vulnerabilities unknown to us."}</li>
                    <li>{"Security depends on third-party infrastructure functioning correctly. We do not control these systems."}</li>
                    <li>{"AI outputs may be wrong or harmful. You use them at your own risk."}</li>
                    <li>{"We built this in good faith. We do not warrant it will prevent all unauthorized access."}</li>
                    <li>{"The source code is public and the running instance is cryptographically verifiable. You are responsible for verifying the system before relying on it. Use without verification is at your own discretion and risk."}</li>
                    <li>{"To the fullest extent permitted by law, Lightfriend disclaims all warranties regarding the security, privacy, or integrity of your data."}</li>
                </ul>
            </section>

            <section>
                <h2>{"17. Data Breach Notification"}</h2>
                <p>{"In the event of a personal data breach, we will notify the relevant supervisory authority within 72 hours of becoming aware of the breach, where feasible, in accordance with GDPR Article 33. If the breach is likely to result in a high risk to your rights and freedoms, we will also notify you without undue delay."}</p>
            </section>

            <section>
                <h2>{"18. Cookies"}</h2>
                <p>{"We use strictly necessary cookies for authentication and session management. These cookies are required for the Service to function and cannot be disabled. We do not use tracking cookies, advertising cookies, or third-party analytics cookies."}</p>
            </section>

            <section>
                <h2>{"19. Children"}</h2>
                <p>{"The Service is not intended for children under the age of 16. We do not knowingly collect personal data from children under 16. If you become aware that a child has provided us with personal data, please contact us and we will delete it."}</p>
            </section>

            <section>
                <h2>{"20. Changes"}</h2>
                <p>{"We will notify you of material changes to this policy at least 30 days before they take effect, via email or through the Service. Changes are also visible in our open source repository. If you do not agree with the changes, you may terminate your account before the effective date. Continued use after the effective date constitutes acceptance."}</p>
            </section>

            <section>
                <h2>{"21. Contact"}</h2>
                <p>{"Rasmus Multiverse"}</p>
                <p>{"Tampere, Finland"}</p>
                <p><a href="mailto:rasmus@lightfriend.ai">{"rasmus@lightfriend.ai"}</a></p>
            </section>
            <div class="legal-links">
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Trustless}>{"Verifiably Private"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::TrustChain}>{"Trust Chain"}</Link<Route>>
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
            <h1>{"Lightfriend Terms and Conditions"}</h1>
            <p class="company-name">{"Provided by Rasmus Multiverse"}</p>
            <p class="last-updated">{"Last updated: April 12, 2026"}</p>

            <section>
                <h2>{"1. Introduction"}</h2>
                <p>{"These Terms and Conditions (\"Terms\") govern your use of the Lightfriend platform (\"Service\"), operated by Rasmus Multiverse (sole proprietorship of Rasmus \u{00C4}ht\u{00E4}v\u{00E4}), Tampere, Finland. By accessing or using our Service, you agree to comply with and be bound by these Terms."}</p>
                <p>{"Lightfriend is open-source software hosted as a service. The entire source code is publicly available and the running instance is cryptographically verifiable. We make no guarantees beyond hosting the published code. You are responsible for reviewing the source code, verifying the cryptographic attestation, and satisfying yourself that the system behaves as described before relying on it. All claims about the system's behavior on our website and documentation are descriptions of the open-source code's intended design, not warranties or guarantees by the operator."}</p>
            </section>

            <section>
                <h2>{"2. Eligibility and User Accounts"}</h2>
                <p>{"You must be at least 16 years old to use the Service. By creating an account, you represent that you meet this age requirement."}</p>
                <p>{"To access certain features, you must create an account. You are responsible for maintaining the confidentiality of your account information and for all activities that occur under your account. Lightfriend never has access to your passwords. Please do not provide your password to anyone claiming to be a representative of Lightfriend, or any third party."}</p>
            </section>

            <section>
                <h2>{"3. Acceptable Use"}</h2>
                <p>{"You agree not to use the Service for any unlawful purpose or in any way that could harm the Service or impair anyone else's use of it."}</p>
            </section>

            <section>
                <h2>{"4. Billing, Payments, and Refund Policy"}</h2>

                <h3>{"Payment Processing"}</h3>
                <p>{"Payment processing is handled by Stripe, Inc. We do not store your credit card information. All payment data is processed by Stripe in accordance with PCI DSS standards. Your use of payment features is also subject to "}<a href="https://stripe.com/legal" target="_blank" rel="noopener noreferrer">{"Stripe's terms of service"}</a>{"."}</p>

                <h3>{"Prepaid Credits"}</h3>
                <ul>
                    <li>{"The Service operates on a prepaid credit model where you purchase credits in advance to use for calling and texting features."}</li>
                    <li>{"Usage is deducted from your credit balance based on your actual consumption of the Service's features."}</li>
                    <li>{"You can optionally enable automatic top-up to add more credits to your account when your balance runs low, ensuring uninterrupted service."}</li>
                    <li>{"Detailed usage information and credit history can be accessed through your account profile billing section."}</li>
                </ul>

                <h3>{"EU Right of Withdrawal"}</h3>
                <p>{"Under the EU Consumer Rights Directive (2011/83/EU), you have a 14-day right of withdrawal for online purchases. However, by purchasing credits, you expressly consent that the digital content (credits) becomes available for use immediately upon purchase. You acknowledge that you thereby lose your right of withdrawal once credits are added to your account, pursuant to Article 16(m) of Directive 2011/83/EU."}</p>

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
                    <li>{"Changes to pricing or fundamental service features will be communicated to users at least 30 days in advance."}</li>
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
                <p>{"The Lightfriend name, logos, and visual branding are the property of Rasmus Multiverse and are protected by intellectual property laws. The Lightfriend software source code is licensed under the GNU Affero General Public License v3 (AGPLv3). Your rights to the source code are governed by that license. User-generated content remains your property."}</p>
            </section>

            <section>
                <h2>{"6. Termination"}</h2>
                <p>{"We may suspend or terminate your access to the Service immediately and without notice for conduct that violates these Terms or is harmful to other users or the Service."}</p>
                <p>{"If we terminate your account for reasons other than a violation of these Terms, we will provide 14 days' notice and refund any unused prepaid credits."}</p>
                <p>{"You may terminate your account at any time through your account settings or by contacting us."}</p>
            </section>

            <section>
                <h2>{"7. Limitation of Liability"}</h2>
                <p>{"THE SERVICE IS PROVIDED \"AS IS\" AND \"AS AVAILABLE\" WITHOUT WARRANTIES OF ANY KIND, EXPRESS OR IMPLIED. TO THE FULLEST EXTENT PERMITTED BY LAW:"}</p>
                <ul>
                    <li>{"Lightfriend makes no warranties regarding accuracy, reliability, availability, or security of the Service."}</li>
                    <li>{"Lightfriend shall not be liable for any indirect, incidental, special, consequential, or punitive damages arising from your use of or inability to use the Service."}</li>
                    <li>{"Lightfriend is not responsible for any actions taken by third-party services, integrations, or AI models."}</li>
                    <li>{"The Service's privacy architecture depends on third-party infrastructure (AWS Nitro Enclaves, Marlin, Tinfoil, and others). Lightfriend does not warrant the correct functioning of these systems and is not liable for failures in third-party infrastructure."}</li>
                    <li>{"Descriptions of security architecture on the Service's website and documentation describe intended system design, not warranties. No computing system is perfectly secure."}</li>
                    <li>{"You assume all risk associated with your use of the Service, any connected third-party services, and any actions taken based on AI-generated content."}</li>
                    <li>{"The Service provides verification tools so you can independently assess its properties. Use of the Service without performing verification is at your own discretion and risk."}</li>
                    <li>{"In no event shall Lightfriend's total liability exceed the amount you paid for the Service in the preceding 12 months."}</li>
                </ul>
                <p>{"Nothing in these Terms excludes or limits our liability for: (a) death or personal injury caused by our negligence; (b) fraud or fraudulent misrepresentation; (c) any liability that cannot be excluded or limited under applicable law, including mandatory consumer protection laws of the European Union."}</p>
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
                <p>{"THIS IS AN EXPERIMENTAL FEATURE. Vehicle control is entirely user-initiated: you choose to connect your vehicle account, you choose to enable the integration, and you choose to send each command. Lightfriend hosts the open-source code that relays your commands - it does not autonomously initiate vehicle actions. BY USING THE TESLA INTEGRATION, YOU EXPRESSLY ACKNOWLEDGE AND AGREE:"}</p>
                <ul>
                    <li>{"Commands sent through Lightfriend may control real vehicle functions including unlocking, climate control, and other operations."}</li>
                    <li>{"YOU ACCEPT FULL AND SOLE RESPONSIBILITY for all vehicle commands and their consequences. You initiated the connection, you enabled the integration, and you sent or approved the command."}</li>
                    <li>{"You must verify that conditions are safe before sending any vehicle command."}</li>
                    <li>{"The integration code is open source. You are responsible for reviewing it and satisfying yourself that it behaves acceptably before connecting your vehicle."}</li>
                    <li>{"Lightfriend does not verify the safety, appropriateness, or timing of any command."}</li>
                    <li>{"LIGHTFRIEND ACCEPTS NO LIABILITY WHATSOEVER for any accidents, vehicle damage, personal injury, death, theft, property damage, or any other consequences resulting from vehicle commands."}</li>
                    <li>{"By using this integration, you EXPLICITLY WAIVE any and all claims against Lightfriend related to vehicle control, to the fullest extent permitted by applicable law."}</li>
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
                <h2>{"11. Messaging Services and Third-Party Platform Risk"}</h2>
                <p>{"The Service connects to messaging platforms such as WhatsApp, Signal, and Telegram using open-source bridge software. These bridges are not official clients and are not endorsed, sponsored, or affiliated with the messaging platforms they connect to. WhatsApp is a trademark of Meta Platforms, Inc. Signal is a trademark of Signal Technology Foundation. Telegram is a trademark of Telegram FZ-LLC. By using these integrations, you acknowledge and agree:"}</p>
                <ul>
                    <li><strong>{"Unofficial client risk: "}</strong>{"Connecting your messaging accounts through Lightfriend uses third-party bridge software that is not approved by the messaging platform providers. This may violate the terms of service of those platforms."}</li>
                    <li><strong>{"Account restrictions: "}</strong>{"Your messaging accounts may be flagged, restricted, suspended, or permanently banned by the platform provider as a result of using unofficial bridge software. This is a known risk that you accept by using these integrations."}</li>
                    <li><strong>{"No guarantee of continued access: "}</strong>{"Messaging platform providers may change their systems, policies, or enforcement at any time, which may cause integrations to stop working or trigger account restrictions without warning."}</li>
                    <li><strong>{"Your responsibility: "}</strong>{"You are the owner of each messaging account you connect and are solely responsible for compliance with each platform's terms of service. You connect these accounts at your own risk."}</li>
                    <li><strong>{"No liability: "}</strong>{"Lightfriend is not liable for any account suspension, data loss, service interruption, or any other consequence resulting from your use of messaging integrations, including actions taken by messaging platform providers against your account."}</li>
                    <li>{"You are solely responsible for all message content sent through the Service."}</li>
                    <li>{"Lightfriend acts only as a technical intermediary and does not monitor or control message content."}</li>
                </ul>
            </section>

            <section>
                <h2>{"12. MCP Server Integration"}</h2>
                <p><strong>{"IMPORTANT: "}</strong>{"When you configure MCP (Model Context Protocol) servers, data processed by those servers leaves the sealed enclave environment and is sent to your configured external servers. The privacy guarantees of the enclave do not extend to data processed by external MCP servers."}</p>
                <ul>
                    <li>{"You assume full responsibility for any third-party MCP servers you configure."}</li>
                    <li>{"Data sent to MCP servers as part of AI processing is transmitted at your own risk."}</li>
                    <li>{"Lightfriend is not liable for the behavior, data handling, availability, or security of any third-party MCP server."}</li>
                    <li>{"You must ensure that your use of third-party MCP servers complies with applicable laws and the terms of those services."}</li>
                </ul>
            </section>

            <section>
                <h2>{"13. AI Transparency"}</h2>
                <p>{"In accordance with the EU AI Act, you are informed that:"}</p>
                <ul>
                    <li>{"The Service uses artificial intelligence systems to process your messages and generate responses, summaries, and suggested actions."}</li>
                    <li>{"AI processing is performed by third-party large language models accessed through sealed inference environments."}</li>
                    <li>{"No automated decision with legal or similarly significant effects is made solely by AI. All AI outputs are recommendations only. You are always in control of whether to act on them."}</li>
                </ul>
            </section>

            <section>
                <h2>{"14. Indemnification"}</h2>
                <p>{"To the extent permitted by applicable law, you agree to compensate Lightfriend, its owners, employees, and affiliates for losses directly resulting from:"}</p>
                <ul>
                    <li>{"Your breach of these Terms."}</li>
                    <li>{"Your violation of any applicable law or third-party rights."}</li>
                    <li>{"Your willful misconduct or gross negligence in using the Service or connected integrations."}</li>
                </ul>
            </section>

            <section>
                <h2>{"15. Changes to Terms"}</h2>
                <p>{"We will notify you of material changes to these Terms at least 30 days before they take effect, via email or through the Service. Your continued use after the effective date constitutes acceptance. If you do not agree with the changes, you may terminate your account before the effective date."}</p>
            </section>

            <section>
                <h2>{"16. Governing Law and Dispute Resolution"}</h2>
                <p>{"These Terms are governed by and construed in accordance with the laws of Finland. Any disputes shall be resolved in the courts of Tampere, Finland."}</p>
                <p>{"The European Commission provides an online dispute resolution platform at "}<a href="https://ec.europa.eu/consumers/odr" target="_blank" rel="noopener noreferrer">{"https://ec.europa.eu/consumers/odr"}</a>{". We are not obligated to participate in alternative dispute resolution proceedings but will consider requests in good faith."}</p>
            </section>

            <section>
                <h2>{"17. Data Protection and Privacy"}</h2>
                <p>{"Our Privacy Policy describes how the system is designed to handle your data. It forms an integral part of these Terms. By using the Service, you acknowledge that you have read and understood our Privacy Policy."}</p>
            </section>

            <section>
                <h2>{"18. General Provisions"}</h2>
                <ul>
                    <li><strong>{"Severability: "}</strong>{"If any provision of these Terms is found to be unenforceable or invalid, that provision will be limited or eliminated to the minimum extent necessary so that these Terms will otherwise remain in full force and effect."}</li>
                    <li><strong>{"Entire agreement: "}</strong>{"These Terms, together with the Privacy Policy and any other policies referenced herein, constitute the entire agreement between you and Lightfriend regarding the Service and supersede all prior agreements."}</li>
                    <li><strong>{"Waiver: "}</strong>{"The failure of Lightfriend to enforce any right or provision of these Terms will not be considered a waiver of those rights."}</li>
                    <li><strong>{"Assignment: "}</strong>{"We may assign or transfer these Terms and our rights and obligations in connection with a merger, acquisition, or sale of assets. You may not assign your rights or obligations under these Terms without our prior written consent."}</li>
                    <li><strong>{"Force majeure: "}</strong>{"We shall not be liable for any failure or delay in performing our obligations where such failure or delay results from circumstances beyond our reasonable control, including but not limited to natural disasters, war, epidemics, internet or telecommunications failures, or failures of third-party infrastructure."}</li>
                </ul>
            </section>

            <section>
                <h2>{"19. Contact Us"}</h2>
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
                {" | "}
                <Link<Route> to={Route::TrustChain}>{"Trust Chain"}</Link<Route>>
            </div>
        </div>

    }
}
