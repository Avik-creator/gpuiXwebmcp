use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use gpui::{
    actions, div, prelude::*, px, rgb, size, App, Application, Bounds, ClipboardItem, Context,
    ScrollHandle,
    Entity, FocusHandle, Focusable, KeyBinding, TitlebarOptions, Window,
    WindowBounds, WindowOptions,
};
use serde_json::Value;
use webmcp_protocol::{
    find_page_for_url, normalize_inspect_url, origin_from_http_url, BrowserEvent, ConnectionStatus,
    DebuggerCommand, DebuggerState, EventKind, ExecutionId, LogEvent, PageId,
    ToolExecution,
};

mod demo;
mod diff;
mod fixture;
mod form;
mod input;
mod palette;
mod place;
mod schema;
mod shell;
mod theme;
mod walkthrough;

use debugger::ws::{BridgeEvent, ChromeBridge};
use fixture::{FixtureBackend, ToolBackend};
use input::{bind_text_input_keys, TextInput, TextInputEvent};
use palette::{Command, Hit, PageHit, ToolHit, COMMANDS};
use shell::{Altitude, Nav, Place};
use theme::mono;

const EXECUTE_TIMEOUT: Duration = Duration::from_secs(15);
const BRIDGE_POLL: Duration = Duration::from_millis(50);

actions!(
    debugger,
    [
        ExecuteTool,
        ToggleBackend,
        CopyResult,
        GoSurvey,
        GoCompose,
        GoRecord,
        GoBack,
        GoForward,
        ToggleAltitude,
        CancelExecution,
        EditArguments,
        TogglePalette,
        PaletteUp,
        PaletteDown,
        PaletteDismiss,
        ToggleTheme,
        FocusSite,
        OpenDemoSite,
        FocusNextField,
        FocusPrevField,
    ]
);

/// Turn one table entry into the binding the app actually listens for.
///
/// The match is exhaustive, so a command added to the palette table cannot be
/// shipped without a working shortcut — the compiler refuses.
fn binding_for(entry: &palette::Entry) -> KeyBinding {
    let key = entry.binding;
    let scope = Some("Debugger");
    match entry.command {
        Command::Go(Place::Survey) => KeyBinding::new(key, GoSurvey, scope),
        Command::Go(Place::Compose) => KeyBinding::new(key, GoCompose, scope),
        Command::Go(Place::Record) => KeyBinding::new(key, GoRecord, scope),
        Command::Back => KeyBinding::new(key, GoBack, scope),
        Command::Forward => KeyBinding::new(key, GoForward, scope),
        Command::FocusSite => KeyBinding::new(key, FocusSite, scope),
        Command::OpenDemo => KeyBinding::new(key, OpenDemoSite, scope),
        Command::ToggleBackend => KeyBinding::new(key, ToggleBackend, scope),
        Command::ToggleTheme => KeyBinding::new(key, ToggleTheme, scope),
        Command::ToggleAltitude => KeyBinding::new(key, ToggleAltitude, scope),
        Command::Execute => KeyBinding::new(key, ExecuteTool, scope),
        Command::Cancel => KeyBinding::new(key, CancelExecution, scope),
        Command::Copy => KeyBinding::new(key, CopyResult, scope),
    }
}

fn bind_debugger_keys(cx: &mut App) {
    let mut keys: Vec<KeyBinding> = COMMANDS.iter().map(binding_for).collect();
    // Keys that drive the chrome itself rather than a listed command.
    keys.extend([
        KeyBinding::new("ctrl-enter", ExecuteTool, Some("Debugger")),
        KeyBinding::new("cmd-k", TogglePalette, Some("Debugger")),
        KeyBinding::new("cmd-[", GoBack, Some("Debugger")),
        KeyBinding::new("cmd-]", GoForward, Some("Debugger")),
        KeyBinding::new("up", PaletteUp, Some("Debugger")),
        KeyBinding::new("down", PaletteDown, Some("Debugger")),
        KeyBinding::new("escape", PaletteDismiss, Some("Debugger")),
        KeyBinding::new("tab", FocusNextField, Some("Debugger")),
        KeyBinding::new("shift-tab", FocusPrevField, Some("Debugger")),
    ]);
    cx.bind_keys(keys);
}

/// What the site field is doing right now.
///
/// Every one of these used to be a line in the history screen, which is not
/// where you are looking when you press Open. They belong next to the field.
#[derive(Clone, Debug, PartialEq)]
enum SiteStatus {
    Idle,
    Opening(String),
    Problem(String),
}

/// Where the tools on screen come from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Source {
    /// A real browser, over the extension.
    Chrome,
    /// Built-in sample data, so the app can be learned with no browser at all.
    Playground,
}

struct PendingExecution {
    id: ExecutionId,
    /// The tab the run went to, so a cancel reaches it even after you switch pages.
    page_id: Option<PageId>,
}

