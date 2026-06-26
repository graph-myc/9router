//! M Y C frontend (Leptos CSR): polished dashboard over the backend API.

use gloo_net::http::Request;
use leptos::prelude::*;
use leptos::task::spawn_local;
use futures::StreamExt;
use serde_json::Value;
use std::collections::HashSet;
use wasm_bindgen::JsValue;

const API_BASE: &str = "http://localhost:20130/v1";

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
                        "providers" => view! { <ProvidersView/> }.into_any(),
                        "usage" => view! { <UsageView/> }.into_any(),
                        "quota" => view! { <QuotaView/> }.into_any(),
                        "console" => view! { <ConsoleLogView/> }.into_any(),
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
                <div><h1>"M Y C"</h1><small>"Aggregator · v0.0.1"</small></div>
            </div>
            <button class="navitem" class:active=move || view.get() == "endpoint"
                on:click=move |_| view.set("endpoint".to_string())>
                <span class="ic">"🔌"</span><span>"Endpoint"</span>
            </button>
            <button class="navitem" class:active=move || view.get() == "providers"
                on:click=move |_| view.set("providers".to_string())>
                <span class="ic">"🧠"</span><span>"Providers"</span>
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
            <button class="navitem" class:active=move || view.get() == "usage"
                on:click=move |_| view.set("usage".to_string())>
                <span class="ic">"📊"</span><span>"Usage"</span>
            </button>
            <button class="navitem" class:active=move || view.get() == "quota"
                on:click=move |_| view.set("quota".to_string())>
                <span class="ic">"📈"</span><span>"Quota"</span>
            </button>
            <button class="navitem" class:active=move || view.get() == "console"
                on:click=move |_| view.set("console".to_string())>
                <span class="ic">"🖥"</span><span>"Console Log"</span>
            </button>
            <button class="navitem" disabled=true><span class="ic">"🔑"</span><span>"API Keys"</span></button>
            <button class="navitem" disabled=true><span class="ic">"⚙"</span><span>"Settings"</span></button>
        </aside>
    }
}

