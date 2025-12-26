use yew::prelude::*;
use crate::proactive::contact_profiles::ContactProfilesSection;

#[derive(Properties, PartialEq)]
pub struct CriticalSectionProps {
    #[prop_or(false)]
    pub proactive_disabled: bool,
}

#[function_component(CriticalSection)]
pub fn critical_section(props: &CriticalSectionProps) -> Html {
    html! {
        <ContactProfilesSection
            critical_disabled={props.proactive_disabled}
        />
    }
}