struct Debugger {
    state: DebuggerState,
    form: schema::Form,
    form_schema: Value,
    form_tool: Option<String>,
    raw: form::Raw,
    inputs: BTreeMap<String, Entity<TextInput>>,
    compose_editing: bool,
    comparing: bool,
    palette_open: bool,
    palette_query: Entity<TextInput>,
    palette_index: usize,
    palette_run_pending: bool,
    seen_history: bool,
    walkthrough_hidden: bool,
    pending: Option<PendingExecution>,
    execution_seq: u64,
    run_id: String,
    nav: Nav,
    altitude: Altitude,
    record_scroll: ScrollHandle,
    seen_events: usize,
    source: Source,
    bridge: Option<ChromeBridge>,
    extension_clients: usize,
    site_input: Entity<TextInput>,
    pending_site: Option<String>,
    site_status: SiteStatus,
    playground: bool,
    demo: Option<demo::DemoSite>,
    demo_problem: Option<String>,
    focus: FocusHandle,
}

impl Debugger {
    fn new(cx: &mut Context<Self>) -> Self {
        let (bridge, bind_error) = match ChromeBridge::bind() {
            Ok(bridge) => (Some(bridge), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let site_input = cx.new(|cx| TextInput::new(cx, "http://localhost:5173").confirmable());
        let palette_query = cx.new(|cx| TextInput::new(cx, "go to, run, or find a tool").confirmable());
        cx.observe(&palette_query, |this, _, cx| {
            this.palette_index = 0;
            cx.notify();
        })
        .detach();
        cx.subscribe(&palette_query, |this, _, event, cx| match event {
            TextInputEvent::Confirm => this.run_palette_deferred(cx),
        })
        .detach();
        cx.observe(&site_input, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&site_input, |this, _, event, cx| match event {
            TextInputEvent::Confirm => this.open_site(cx),
        })
        .detach();
        let mut this = Self {
            state: DebuggerState::waiting_for_extension(),
            form: schema::Form::Raw { reason: "no tool".into() },
            form_schema: Value::Null,
            form_tool: None,
            raw: form::Raw::default(),
            inputs: BTreeMap::new(),
            compose_editing: true,
            comparing: false,
            palette_open: false,
            palette_query,
            palette_index: 0,
            palette_run_pending: false,
            seen_history: false,
            walkthrough_hidden: false,
            pending: None,
            execution_seq: 0,
            run_id: new_run_id(),
            nav: Nav::new(Place::Survey),
            altitude: Altitude::Runs,
            record_scroll: ScrollHandle::default(),
            seen_events: 0,
            source: Source::Chrome,
            bridge,
            extension_clients: 0,
            site_input,
            pending_site: None,
            site_status: SiteStatus::Idle,
            // Serving the demo site ourselves removes the "run python in another
            // window first" step, and removes the dependency on python existing.
            playground: false,
            demo: None,
            demo_problem: None,
            focus: cx.focus_handle(),
        };
        if let Some(error) = bind_error {
            this.state.events.push(LogEvent {
                timestamp: Utc::now(),
                kind: EventKind::Disconnected,
                message: format!("ws bind failed: {error}"),
            });
        }
        this.rebuild_form(cx);
        this.start_bridge_pump(cx);
        this
    }

    fn start_bridge_pump(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, async_cx| loop {
            async_cx.background_executor().timer(BRIDGE_POLL).await;
            if this
                .update(async_cx, |this, cx| this.drain_bridge(cx))
                .is_err()
            {
                break;
            }
        })
        .detach();
    }

    fn drain_bridge(&mut self, cx: &mut Context<Self>) {
        let Some(bridge) = &self.bridge else {
            return;
        };
        let events = bridge.poll();
        if events.is_empty() {
            return;
        }
        let mut rebuild = false;
        let mut clear_pending = false;
        for event in events {
            match event {
                BridgeEvent::ClientsChanged { connected } => {
                    self.extension_clients = connected;
                    if self.source == Source::Chrome
                        && connected == 0
                        && self.state.connection != ConnectionStatus::Disconnected
                    {
                        let _ = self.state.apply_browser_event(BrowserEvent::Disconnected {
                            timestamp: Utc::now(),
                        });
                    }
                    // A run in flight when the browser goes away can never
                    // complete. Fail it now instead of leaving the UI on
                    // WORKING until the 15s timer expires.
                    if self.source == Source::Chrome && connected == 0 {
                        if let Some(id) = self.pending.as_ref().map(|pending| pending.id.clone()) {
                            self.complete_execution(
                                id,
                                Err("extension disconnected mid-run".to_string()),
                                cx,
                            );
                        }
                    }
                }
                BridgeEvent::Unparsable { reason, raw } => {
                    self.state.record_protocol_error(&reason, &raw, Utc::now());
                }
                BridgeEvent::Browser(browser) => {
                    if self.source != Source::Chrome {
                        continue;
                    }
                    self.focus_pending_site(&browser);
                    let finishes_pending = match &browser {
                        BrowserEvent::ToolExecutionFinished { execution_id, .. }
                        | BrowserEvent::ToolExecutionFailed { execution_id, .. } => self
                            .pending
                            .as_ref()
                            .is_some_and(|pending| pending.id == *execution_id),
                        BrowserEvent::Hello { .. }
                        | BrowserEvent::PageChanged { .. }
                        | BrowserEvent::ToolsChanged { .. }
                        | BrowserEvent::ToolExecutionStarted { .. }
                        | BrowserEvent::PageClosed { .. }
                        | BrowserEvent::Disconnected { .. } => false,
                    };
                    if self.state.apply_browser_event(browser) {
                        rebuild = true;
                    }
                    if finishes_pending {
                        clear_pending = true;
                    }
                }
            }
        }
        if clear_pending {
            self.pending = None;
        }
        if rebuild {
            self.rebuild_form(cx);
        }
        cx.notify();
    }

    fn toggle_backend(&mut self, cx: &mut Context<Self>) {
        if self.playground {
            self.leave_playground(cx);
        } else {
            self.enter_playground(cx);
        }
    }

    /// Learn the app with the app providing the subject.
    ///
    /// Always opens on the built-in sample tools, so it cannot land you on an
    /// empty screen. Auto-opening the demo page instead meant that if Chrome
    /// could not register tools — no WebMCP flag, wrong Chrome — the playground
    /// showed nothing at all, which is the one thing it must never do. The demo
    /// page is offered as a step up from here, not a prerequisite.
    fn enter_playground(&mut self, cx: &mut Context<Self>) {
        self.playground = true;
        self.seen_history = false;
        self.walkthrough_hidden = false;
        self.pending = None;
        self.comparing = false;
        self.pending_site = None;
        self.site_status = SiteStatus::Idle;

        match start_demo_site() {
            Ok(site) => {
                self.demo_problem = None;
                self.demo = Some(site);
            }
            Err(problem) => {
                self.demo = None;
                self.demo_problem = Some(problem);
            }
        }

        self.source = Source::Playground;
        self.state = FixtureBackend.snapshot();
        self.state.selected_tool = None;
        self.rebuild_form(cx);
        self.navigate(Place::Survey);
        cx.notify();
    }

    /// Leaving stops the demo server; nothing of the playground outlives it.
    fn leave_playground(&mut self, cx: &mut Context<Self>) {
        self.playground = false;
        self.demo = None; // Drop stops the server.
        self.demo_problem = None;
        self.switch_to_live(cx);
        self.navigate(Place::Survey);
        cx.notify();
    }

    fn chrome_connected(&self) -> bool {
        self.extension_clients > 0 && self.bridge.is_some()
    }

    fn in_playground(&self) -> bool {
        self.playground
    }

    fn walkthrough_step(&self) -> walkthrough::Step {
        let picked = self.state.selected_tool.is_some();
        let has_result = self
            .state
            .executions
            .iter()
            .any(|run| run.result.is_some() || run.error.is_some());
        walkthrough::step(picked, has_result, self.seen_history)
    }

    fn switch_to_live(&mut self, cx: &mut Context<Self>) {
        self.source = Source::Chrome;
        self.pending = None;
        self.pending_site = None;
        self.state = DebuggerState::waiting_for_extension();
        if self.extension_clients > 0 {
            self.state.connection = ConnectionStatus::Connected;
        }
        self.rebuild_form(cx);
    }

    fn focus_pending_site(&mut self, browser: &BrowserEvent) {
        let Some(want) = self.pending_site.as_deref() else {
            return;
        };
        let (page_id, origin, url) = match browser {
            BrowserEvent::PageChanged { page, .. } => {
                (page.id.clone(), page.origin.as_str(), page.url.as_str())
            }
            BrowserEvent::ToolsChanged {
                page_id,
                origin,
                url,
                ..
            } => (page_id.clone(), origin.as_str(), url.as_str()),
            BrowserEvent::Hello { .. }
            | BrowserEvent::ToolExecutionStarted { .. }
            | BrowserEvent::ToolExecutionFinished { .. }
            | BrowserEvent::ToolExecutionFailed { .. }
            | BrowserEvent::PageClosed { .. }
            | BrowserEvent::Disconnected { .. } => return,
        };
        let origin_hit = origin.eq_ignore_ascii_case(want);
        let url_hit = url == want
            || origin_from_http_url(url)
                .as_deref()
                .is_some_and(|got| got.eq_ignore_ascii_case(want));
        if origin_hit || url_hit {
            self.state.selected_page = Some(page_id);
            self.pending_site = None;
            self.site_status = SiteStatus::Idle;
        }
    }


    /// How long to wait for a page to report its tools before saying so.
    const SITE_TIMEOUT: Duration = Duration::from_secs(12);

    /// The address the bundled demo site is being served on, if it is running.
    /// Only ever while the playground is open — it is part of the playground,
    /// not something squatting on a port for the whole session.
    fn demo_url(&self) -> Option<&str> {
        self.demo.as_ref().map(|site| site.url.as_str())
    }

    /// Why it did not start, if it did not. Swallowing this left the link simply
    /// missing, with nothing on screen to explain the absence.
    fn demo_problem(&self) -> Option<&str> {
        self.demo_problem.as_deref()
    }

    /// Put the demo address in the field and open it, so trying the tool is one
    /// click rather than copy, paste, switch window.
    /// Step up from sample tools to the real thing: the demo page, in Chrome,
    /// reporting its tools over the extension like any other site.
    fn open_demo_site(&mut self, cx: &mut Context<Self>) {
        if !self.playground {
            // The demo site belongs to the playground, so asking for it asks for that.
            self.enter_playground(cx);
            return;
        }
        let Some(url) = self.demo_url().map(str::to_string) else {
            return;
        };
        self.site_input
            .update(cx, |input, cx| input.set_text_notify(url, cx));
        self.open_site(cx);
    }

    fn open_site(&mut self, cx: &mut Context<Self>) {
        let raw = self.site_input.read(cx).text();
        let url = match normalize_inspect_url(&raw) {
            Ok(url) => url,
            Err(error) => {
                self.site_status = SiteStatus::Problem(error);
                cx.notify();
                return;
            }
        };

        // Already know this one — just go there.
        if let Some(page) = find_page_for_url(&self.state.pages, &url).cloned() {
            self.pending_site = None;
            self.site_status = SiteStatus::Idle;
            self.select_page(page.id, cx);
            if let Some(bridge) = &self.bridge {
                if self.source == Source::Chrome {
                    let _ = bridge.send(&DebuggerCommand::OpenPage { url });
                }
            }
            cx.notify();
            return;
        }

        // Opening a site needs a real browser. Say so here, not in the history.
        if self.bridge.is_none() {
            self.site_status = SiteStatus::Problem(
                "Port 17321 is already in use, so Chrome cannot connect. Close the other debugger and restart this one.".into(),
            );
            cx.notify();
            return;
        }
        if self.extension_clients == 0 {
            self.site_status = SiteStatus::Problem(
                "Chrome is not connected. Open Chrome with the extension loaded, or press ⌃T to use the built-in sample data.".into(),
            );
            cx.notify();
            return;
        }
        if self.source == Source::Playground {
            self.switch_to_live(cx);
        }

        self.pending_site = origin_from_http_url(&url).or_else(|| Some(url.clone()));
        self.state.tools.clear();
        self.state.selected_tool = None;
        self.rebuild_form(cx);

        let sent = self
            .bridge
            .as_ref()
            .is_some_and(|bridge| bridge.send(&DebuggerCommand::OpenPage { url: url.clone() }));
        if !sent {
            self.pending_site = None;
            self.site_status =
                SiteStatus::Problem("Could not reach the Chrome extension.".into());
            cx.notify();
            return;
        }

        self.site_status = SiteStatus::Opening(url.clone());
        self.navigate(Place::Survey);
        // Chrome will focus or open the tab. If nothing comes back, the page
        // probably does not use WebMCP — which is worth saying out loud rather
        // than leaving an empty screen.
        cx.spawn(async move |this, async_cx| {
            async_cx.background_executor().timer(Self::SITE_TIMEOUT).await;
            this.update(async_cx, |this, cx| {
                if this.site_status == SiteStatus::Opening(url.clone()) {
                    this.pending_site = None;
                    this.site_status = SiteStatus::Problem(format!(
                        "Opened {url}, but the page has not reported any tools. It may not use WebMCP, or the tab may need reloading."
                    ));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Tools come from the cache, so the list never blanks while a refresh is
    /// in flight. The subscribe still goes out, to pick up anything that changed.
    fn select_page(&mut self, id: PageId, cx: &mut Context<Self>) {
        if !self.state.select_page(id.clone()) {
            return;
        }
        self.rebuild_form(cx);
        if self.source == Source::Chrome {
            if let Some(bridge) = &self.bridge {
                let _ = bridge.send(&DebuggerCommand::SubscribePage { page_id: id });
            }
        }
        cx.notify();
    }

    fn select_tool(&mut self, name: String, cx: &mut Context<Self>) {
        self.navigate(Place::Compose);
        if self.state.selected_tool.as_deref() == Some(name.as_str()) {
            return;
        }
        self.state.selected_tool = Some(name);
        self.rebuild_form(cx);
        cx.notify();
    }

    /// Ready when nothing is in flight, a tool is chosen, the backend can reach
    /// it, and the form has no complaints. One source of truth for the last part:
    /// the same errors the fields display.
    fn can_execute(&self, cx: &App) -> bool {
        if self.pending.is_some() || self.state.selected_tool.is_none() {
            return false;
        }
        if self.source == Source::Chrome && self.state.connection != ConnectionStatus::Connected
        {
            return false;
        }
        self.form_errors(cx).is_empty()
    }

    fn execute_selected(&mut self, cx: &mut Context<Self>) {
        if !self.can_execute(cx) {
            return;
        }
        let Some(tool) = self.state.selected_tool.clone() else {
            return;
        };
        let Ok(arguments) = self.collect_arguments(cx) else {
            return;
        };
        self.execution_seq += 1;
        let id = ExecutionId::from(format!("exec_{}_{}", self.run_id, self.execution_seq));
        self.state.record_execution_started(ToolExecution {
            id: id.clone(),
            tool_name: tool.clone(),
            arguments: arguments.clone(),
            result: None,
            error: None,
            started_at: Utc::now(),
            finished_at: None,
        });
        self.pending = Some(PendingExecution {
            id: id.clone(),
            page_id: self.state.selected_page.clone(),
        });
        self.compose_editing = false;
        self.comparing = false;
        self.navigate(Place::Compose);
        cx.notify();
        match self.source {
            Source::Playground => {
                let delay = fixture_delay();
                cx.spawn(async move |this, async_cx| {
                    async_cx.background_executor().timer(delay).await;
                    let result = FixtureBackend.execute(&tool, &arguments);
                    this.update(async_cx, |this, cx| {
                        this.complete_execution(id, result, cx);
                    })
                    .ok();
                })
                .detach();
            }
            Source::Chrome => {
                let Some(page_id) = self.state.selected_page.clone() else {
                    self.complete_execution(id, Err("no page selected".to_string()), cx);
                    return;
                };
                let sent = self.bridge.as_ref().is_some_and(|bridge| {
                    bridge.send(&DebuggerCommand::ExecuteTool {
                        page_id,
                        tool,
                        arguments,
                        execution_id: id.clone(),
                    })
                });
                if !sent {
                    self.complete_execution(id, Err("extension not connected".to_string()), cx);
                    return;
                }
                cx.spawn(async move |this, async_cx| {
                    async_cx.background_executor().timer(EXECUTE_TIMEOUT).await;
                    this.update(async_cx, |this, cx| {
                        if this
                            .pending
                            .as_ref()
                            .is_some_and(|pending| pending.id == id)
                        {
                            this.complete_execution(
                                id,
                                Err("timed out waiting for Chrome".to_string()),
                                cx,
                            );
                        }
                    })
                    .ok();
                })
                .detach();
            }
        }
    }

    fn complete_execution(
        &mut self,
        id: ExecutionId,
        result: Result<Value, String>,
        cx: &mut Context<Self>,
    ) {
        let matches_pending = self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.id == id);
        if !matches_pending {
            // A local timer landing after the run was already settled. Record it
            // rather than discard it silently.
            let note = match &result {
                Ok(_) => format!("local result for {} arrived after it settled", id.as_str()),
                Err(error) => format!(
                    "local failure for {} arrived after it settled ({error})",
                    id.as_str()
                ),
            };
            self.state.record_late_result(note, Utc::now());
            cx.notify();
            return;
        }
        let finished_at = Utc::now();
        match result {
            Ok(value) => self
                .state
                .record_execution_finished(&id, value, finished_at),
            Err(error) => self.state.record_execution_failed(&id, error, finished_at),
        }
        self.pending = None;
        cx.notify();
    }

    /// Rebuild only when the tool or its schema actually changed.
    /// A background refresh must not wipe typing; two same-shaped tools must not share a form.
    fn rebuild_form(&mut self, cx: &mut Context<Self>) {
        let tool = self.state.selected_tool.clone();
        let schema = self
            .state
            .selected_tool()
            .map(|tool| tool.input_schema.clone())
            .unwrap_or(Value::Null);
        if schema == self.form_schema && tool == self.form_tool {
            return;
        }
        self.form_tool = tool;
        self.form_schema = schema.clone();
        self.form = schema::form_from_schema(&schema);
        self.raw = form::Raw::default();
        form::seed(self.form.fields(), "", &mut self.raw);
        self.inputs.clear();
        self.compose_editing = true;
        cx.notify();
    }

    /// A text field per path, created once and kept. Never rebuilt mid-edit.
    fn ensure_input(
        &mut self,
        path: &str,
        placeholder: &str,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        if let Some(existing) = self.inputs.get(path) {
            return existing.clone();
        }
        let seeded = self.raw.text.get(path).cloned();
        let placeholder = placeholder.to_string();
        let entity = cx.new(|cx| {
            let mut input = TextInput::new(cx, placeholder);
            if let Some(text) = seeded {
                input.set_text(text);
            }
            input
        });
        cx.observe(&entity, |this, _, cx| {
            this.compose_editing = true;
            cx.notify();
        })
        .detach();
        self.inputs.insert(path.to_string(), entity.clone());
        entity
    }

    /// Widget state with the live text folded in.
    fn snapshot_raw(&self, cx: &App) -> form::Raw {
        let mut raw = self.raw.clone();
        for (path, entity) in &self.inputs {
            raw.text.insert(path.clone(), entity.read(cx).text());
        }
        raw
    }

    fn collect_arguments(&self, cx: &App) -> Result<Value, String> {
        let raw = self.snapshot_raw(cx);
        let (value, errors) = form::assemble(self.form.fields(), &raw, "");
        match errors.first() {
            Some(error) => Err(format!("{}: {}", error.path, error.message)),
            None => Ok(value),
        }
    }

    fn form_errors(&self, cx: &App) -> Vec<schema::FieldError> {
        let raw = self.snapshot_raw(cx);
        let (value, mut errors) = form::assemble(self.form.fields(), &raw, "");
        errors.extend(schema::validate(&self.form, &value));
        errors
    }

    /// Widgets move with their rows: everything behind the removed one is
    /// renumbered, nested paths included, and the row's own widgets are dropped.
    fn drop_list_row(&mut self, path: &str, index: usize, cx: &mut Context<Self>) {
        if index >= self.raw.list_len(path) {
            return;
        }
        // Live text lives in the widgets; fold it in before anything renumbers.
        self.raw = self.snapshot_raw(cx);
        self.raw.drop_row(path, index);
        self.inputs = form::rekey_map(std::mem::take(&mut self.inputs), path, index);
        self.compose_editing = true;
        cx.notify();
    }

    fn toggle_compare(&mut self, cx: &mut Context<Self>) {
        self.comparing = !self.comparing;
        cx.notify();
    }

    fn edit_arguments(&mut self, cx: &mut Context<Self>) {
        self.compose_editing = true;
        self.navigate(Place::Compose);
        cx.notify();
    }

    /// Ask the page to abort, then close the run here.
    ///
    /// Where the browser passes `execute` an `AbortSignal` this is a real abort.
    /// Where it does not, the tab keeps going and only we stop waiting — so the
    /// recorded reason says so rather than claiming the tool stopped.
    fn cancel_execution(&mut self, cx: &mut Context<Self>) {
        let Some((id, page_id)) = self
            .pending
            .as_ref()
            .map(|pending| (pending.id.clone(), pending.page_id.clone()))
        else {
            return;
        };
        let asked = match (&self.bridge, page_id) {
            (Some(bridge), Some(page_id)) if self.source == Source::Chrome => {
                bridge.send(&DebuggerCommand::CancelExecution {
                    page_id,
                    execution_id: id.clone(),
                })
            }
            _ => false,
        };
        let reason = if asked {
            "cancelled — abort sent to the tab"
        } else {
            "cancelled here — the tab may still be running it"
        };
        self.complete_execution(id, Err(reason.to_string()), cx);
    }

    fn palette_hits(&self, cx: &App) -> Vec<Hit> {
        let query = self.palette_query.read(cx).text();
        let tools: Vec<ToolHit> = self
            .state
            .tools
            .iter()
            .map(|tool| ToolHit {
                name: tool.name.clone(),
                mutates: place::survey::access(tool).1,
            })
            .collect();
        let pages: Vec<PageHit> = self
            .state
            .pages
            .iter()
            .map(|page| PageHit {
                id: page.id.as_str().to_string(),
                origin: page.origin.clone(),
                // Every page's tools are cached, so no site is "0 tools" merely
                // because you are not looking at it.
                tools: if Some(&page.id) == self.state.selected_page.as_ref() {
                    self.state.tools.len()
                } else {
                    self.state
                        .tools_by_page
                        .get(page.id.as_str())
                        .map(Vec::len)
                        .unwrap_or(0)
                },
            })
            .collect();
        palette::matches(&query, &tools, &pages)
    }

    fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_open = !self.palette_open;
        self.palette_index = 0;
        if self.palette_open {
            self.palette_query.update(cx, |input, cx| input.set_text_notify("", cx));
            let handle = self.palette_query.read(cx).focus_handle(cx);
            window.focus(&handle);
        } else {
            window.focus(&self.focus);
        }
        cx.notify();
    }

    fn move_palette(&mut self, forward: bool, cx: &mut Context<Self>) {
        if !self.palette_open {
            return;
        }
        let len = self.palette_hits(cx).len();
        self.palette_index = palette::step(self.palette_index, len, forward);
        cx.notify();
    }

    /// Enter arrives as a `Confirm` event from the query field, and that
    /// subscription carries no window. Commands need one to move focus, so the
    /// request is parked and picked up by the next render, which has one.
    fn run_palette_deferred(&mut self, cx: &mut Context<Self>) {
        self.palette_run_pending = true;
        cx.notify();
    }

    fn run_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let hits = self.palette_hits(cx);
        let index = palette::clamp(self.palette_index, hits.len());
        let Some(hit) = hits.get(index).cloned() else {
            return;
        };
        self.palette_open = false;
        window.focus(&self.focus);
        match hit {
            Hit::Tool { name, .. } => self.select_tool(name, cx),
            Hit::Page { id, .. } => self.select_page(PageId::from(id), cx),
            Hit::Command(slot) => self.run_command(COMMANDS[slot].command, window, cx),
        }
        cx.notify();
    }

    fn run_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            Command::Go(place) => self.go_to(place, cx),
            Command::Back => self.go_back(cx),
            Command::Forward => self.go_forward(cx),
            Command::FocusSite => self.focus_site(window, cx),
            Command::OpenDemo => self.open_demo_site(cx),
            Command::ToggleBackend => self.toggle_backend(cx),
            Command::ToggleTheme => self.toggle_theme(cx),
            Command::ToggleAltitude => self.toggle_altitude(cx),
            Command::Execute => self.execute_selected(cx),
            Command::Cancel => self.cancel_execution(cx),
            Command::Copy => self.copy_last_result(cx),
        }
    }

    fn focus_site(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(Place::Survey);
        let handle = self.site_input.read(cx).focus_handle(cx);
        window.focus(&handle);
        cx.notify();
    }

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        let next = theme::mode(cx).flipped();
        cx.set_global(theme::Theme { mode: next });
        cx.notify();
    }

    /// Tab follows the schema's order, not the map's. See `form::ordered_paths`.
    fn focus_field(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open || self.nav.current() != Place::Compose {
            return;
        }
        let paths: Vec<String> = form::ordered_paths(self.form.fields(), &self.raw, "")
            .into_iter()
            .filter(|path| self.inputs.contains_key(path))
            .collect();
        if paths.is_empty() {
            return;
        }
        let current = paths.iter().position(|path| {
            self.inputs
                .get(path)
                .is_some_and(|entity| entity.read(cx).focus_handle(cx).is_focused(window))
        });
        let next = match current {
            Some(index) => palette::step(index, paths.len(), forward),
            None => 0,
        };
        if let Some(entity) = self.inputs.get(&paths[next]) {
            let handle = entity.read(cx).focus_handle(cx);
            window.focus(&handle);
        }
        cx.notify();
    }

    /// Any place, from any screen. Compose needs a tool first.
    fn go_to(&mut self, place: Place, cx: &mut Context<Self>) {
        if !place.reachable(self.state.selected_tool.is_some()) {
            return;
        }
        self.navigate(place);
        cx.notify();
    }

    /// The one place a screen change is recorded, so back always has the truth.
    fn navigate(&mut self, place: Place) {
        if place == Place::Record {
            self.seen_history = true;
        }
        self.nav.go(place);
    }

    fn go_back(&mut self, cx: &mut Context<Self>) {
        // Run stops existing when its tool does; do not send anyone back to it.
        if self.state.selected_tool.is_none() {
            self.nav.forget(Place::Compose);
        }
        if self.nav.back() {
            cx.notify();
        }
    }

    fn go_forward(&mut self, cx: &mut Context<Self>) {
        if self.state.selected_tool.is_none() {
            self.nav.forget(Place::Compose);
        }
        if self.nav.forward() {
            cx.notify();
        }
    }

    fn toggle_altitude(&mut self, cx: &mut Context<Self>) {
        self.altitude = self.altitude.flipped();
        self.seen_history = true;
        self.navigate(Place::Record);
        cx.notify();
    }

    /// What the top-right corner says, in words rather than a status code.
    ///
    /// "LIVE · 1" and "WAIT" meant nothing to anyone who had not read the source.
    fn status_tag(&self) -> (String, bool) {
        match self.source {
            Source::Playground => ("Playground".into(), false),
            Source::Chrome => match self.state.connection {
                ConnectionStatus::Connected if self.extension_clients > 1 => (
                    format!("{} browsers connected", self.extension_clients),
                    true,
                ),
                ConnectionStatus::Connected => ("Chrome connected".into(), false),
                ConnectionStatus::Disconnected | ConnectionStatus::Fixture => {
                    if self.bridge.is_none() {
                        ("Port 17321 in use".into(), true)
                    } else {
                        ("Waiting for Chrome".into(), true)
                    }
                }
            },
        }
    }

    fn copy_last_result(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.state.selected_tool.as_deref() else {
            return;
        };
        let Some(execution) = self.state.last_execution_for(name) else {
            return;
        };
        let text = if let Some(error) = &execution.error {
            error.clone()
        } else if let Some(result) = &execution.result {
            serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".into())
        } else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }
}

impl Focusable for Debugger {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Per-launch prefix for execution ids. The counter used to restart at `exec_1`
/// every run, so a stale reply from a previous process was indistinguishable
/// from a fresh one.
/// Start the bundled demo site, reporting why not rather than failing quietly.
fn start_demo_site() -> Result<demo::DemoSite, String> {
    let Some(root) = demo::find_root() else {
        return Err(
            "Could not find the demo-site folder next to the binary. Set WEBMCP_DEMO_SITE to its path."
                .to_string(),
        );
    };
    match demo::DemoSite::start(root) {
        Ok(site) => {
            eprintln!("demo site serving on {}", site.url);
            Ok(site)
        }
        Err(error) => Err(format!("Could not serve the demo site: {error}")),
    }
}

fn new_run_id() -> String {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::from(elapsed.subsec_nanos()) ^ elapsed.as_secs())
        .unwrap_or(0);
    format!("{seed:08x}")
}

fn fixture_delay() -> Duration {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(0);
    Duration::from_millis(50 + u64::from(nanos % 151))
}

impl Render for Debugger {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.palette_run_pending {
            self.palette_run_pending = false;
            self.run_palette(window, cx);
        }
        let palette = theme::theme(cx);
        // Follow the newest line. Only when the count actually grew, so a user
        // who has scrolled back to read something is not yanked to the bottom
        // by an unrelated repaint.
        if self.nav.current() == Place::Record && self.state.events.len() != self.seen_events {
            self.seen_events = self.state.events.len();
            self.record_scroll.scroll_to_bottom();
        }
        div()
            .key_context("Debugger")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ExecuteTool, _, cx| this.execute_selected(cx)))
            .on_action(cx.listener(|this, _: &ToggleBackend, _, cx| this.toggle_backend(cx)))
            .on_action(cx.listener(|this, _: &CopyResult, _, cx| this.copy_last_result(cx)))
            .on_action(cx.listener(|this, _: &GoSurvey, _, cx| this.go_to(Place::Survey, cx)))
            .on_action(cx.listener(|this, _: &GoCompose, _, cx| this.go_to(Place::Compose, cx)))
            .on_action(cx.listener(|this, _: &GoRecord, _, cx| this.go_to(Place::Record, cx)))
            .on_action(cx.listener(|this, _: &GoBack, _, cx| this.go_back(cx)))
            .on_action(cx.listener(|this, _: &GoForward, _, cx| this.go_forward(cx)))
            .on_action(cx.listener(|this, _: &ToggleAltitude, _, cx| this.toggle_altitude(cx)))
            .on_action(cx.listener(|this, _: &CancelExecution, _, cx| this.cancel_execution(cx)))
            .on_action(cx.listener(|this, _: &EditArguments, _, cx| this.edit_arguments(cx)))
            .on_action(cx.listener(|this, _: &TogglePalette, window, cx| this.toggle_palette(window, cx)))
            .on_action(cx.listener(|this, _: &PaletteUp, _, cx| this.move_palette(false, cx)))
            .on_action(cx.listener(|this, _: &PaletteDown, _, cx| this.move_palette(true, cx)))
            .on_action(cx.listener(|this, _: &PaletteDismiss, window, cx| {
                // Escape is "get me out of here": close the palette if it is
                // open, otherwise step back a screen.
                if this.palette_open {
                    this.toggle_palette(window, cx);
                } else {
                    this.go_back(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleTheme, _, cx| this.toggle_theme(cx)))
            .on_action(cx.listener(|this, _: &FocusSite, window, cx| this.focus_site(window, cx)))
            .on_action(cx.listener(|this, _: &OpenDemoSite, _, cx| this.open_demo_site(cx)))
            .on_action(cx.listener(|this, _: &FocusNextField, window, cx| this.focus_field(true, window, cx)))
            .on_action(cx.listener(|this, _: &FocusPrevField, window, cx| this.focus_field(false, window, cx)))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(palette.paper))
            .text_color(rgb(palette.ink))
            .font(mono())
            .text_size(px(12.))
            .line_height(px(18.))
            .child(shell::bar(self, cx))
            .child(match self.nav.current() {
                Place::Survey => place::survey::render(self, cx).into_any_element(),
                Place::Record => place::record::render(self, cx).into_any_element(),
                Place::Compose => place::compose::render(self, cx).into_any_element(),
            })
            .when(self.palette_open, |el| {
                el.child(palette::overlay(self, palette, cx))
            })
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.set_global(theme::Theme {
            mode: theme::Mode::Dark,
        });
        bind_text_input_keys(cx);
        bind_debugger_keys(cx);
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("WEBMCP".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let debugger = cx.new(Debugger::new);
                window.focus(&debugger.read(cx).focus);
                debugger
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
