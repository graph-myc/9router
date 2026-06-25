//! 9Router frontend (Leptos CSR): polished dashboard over the backend API.

use gloo_net::http::Request;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde_json::Value;
use std::collections::HashSet;

const API_BASE: &str = "http://localhost:20129/v1";

/// (id, name)
type Models = RwSignal<Vec<(String, String)>>;
/// (name, strategy, targets)
type Combos = RwSignal<Vec<(String, String, Vec<String>)>>;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

fn pretty_strategy(s: &str) -> String {
    s.replace('_', "-")
}

#[component]
fn App() -> impl IntoView {
    let view = RwSignal::new("endpoint".to_string());
    let version = RwSignal::new("…".to_string());
    let models: Models = RwSignal::new(Vec::new());
    let combos: Combos = RwSignal::new(Vec::new());

    spawn_local(async move {
        if let Some(v) = fetch_json("/version").await {
            version.set(v["version"].as_str().unwrap_or("?").to_string());
        }
    });
    spawn_local(async move {
        if let Some(v) = fetch_json("/v1/models").await {
            let list = v["data"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|m| {
                    Some((
                        m["id"].as_str()?.to_string(),
                        m["name"].as_str().unwrap_or("").to_string(),
                    ))
                })
                .collect();
            models.set(list);
        }
    });
    spawn_local(async move {
        if let Some(v) = fetch_json("/api/combos").await {
            let list = v["combos"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|c| {
                    let name = c["name"].as_str().unwrap_or("").to_string();
                    let strat = c["strategy"]["type"].as_str().unwrap_or("fallback").to_string();
                    let targets = c["targets"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .iter()
                        .map(|t| {
                            format!(
                                "{}/{}",
                                t["provider"].as_str().unwrap_or(""),
                                t["model"].as_str().unwrap_or("")
                            )
                        })
                        .collect();
                    (name, strat, targets)
                })
                .collect();
            combos.set(list);
        }
    });

    view! {
        <div class="layout">
            <Sidebar view=view/>
            <div class="main">
                <Topbar view=view version=version/>
                <div class="content">
                    {move || match view.get().as_str() {
                        "models" => view! { <ModelsView models=models combos=combos/> }.into_any(),
                        "playground" => view! { <Playground models=models combos=combos/> }.into_any(),
                        _ => view! { <EndpointView version=version models=models combos=combos/> }.into_any(),
                    }}
                </div>
            </div>
        </div>
    }
}

#[component]
fn Sidebar(view: RwSignal<String>) -> impl IntoView {
    view! {
        <aside class="sidebar">
            <div class="brand">
                <div class="logo">"✦"</div>
                <div><h1>"9Router"</h1><small>"Aggregator · v0.0.1"</small></div>
            </div>
            <button class="navitem" class:active=move || view.get() == "endpoint"
                on:click=move |_| view.set("endpoint".to_string())>
                <span class="ic">"🔌"</span><span>"Endpoint"</span>
            </button>
            <button class="navitem" class:active=move || view.get() == "models"
                on:click=move |_| view.set("models".to_string())>
                <span class="ic">"📦"</span><span>"Models & Combos"</span>
            </button>
            <button class="navitem" class:active=move || view.get() == "playground"
                on:click=move |_| view.set("playground".to_string())>
                <span class="ic">"▶"</span><span>"Playground"</span>
            </button>
            <div class="navgroup">"System"</div>
            <button class="navitem" disabled=true><span class="ic">"📊"</span><span>"Usage"</span></button>
            <button class="navitem" disabled=true><span class="ic">"🔑"</span><span>"API Keys"</span></button>
            <button class="navitem" disabled=true><span class="ic">"⚙"</span><span>"Settings"</span></button>
        </aside>
    }
}

#[component]
fn Topbar(view: RwSignal<String>, version: RwSignal<String>) -> impl IntoView {
    let title = move || match view.get().as_str() {
        "models" => "Models & Combos",
        "playground" => "Playground",
        _ => "Endpoint",
    };
    view! {
        <div class="topbar">
            <div>
                <h2>{title}</h2>
                <div class="sub">"Agentic orchestrator — fallback · round-robin · fusion"</div>
            </div>
            <div class="row">
                <span class="badge">{move || format!("v{}", version.get())}</span>
                <span class="badge ok">"● online"</span>
            </div>
        </div>
    }
}

#[component]
fn EndpointView(version: RwSignal<String>, models: Models, combos: Combos) -> impl IntoView {
    let (copied, set_copied) = signal(false);
    view! {
        <div class="card">
            <h3><span>"🔌"</span>"API Endpoint"</h3>
            <div class="row">
                <span class="chip">"Local"</span>
                <input class="input" prop:value=API_BASE readonly=true/>
                <button class="btn ghost" on:click=move |_| set_copied.set(true)>
                    {move || if copied.get() { "Copied" } else { "Copy" }}
                </button>
            </div>
            <p class="muted" style="margin-top:12px;margin-bottom:0">
                "OpenAI-compatible. Point any client (Claude Code, Cursor, Cline…) at this base URL."
            </p>
        </div>
        <div class="card">
            <h3><span>"❤️"</span>"Status"</h3>
            <div class="grid">
                <div class="modelrow"><span class="muted">"Version"</span><b>{move || version.get()}</b></div>
                <div class="modelrow"><span class="muted">"Provider models"</span>
                    <b>{move || {
                        let cn: HashSet<String> = combos.get().into_iter().map(|c| c.0).collect();
                        models.get().into_iter().filter(|(id, _)| !cn.contains(id)).count().to_string()
                    }}</b>
                </div>
                <div class="modelrow"><span class="muted">"Combos"</span><b>{move || combos.get().len().to_string()}</b></div>
                <div class="modelrow"><span class="muted">"Backend"</span><span class="chip round-robin">"healthy"</span></div>
            </div>
        </div>
    }
}

#[component]
fn ModelsView(models: Models, combos: Combos) -> impl IntoView {
    view! {
        <div class="card">
            <h3><span>"🧩"</span>"Combos"</h3>
            <div class="grid">
                {move || combos.get().into_iter().map(|(name, strat, targets)| {
                    let cls = format!("chip {}", pretty_strategy(&strat));
                    view! {
                        <div class="modelrow">
                            <div>
                                <b>{name}</b>
                                <div class="muted" style="margin-top:4px">{targets.join("  ·  ")}</div>
                            </div>
                            <span class=cls>{pretty_strategy(&strat)}</span>
                        </div>
                    }
                }).collect_view()}
            </div>
        </div>
        <div class="card">
            <h3><span>"📦"</span>"Provider models"</h3>
            <div class="grid">
                {move || {
                    let cn: HashSet<String> = combos.get().into_iter().map(|c| c.0).collect();
                    models.get().into_iter().filter(|(id, _)| !cn.contains(id)).map(|(id, name)| {
                        view! {
                            <div class="modelrow">
                                <code style="font-size:12.5px">{id}</code>
                                <span class="muted">{name}</span>
                            </div>
                        }
                    }).collect_view()
                }}
            </div>
        </div>
    }
}

#[component]
fn Playground(models: Models, combos: Combos) -> impl IntoView {
    let model = RwSignal::new(String::new());
    let prompt = RwSignal::new(String::new());
    let out = RwSignal::new(String::new());
    let busy = RwSignal::new(false);

    let send = move |_| {
        let m = model.get();
        let p = prompt.get();
        if m.is_empty() {
            out.set("Pick a model or combo first.".to_string());
            return;
        }
        busy.set(true);
        out.set("…".to_string());
        spawn_local(async move {
            let body = serde_json::json!({
                "model": m, "messages": [{ "role": "user", "content": p }], "stream": false
            });
            let result = match Request::post(&format!("{API_BASE}/chat/completions")).json(&body) {
                Ok(req) => match req.send().await {
                    Ok(resp) => match resp.json::<Value>().await {
                        Ok(v) => format_result(&v),
                        Err(e) => format!("parse error: {e}"),
                    },
                    Err(e) => format!("network error: {e}"),
                },
                Err(e) => format!("request error: {e}"),
            };
            out.set(result);
            busy.set(false);
        });
    };

    view! {
        <div class="card">
            <h3><span>"▶"</span>"Playground"</h3>
            <div class="grid">
                <select class="input" on:change=move |e| model.set(event_target_value(&e))>
                    <option value="">"— pick a model or combo —"</option>
                    {move || combos.get().into_iter().map(|(n, s, _)| {
                        let label = format!("{}  (combo · {})", n, pretty_strategy(&s));
                        view! { <option value=n>{label}</option> }
                    }).collect_view()}
                    {move || {
                        let cn: HashSet<String> = combos.get().into_iter().map(|c| c.0).collect();
                        models.get().into_iter().filter(|(id, _)| !cn.contains(id)).map(|(id, _)| {
                            let label = id.clone();
                            view! { <option value=id>{label}</option> }
                        }).collect_view()
                    }}
                </select>
                <textarea class="input" rows="4" placeholder="Type a prompt…"
                    on:input=move |e| prompt.set(event_target_value(&e))
                    prop:value=move || prompt.get()></textarea>
                <div class="row">
                    <button class="btn" on:click=send disabled=move || busy.get()>
                        {move || if busy.get() { "Sending…" } else { "Send" }}
                    </button>
                    <span class="muted">"Routed through the orchestrator; the answering target is reported."</span>
                </div>
                <pre class="codeblock">{move || out.get()}</pre>
            </div>
        </div>
    }
}

fn format_result(v: &Value) -> String {
    if let Some(err) = v.get("error").filter(|e| !e.is_null()) {
        return format!("✗ error: {}", err["message"].as_str().unwrap_or("unknown"));
    }
    let content = v["choices"][0]["message"]["content"].as_str().unwrap_or("");
    let target = v["x_9router_target"].as_str().unwrap_or("?");
    let fused = v["x_9router_fused"].as_bool().unwrap_or(false);
    let pin = v["usage"]["prompt_tokens"].as_u64().unwrap_or(0);
    let pout = v["usage"]["completion_tokens"].as_u64().unwrap_or(0);
    format!(
        "✓ {target}{}\n\n{content}\n\n[tokens: {pin} in / {pout} out]",
        if fused { "  ·  fused" } else { "" }
    )
}

async fn fetch_json(url: &str) -> Option<Value> {
    Request::get(url).send().await.ok()?.json::<Value>().await.ok()
}