#[component]
fn Topbar(view: RwSignal<String>, version: RwSignal<String>) -> impl IntoView {
    let title = move || match view.get().as_str() {
        "providers" => "Providers",
        "usage" => "Usage",
        "quota" => "Quota",
        "console" => "Console Log",
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
fn ProvidersView() -> impl IntoView {
    let providers: RwSignal<Vec<Value>> = RwSignal::new(Vec::new());
    let reload = Callback::new(move |_| {
        spawn_local(async move {
            if let Some(v) = fetch_json("/api/providers").await {
                providers.set(v["providers"].as_array().cloned().unwrap_or_default());
            }
        });
    });
    reload.run(());

    let nid = RwSignal::new(String::new());
    let nurl = RwSignal::new(String::new());
    let nkey = RwSignal::new(String::new());
    let add_msg = RwSignal::new(String::new());
    let add = move |_| {
        let id = nid.get();
        let url = nurl.get();
        if id.is_empty() || url.is_empty() {
            add_msg.set("id and base URL are required".to_string());
            return;
        }
        let body = serde_json::json!({
            "id": id, "base_url": url, "api_key": nkey.get(), "models": []
        });
        spawn_local(async move {
            post_json("/api/providers", &body).await;
            nid.set(String::new());
            nurl.set(String::new());
            nkey.set(String::new());
            add_msg.set(String::new());
            reload.run(());
        });
    };

    view! {
        <div class="card">
            <h3><span>"➕"</span>"Add provider"</h3>
            <div class="grid">
                <div class="row">
                    <input class="input" placeholder="id (e.g. openai)" prop:value=move || nid.get()
                        on:input=move |e| nid.set(event_target_value(&e))/>
                    <input class="input" placeholder="base URL (https://api.…/v1)" prop:value=move || nurl.get()
                        on:input=move |e| nurl.set(event_target_value(&e))/>
                </div>
                <div class="row">
                    <input class="input" placeholder="API key (optional)" prop:value=move || nkey.get()
                        on:input=move |e| nkey.set(event_target_value(&e))/>
                    <button class="btn" on:click=add>"Add"</button>
                </div>
                {move || { let m = add_msg.get(); (!m.is_empty()).then(|| view! { <span class="muted">{m}</span> }) }}
            </div>
        </div>
        {move || providers.get().into_iter()
            .map(|p| view! { <ProviderCard prov=p reload=reload/> })
            .collect_view()}
    }
}

#[component]
fn ProviderCard(prov: Value, reload: Callback<()>) -> impl IntoView {
    let id = StoredValue::new(prov["id"].as_str().unwrap_or("").to_string());
    let base_url = RwSignal::new(prov["base_url"].as_str().unwrap_or("").to_string());
    let key_set = prov["api_key_set"].as_bool().unwrap_or(false);
    let api_key = RwSignal::new(String::new());
    let init_models: Vec<(String, String)> = prov["models"]
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
    let models = RwSignal::new(init_models);
    let fetched: RwSignal<Vec<String>> = RwSignal::new(Vec::new());
    let new_model = RwSignal::new(String::new());
    let out = RwSignal::new(String::new());
    let busy = RwSignal::new(false);

    let persist = move || {
        serde_json::json!({
            "id": id.get_value(),
            "base_url": base_url.get(),
            "api_key": api_key.get(),
            "models": models.get().iter()
                .map(|(i, n)| serde_json::json!({ "id": i, "name": n }))
                .collect::<Vec<_>>(),
        })
    };

    let save = move |_| {
        let body = persist();
        busy.set(true);
        spawn_local(async move {
            post_json("/api/providers", &body).await;
            busy.set(false);
            out.set("saved".to_string());
            reload.run(());
        });
    };
    let del = move |_| {
        spawn_local(async move {
            delete_req(&format!("/api/providers/{}", id.get_value())).await;
            reload.run(());
        });
    };
    let test = move |_| {
        busy.set(true);
        out.set("testing…".to_string());
        spawn_local(async move {
            let r = post_json(
                &format!("/api/providers/{}/test", id.get_value()),
                &serde_json::json!({}),
            )
            .await;
            out.set(fmt_test(r));
            busy.set(false);
        });
    };
    let fetch = move |_| {
        busy.set(true);
        spawn_local(async move {
            let r = post_json(
                &format!("/api/providers/{}/models/fetch", id.get_value()),
                &serde_json::json!({}),
            )
            .await;
            match r {
                Some(v) if v["ok"].as_bool().unwrap_or(false) => {
                    let list: Vec<String> = v["models"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect();
                    let n = list.len();
                    fetched.set(list);
                    out.set(format!("fetched {n} models — click to add, then Save"));
                }
                Some(v) => out.set(format!("✗ {}", v["error"].as_str().unwrap_or("fetch failed"))),
                None => out.set("✗ network error".to_string()),
            }
            busy.set(false);
        });
    };
    let test_add = move |_| {
        let m = new_model.get();
        if m.is_empty() {
            out.set("enter a model id".to_string());
            return;
        }
        busy.set(true);
        out.set(format!("testing {m}…"));
        spawn_local(async move {
            let r = post_json(
                &format!("/api/providers/{}/test", id.get_value()),
                &serde_json::json!({ "model": m }),
            )
            .await;
            match r {
                Some(v) if v["ok"].as_bool().unwrap_or(false) => {
                    if !models.get().iter().any(|(i, _)| i == &m) {
                        models.update(|ms| ms.push((m.clone(), m.clone())));
                    }
                    post_json("/api/providers", &persist()).await;
                    out.set(format!("✓ added {m} ({} ms)", v["latency_ms"].as_u64().unwrap_or(0)));
                    new_model.set(String::new());
                    reload.run(());
                }
                Some(v) => out.set(format!(
                    "✗ {} [{}]",
                    v["error"].as_str().unwrap_or("test failed"),
                    v["status"].as_u64().unwrap_or(0)
                )),
                None => out.set("✗ network error".to_string()),
            }
            busy.set(false);
        });
    };

    view! {
        <div class="card">
            <h3><span>"🧠"</span>{id.get_value()}</h3>
            <div class="grid">
                <div class="row">
                    <input class="input" prop:value=move || base_url.get()
                        on:input=move |e| base_url.set(event_target_value(&e))/>
                    <span class=move || if key_set { "chip round-robin" } else { "chip" }>
                        {move || if key_set { "key set" } else { "no key" }}
                    </span>
                </div>
                <div class="row">
                    <input class="input"
                        placeholder=if key_set { "API key — leave blank to keep current" } else { "API key" }
                        prop:value=move || api_key.get()
                        on:input=move |e| api_key.set(event_target_value(&e))/>
                </div>
                <div class="row">
                    <button class="btn" on:click=save disabled=move || busy.get()>"Save"</button>
                    <button class="btn ghost" on:click=test disabled=move || busy.get()>"Test"</button>
                    <button class="btn ghost" on:click=fetch disabled=move || busy.get()>"Fetch Models"</button>
                    <button class="btn ghost" on:click=del>"Delete"</button>
                </div>
                <div class="muted">"Models"</div>
                {move || {
                    let ms = models.get();
                    if ms.is_empty() {
                        view! { <span class="muted">"no models yet"</span> }.into_any()
                    } else {
                        ms.into_iter().map(|(mid, _)| {
                            let x = mid.clone();
                            view! {
                                <div class="modelrow">
                                    <code style="font-size:12.5px">{mid.clone()}</code>
                                    <button class="btn ghost" on:click=move |_| {
                                        let y = x.clone();
                                        models.update(|list| list.retain(|(i, _)| i != &y));
                                    }>"✕"</button>
                                </div>
                            }
                        }).collect_view().into_any()
                    }
                }}
                {move || (!fetched.get().is_empty()).then(|| view! {
                    <div class="row" style="flex-wrap:wrap;gap:6px">
                        {move || fetched.get().into_iter().map(|m| {
                            let mm = m.clone();
                            view! {
                                <button class="chip fusion" on:click=move |_| {
                                    let m2 = mm.clone();
                                    if !models.get().iter().any(|(i, _)| i == &m2) {
                                        models.update(|list| list.push((m2.clone(), m2.clone())));
                                    }
                                }>{m}</button>
                            }
                        }).collect_view()}
                    </div>
                })}
                <div class="row">
                    <input class="input" placeholder="custom model id" prop:value=move || new_model.get()
                        on:input=move |e| new_model.set(event_target_value(&e))/>
                    <button class="btn" on:click=test_add disabled=move || busy.get()>"Test & Add"</button>
                </div>
                {move || { let o = out.get(); (!o.is_empty()).then(|| view! { <pre class="codeblock">{o}</pre> }) }}
            </div>
        </div>
    }
}

#[component]
fn UsageView() -> impl IntoView {
    let period = RwSignal::new("today".to_string());
    let tab = RwSignal::new("overview".to_string());
    let summary: RwSignal<Option<Value>> = RwSignal::new(None);
    let logs: RwSignal<Vec<Value>> = RwSignal::new(Vec::new());

    Effect::new(move |_| {
        let p = period.get();
        spawn_local(async move {
            if let Some(v) = fetch_json(&format!("/api/usage/summary?period={p}")).await {
                summary.set(Some(v));
            }
        });
    });
    let load_logs = move || {
        spawn_local(async move {
            if let Some(v) = fetch_json("/api/usage/logs?limit=200").await {
                logs.set(v["logs"].as_array().cloned().unwrap_or_default());
            }
        });
    };
    load_logs();

    let periods = ["today", "24h", "7d", "30d", "60d"];

    view! {
        <div class="card">
            <div class="row" style="justify-content:space-between;flex-wrap:wrap;gap:10px">
                <div class="row">
                    <button class=move || if tab.get() == "overview" { "btn" } else { "btn ghost" }
                        on:click=move |_| tab.set("overview".to_string())>"Overview"</button>
                    <button class=move || if tab.get() == "logs" { "btn" } else { "btn ghost" }
                        on:click=move |_| { tab.set("logs".to_string()); load_logs(); }>"Logs"</button>
                </div>
                {move || (tab.get() == "overview").then(|| view! {
                    <div class="row" style="flex-wrap:wrap;gap:6px">
                        {periods.iter().map(|&p| {
                            let set_v = p.to_string();
                            let cmp_v = p.to_string();
                            let label = p.to_uppercase();
                            view! {
                                <button
                                    class=move || if period.get() == cmp_v { "chip round-robin" } else { "chip" }
                                    on:click=move |_| period.set(set_v.clone())>{label}</button>
                            }
                        }).collect_view()}
                    </div>
                })}
            </div>
        </div>
        {move || if tab.get() == "overview" {
            view! {
                <div class="card">
                    <h3><span>"📊"</span>"Overview"</h3>
                    <div class="grid">
                        <div class="modelrow"><span class="muted">"Requests"</span>
                            <b>{move || summary.get().map(|s| s["totals"]["requests"].as_u64().unwrap_or(0)).unwrap_or(0).to_string()}</b></div>
                        <div class="modelrow"><span class="muted">"Total tokens"</span>
                            <b>{move || summary.get().map(|s| s["totals"]["total_tokens"].as_u64().unwrap_or(0)).unwrap_or(0).to_string()}</b></div>
                        <div class="modelrow"><span class="muted">"Prompt / Completion"</span>
                            <b>{move || summary.get().map(|s| format!("{} / {}",
                                s["totals"]["prompt_tokens"].as_u64().unwrap_or(0),
                                s["totals"]["completion_tokens"].as_u64().unwrap_or(0))).unwrap_or_default()}</b></div>
                    </div>
                </div>
                <div class="card">
                    <h3><span>"🎯"</span>"By target"</h3>
                    <div class="grid">
                        {move || {
                            let rows = summary.get().and_then(|s| s["by_target"].as_array().cloned()).unwrap_or_default();
                            if rows.is_empty() {
                                view! { <span class="muted">"No requests in this period yet."</span> }.into_any()
                            } else {
                                rows.into_iter().map(|r| view! {
                                    <div class="modelrow">
                                        <code style="font-size:12.5px">{r["target"].as_str().unwrap_or("").to_string()}</code>
                                        <span class="muted">{format!("{} req · {} tok",
                                            r["requests"].as_u64().unwrap_or(0), r["total_tokens"].as_u64().unwrap_or(0))}</span>
                                    </div>
                                }).collect_view().into_any()
                            }
                        }}
                    </div>
                </div>
            }.into_any()
        } else {
            view! {
                <div class="card">
                    <div class="row" style="justify-content:space-between">
                        <h3 style="margin:0"><span>"🧾"</span>"Request log"</h3>
                        <button class="btn ghost" on:click=move |_| load_logs()>"Refresh"</button>
                    </div>
                    <div class="grid" style="margin-top:14px">
                        {move || {
                            let rows = logs.get();
                            if rows.is_empty() {
                                view! { <span class="muted">"No requests logged yet."</span> }.into_any()
                            } else {
                                rows.into_iter().map(|e| {
                                    let ts = e["ts"].as_u64().unwrap_or(0);
                                    let target = e["target"].as_str().unwrap_or("").to_string();
                                    let tok = e["total_tokens"].as_u64().unwrap_or(0);
                                    let status = e["status"].as_u64().unwrap_or(0);
                                    let stream = e["stream"].as_bool().unwrap_or(false);
                                    view! {
                                        <div class="modelrow">
                                            <div>
                                                <code style="font-size:12.5px">{target}</code>
                                                <div class="muted" style="margin-top:4px">
                                                    {fmt_time(ts)}{if stream { "  ·  stream" } else { "" }}
                                                </div>
                                            </div>
                                            <span class="muted">{format!("{tok} tok · {status}")}</span>
                                        </div>
                                    }
                                }).collect_view().into_any()
                            }
                        }}
                    </div>
                </div>
            }.into_any()
        }}
    }
}

fn log_style(line: &str) -> &'static str {
    if line.starts_with("[ERROR]") {
        "color:#ff6b6b"
    } else if line.starts_with("[WARN]") {
        "color:#f0c050"
    } else if line.starts_with("[INFO]") {
        "color:#6ea8fe"
    } else if line.starts_with("[DEBUG]") {
        "color:#c792ea"
    } else {
        "color:#5fd0a8"
    }
}

#[component]
fn ConsoleLogView() -> impl IntoView {
    let logs: RwSignal<Vec<String>> = RwSignal::new(Vec::new());
    let connected = RwSignal::new(false);
    let pre_ref: NodeRef<leptos::html::Pre> = NodeRef::new();

    // Open the SSE stream once on mount; keep the EventSource alive in the task.
    if let Ok(mut es) = gloo_net::eventsource::futures::EventSource::new("/api/console-logs/stream")
    {
        if let Ok(mut sub) = es.subscribe("message") {
            spawn_local(async move {
                let _hold = es; // dropping would close the connection
                while let Some(item) = sub.next().await {
                    let Ok((_t, msg)) = item else { continue };
                    let Some(data) = msg.data().as_string() else { continue };
                    let Ok(v) = serde_json::from_str::<Value>(&data) else { continue };
                    match v["type"].as_str() {
                        Some("init") => {
                            let lines: Vec<String> = v["logs"]
                                .as_array()
                                .cloned()
                                .unwrap_or_default()
                                .iter()
                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                .collect();
                            logs.set(lines);
                            connected.set(true);
                        }
                        Some("line") => {
                            if let Some(l) = v["line"].as_str() {
                                let l = l.to_string();
                                logs.update(|ls| {
                                    ls.push(l);
                                    let n = ls.len();
                                    if n > 1000 {
                                        ls.drain(0..n - 1000);
                                    }
                                });
                            }
                            connected.set(true);
                        }
                        Some("clear") => logs.set(Vec::new()),
                        _ => {}
                    }
                }
                connected.set(false);
            });
        }
    }

    // Auto-scroll to the newest line.
    Effect::new(move |_| {
        let _ = logs.get();
        if let Some(el) = pre_ref.get() {
            el.set_scroll_top(el.scroll_height());
        }
    });

    let clear = move |_| {
        spawn_local(async move {
            delete_req("/api/console-logs").await;
        });
    };

    view! {
        <div class="card">
            <div class="row" style="justify-content:space-between">
                <h3 style="margin:0"><span>"🖥"</span>"Console log"</h3>
                <div class="row">
                    <span class=move || if connected.get() { "badge ok" } else { "badge" }>
                        {move || if connected.get() { "● live" } else { "○ offline" }}
                    </span>
                    <button class="btn ghost" on:click=clear>"Clear"</button>
                </div>
            </div>
            <pre node_ref=pre_ref class="codeblock"
                style="height:calc(100vh - 260px);max-height:none;margin-top:14px">
                {move || {
                    let ls = logs.get();
                    if ls.is_empty() {
                        view! { <span class="muted">"No console logs yet."</span> }.into_any()
                    } else {
                        ls.into_iter().map(|line| {
                            let st = log_style(&line);
                            view! { <div style=st>{line}</div> }
                        }).collect_view().into_any()
                    }
                }}
            </pre>
        </div>
    }
}

/// A small usage-vs-limit progress bar.
fn quota_bar(label: &str, used: u64, limit: u64) -> impl IntoView {
    let pct = if limit > 0 {
        ((used as f64 / limit as f64) * 100.0).min(100.0)
    } else {
        0.0
    };
    let danger = pct >= 90.0;
    let fill = if danger { "#ff6b6b" } else { "var(--accent)" };
    view! {
        <div style="margin-top:6px">
            <div style="display:flex;justify-content:space-between;font-size:11.5px;color:var(--muted)">
                <span>{label.to_string()}</span>
                <span>{format!("{used} / {limit}")}</span>
            </div>
            <div style="height:8px;background:#0e0e0f;border:1px solid var(--border);border-radius:6px;overflow:hidden;margin-top:3px">
                <div style=format!("height:100%;width:{pct:.0}%;background:{fill}")></div>
            </div>
        </div>
    }
}

#[component]
fn QuotaView() -> impl IntoView {
    let rows: RwSignal<Vec<Value>> = RwSignal::new(Vec::new());
    let load = move || {
        spawn_local(async move {
            if let Some(v) = fetch_json("/api/quota").await {
                rows.set(v["quota"].as_array().cloned().unwrap_or_default());
            }
        });
    };
    load();

    view! {
        <div class="card">
            <div class="row" style="justify-content:space-between">
                <h3 style="margin:0"><span>"📈"</span>"Provider quotas"</h3>
                <button class="btn ghost" on:click=move |_| load()>"Refresh"</button>
            </div>
            <p class="muted" style="margin:6px 0 0">
                "Rate-limit headers are captured from upstream responses; 24h usage is computed from the request log."
            </p>
            <div class="grid" style="margin-top:14px">
                {move || {
                    let rs = rows.get();
                    if rs.is_empty() {
                        return view! {
                            <span class="muted">"No providers configured."</span>
                        }.into_any();
                    }
                    rs.into_iter().map(|q| {
                        let id = q["id"].as_str().unwrap_or("").to_string();
                        let reqs = q["requests_24h"].as_u64().unwrap_or(0);
                        let toks = q["tokens_24h"].as_u64().unwrap_or(0);
                        let status = q["last_status"].as_u64().unwrap_or(0);
                        let retry = q["retry_after"].as_str().unwrap_or("").to_string();
                        let req_bar = match (
                            q["limit_requests"].as_u64(),
                            q["remaining_requests"].as_u64(),
                        ) {
                            (Some(l), Some(r)) if l > 0 => {
                                Some(quota_bar("requests", l.saturating_sub(r), l))
                            }
                            _ => None,
                        };
                        let tok_bar = match (
                            q["limit_tokens"].as_u64(),
                            q["remaining_tokens"].as_u64(),
                        ) {
                            (Some(l), Some(r)) if l > 0 => {
                                Some(quota_bar("tokens", l.saturating_sub(r), l))
                            }
                            _ => None,
                        };
                        let no_limits = req_bar.is_none() && tok_bar.is_none();
                        let (badge_cls, badge_txt) = if status == 0 {
                            ("chip", "idle".to_string())
                        } else if status >= 400 {
                            ("chip fallback", format!("HTTP {status}"))
                        } else {
                            ("chip round-robin", format!("HTTP {status}"))
                        };
                        view! {
                            <div class="card" style="background:#0f0f10">
                                <div class="row" style="justify-content:space-between">
                                    <b>{id}</b>
                                    <span class=badge_cls>{badge_txt}</span>
                                </div>
                                <div class="muted" style="font-size:12.5px;margin-top:4px">
                                    {format!("24h: {reqs} requests · {toks} tokens")}
                                </div>
                                {req_bar}
                                {tok_bar}
                                {no_limits.then(|| view! {
                                    <div class="muted" style="font-size:11.5px;margin-top:6px">
                                        "No rate-limit headers seen yet."
                                    </div>
                                })}
                                {(!retry.is_empty()).then(|| view! {
                                    <div style="font-size:11.5px;margin-top:6px;color:#f0c050">
                                        {format!("retry-after: {retry}")}
                                    </div>
                                })}
                            </div>
                        }
                    }).collect_view().into_any()
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
    let target = v["x_myc_target"].as_str().unwrap_or("?");
    let fused = v["x_myc_fused"].as_bool().unwrap_or(false);
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

async fn post_json(url: &str, body: &Value) -> Option<Value> {
    Request::post(url)
        .json(body)
        .ok()?
        .send()
        .await
        .ok()?
        .json::<Value>()
        .await
        .ok()
}

async fn delete_req(url: &str) -> Option<Value> {
    Request::delete(url).send().await.ok()?.json::<Value>().await.ok()
}

fn fmt_test(r: Option<Value>) -> String {
    match r {
        Some(v) if v["ok"].as_bool().unwrap_or(false) => format!(
            "\u{2713} {} ({} ms)",
            v["target"].as_str().unwrap_or("ok"),
            v["latency_ms"].as_u64().unwrap_or(0)
        ),
        Some(v) => format!(
            "\u{2717} {} [{}]",
            v["error"].as_str().unwrap_or("failed"),
            v["status"].as_u64().unwrap_or(0)
        ),
        None => "\u{2717} network error".to_string(),
    }
}

/// Format an epoch-seconds timestamp as a local time string (browser locale).
fn fmt_time(ts: u64) -> String {
    let d = js_sys::Date::new(&JsValue::from_f64((ts as f64) * 1000.0));
    d.to_locale_time_string("en-US")
        .as_string()
        .unwrap_or_default()
}
