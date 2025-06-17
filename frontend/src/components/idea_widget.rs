/*
use yew::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use web_sys::window;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::HashSet;

#[derive(Deserialize, Debug, Clone)]
struct Idea {
    id: Option<i32>,
    text: String,
    created_at: i32,
    is_upvoted: bool,
}

#[derive(Serialize)]
struct CreateIdeaRequest {
    text: String,
}

#[derive(Serialize)]
struct EmailSubscriptionRequest {
    email: String,
}

#[derive(Properties, PartialEq)]
pub struct IdeaWidgetProps {
    #[prop_or_default]
    pub creator_id: Option<String>,
}

pub enum IdeaWidgetMsg {
    Toggle,
    SetNewIdea(String),
    SubmitIdea,
    SetEmail(String),
    SubscribeEmail(i32),
    UpvoteIdea(i32),
    LoadIdeas,
    IdeasLoaded(Vec<Idea>),
    Error(String),
    ClearError,
}

pub struct IdeaWidget {
    expanded: bool,
    new_idea: String,
    email: String,
    ideas: Vec<Idea>,
    error: Option<String>,
    upvoted_ideas: HashSet<i32>,
    subscribed_ideas: HashSet<i32>,
}

impl Component for IdeaWidget {
    type Message = IdeaWidgetMsg;
    type Properties = IdeaWidgetProps;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(IdeaWidgetMsg::LoadIdeas);
        
        Self {
            expanded: false,
            new_idea: String::new(),
            email: String::new(),
            ideas: Vec::new(),
            error: None,
            upvoted_ideas: HashSet::new(),
            subscribed_ideas: HashSet::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            IdeaWidgetMsg::Toggle => {
                self.expanded = !self.expanded;
                true
            }
            IdeaWidgetMsg::SetNewIdea(idea) => {
                self.new_idea = idea;
                true
            }
            IdeaWidgetMsg::SetEmail(email) => {
                self.email = email;
                true
            }
            IdeaWidgetMsg::SubmitIdea => {
                if self.new_idea.trim().is_empty() {
                    self.error = Some("Please enter an idea".to_string());
                    return true;
                }

                let idea = self.new_idea.clone();
                ctx.link().send_future(async move {
                    let request = match Request::post("/api/ideas")
                        .json(&CreateIdeaRequest { text: idea }) {
                            Ok(req) => req,
                            Err(e) => return IdeaWidgetMsg::Error(e.to_string()),
                    };

                    match request.send().await {
                        Ok(_) => IdeaWidgetMsg::LoadIdeas,
                        Err(e) => IdeaWidgetMsg::Error(e.to_string()),
                    }
                });
                self.new_idea.clear();
                true
            }
            IdeaWidgetMsg::UpvoteIdea(id) => {
                if !self.upvoted_ideas.contains(&id) {
                    ctx.link().send_future(async move {
                        match Request::post(&format!("/api/ideas/{}/upvote", id))
                            .send()
                            .await
                        {
                            Ok(_) => IdeaWidgetMsg::LoadIdeas,
                            Err(e) => IdeaWidgetMsg::Error(e.to_string()),
                        }
                    });
                    self.upvoted_ideas.insert(id);
                }
                true
            }
            IdeaWidgetMsg::SubscribeEmail(id) => {
                if self.email.trim().is_empty() {
                    self.error = Some("Please enter an email".to_string());
                    return true;
                }

                let email = self.email.clone();
                ctx.link().send_future(async move {
                    let request = match Request::post(&format!("/api/ideas/{}/subscribe", id))
                        .json(&EmailSubscriptionRequest { email }) {
                            Ok(req) => req,
                            Err(e) => return IdeaWidgetMsg::Error(e.to_string()),
                    };

                    match request.send().await {
                        Ok(_) => IdeaWidgetMsg::LoadIdeas,
                        Err(e) => IdeaWidgetMsg::Error(e.to_string()),
                    }
                });
                self.subscribed_ideas.insert(id);
                true
            }
            IdeaWidgetMsg::LoadIdeas => {
                ctx.link().send_future(async {
                    match Request::get("/api/ideas")
                        .send()
                        .await
                    {
                        Ok(response) => {
                            match response.json::<Vec<Idea>>().await {
                                Ok(ideas) => IdeaWidgetMsg::IdeasLoaded(ideas),
                                Err(e) => IdeaWidgetMsg::Error(e.to_string()),
                            }
                        }
                        Err(e) => IdeaWidgetMsg::Error(e.to_string()),
                    }
                });
                false
            }
            IdeaWidgetMsg::IdeasLoaded(ideas) => {
                self.ideas = ideas;
                true
            }
            IdeaWidgetMsg::Error(error) => {
                self.error = Some(error);
                true
            }
            IdeaWidgetMsg::ClearError => {
                self.error = None;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let toggle = ctx.link().callback(|_| IdeaWidgetMsg::Toggle);
        
        html! {
            <div class={classes!("idea-widget", if self.expanded { "expanded" } else { "" })}>
                <button onclick={toggle} class="idea-widget-toggle">
                    if self.expanded {
                        { "✕" }
                    } else {
                        { "💡" }
                    }
                </button>
                
                if self.expanded {
                    <div class="idea-widget-content">
                        <h3>{ "Share Your Ideas" }</h3>
                        
                        if let Some(error) = &self.error {
                            <div class="error-message">
                                { error }
                                <button onclick={ctx.link().callback(|_| IdeaWidgetMsg::ClearError)}>
                                    { "✕" }
                                </button>
                            </div>
                        }
                        
                        <div class="idea-input">
                            <textarea
                                placeholder="What's your idea?"
                                value={self.new_idea.clone()}
                                onchange={ctx.link().callback(|e: Event| {
                                    let input: HtmlInputElement = e.target_unchecked_into();
                                    IdeaWidgetMsg::SetNewIdea(input.value())
                                })}
                            />
                            <button onclick={ctx.link().callback(|_| IdeaWidgetMsg::SubmitIdea)}>
                                { "Submit" }
                            </button>
                        </div>
                        
                        <div class="ideas-list">
                            { for self.ideas.iter().map(|idea| self.render_idea(ctx, idea)) }
                        </div>
                    </div>
                }
            </div>
        }
    }
}

impl IdeaWidget {
    fn render_idea(&self, ctx: &Context<Self>, idea: &Idea) -> Html {
        let idea_id = idea.id.unwrap_or(0);
        let is_upvoted = idea.is_upvoted;
        
        html! {
            <div class="idea-item">
                <p>{ &idea.text }</p>
                <div class="idea-actions">
                    if !self.subscribed_ideas.contains(&idea_id) {
                        <input
                            type="email"
                            placeholder="Your email for updates"
                            value={self.email.clone()}
                            onchange={ctx.link().callback(|e: Event| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                IdeaWidgetMsg::SetEmail(input.value())
                            })}
                        />
                        <button onclick={ctx.link().callback(move |_| IdeaWidgetMsg::SubscribeEmail(idea_id))}>
                            { "Subscribe" }
                        </button>
                    }
                    <button 
                        class={classes!("upvote-btn", if is_upvoted { "upvoted" } else { "" })}
                        onclick={ctx.link().callback(move |_| IdeaWidgetMsg::UpvoteIdea(idea_id))}
                        disabled={is_upvoted}
                    >
                        { if is_upvoted { "Upvoted" } else { "Upvote" } }
                    </button>
                </div>
            </div>
        }
    }
}

/*
