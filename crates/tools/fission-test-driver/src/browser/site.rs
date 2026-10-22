use super::{runtime_exception, BrowserController};
use crate::{SelectorFailure, SelectorFailureKind, SelectorQuery, TestCommand, TestResponse};
use anyhow::{Context, Result};
use serde_json::json;
use std::time::{Duration, Instant};

const SITE_COMMAND: &str = r#"(command => {
  const semanticSelector = '[data-fission-node],[data-fission-semantics],[data-testid],[role],[aria-label],[aria-labelledby],button,a[href],input,textarea,select,option,details,summary,table,thead,tbody,tfoot,tr,th,td,img,h1,h2,h3,h4,h5,h6,form,main,nav,header,footer,section,article';
  const all = () => Array.from(document.querySelectorAll(semanticSelector));
  const rect = el => { const r=el.getBoundingClientRect(); return {x:r.x,y:r.y,width:r.width,height:r.height}; };
  const clippedRect = el => {
    const r=el.getBoundingClientRect(), left=Math.max(0,r.left), top=Math.max(0,r.top);
    const right=Math.min(innerWidth,r.right), bottom=Math.min(innerHeight,r.bottom);
    return right>left&&bottom>top ? {x:left,y:top,width:right-left,height:bottom-top} : null;
  };
  const visible = el => {
    const closedDetails=el.closest('details:not([open])');
    if(closedDetails){
      const summary=closedDetails.querySelector(':scope > summary');
      if(!summary||(el!==summary&&!summary.contains(el)))return false;
    }
    const style=getComputedStyle(el), r=el.getBoundingClientRect();
    return !el.hidden && !el.closest('[hidden],[inert],[aria-hidden="true"]') &&
      style.display!=='none' && style.visibility!=='hidden' && Number(style.opacity)!==0 &&
      r.width>0 && r.height>0 && !!clippedRect(el);
  };
  const role = el => el.getAttribute('role') || (el.tagName==='TH' ? (el.getAttribute('scope')==='row'?'rowheader':'columnheader') : ({
    BUTTON:'button',A:'link',TEXTAREA:'textbox',SELECT:'combobox',OPTION:'option',DETAILS:'group',SUMMARY:'button',TABLE:'table',THEAD:'rowgroup',TBODY:'rowgroup',TFOOT:'rowgroup',TR:'row',TD:'cell',IMG:'img',
    H1:'heading',H2:'heading',H3:'heading',H4:'heading',H5:'heading',H6:'heading',
    FORM:'form',MAIN:'main',NAV:'navigation',HEADER:'banner',FOOTER:'contentinfo',SECTION:'region',ARTICLE:'article'
  }[el.tagName] || (el.tagName==='INPUT' ? ({checkbox:'checkbox',radio:'radio',range:'slider',submit:'button',button:'button'}[el.type]||'textbox') : 'generic')));
  const label = el => {
    const aria=el.getAttribute('aria-label'); if(aria)return aria.trim();
    const ids=(el.getAttribute('aria-labelledby')||'').trim().split(/\s+/).filter(Boolean);
    if(ids.length)return ids.map(id=>document.getElementById(id)?.textContent||'').join(' ').trim();
    if(el.labels?.length)return Array.from(el.labels).map(x=>x.textContent||'').join(' ').trim();
    if(el.tagName==='INPUT'&&['submit','button','reset'].includes(el.type))return (el.value||({submit:'Submit',reset:'Reset'}[el.type])||'').trim();
    return (el.getAttribute('alt')||el.getAttribute('title')||el.textContent||el.getAttribute('placeholder')||'').trim();
  };
  const value = el => ('value' in el ? String(el.value??'') : null);
  const domPath = el => {
    const parts=[];
    while(el&&el.nodeType===1&&el!==document.documentElement){
      let index=1, sibling=el;
      while((sibling=sibling.previousElementSibling))if(sibling.tagName===el.tagName)index++;
      parts.push(`${el.tagName.toLowerCase()}:nth-of-type(${index})`);el=el.parentElement;
    }
    return `dom:${parts.reverse().join('>')}`;
  };
  const nodeId = el => el.getAttribute('data-fission-node') || el.id || el.getAttribute('data-testid') || domPath(el);
  const node = el => {
    const r=rect(el), shown=visible(el), parent=el.parentElement?.closest(semanticSelector);
    const children=all().filter(child=>child!==el && child.parentElement?.closest(semanticSelector)===el).map(nodeId);
    const disabled=!!el.disabled||el.getAttribute('aria-disabled')==='true';
    const readOnly=!!el.readOnly||el.getAttribute('aria-readonly')==='true';
    const editable=el.matches('input:not([type]),input[type="text"],input[type="email"],input[type="password"],input[type="search"],input[type="url"],input[type="tel"],textarea,[contenteditable="true"]');
    const focusable=el.matches('button,a[href],input,textarea,select,summary,[tabindex]:not([tabindex="-1"])');
    const actions=[]; if(focusable)actions.push('focus'); if(el.matches('button,a[href],input,select,option,summary,[role="button"]'))actions.push('activate'); if(editable)actions.push('set_value');
    const disclosure=el.tagName==='DETAILS'?el:(el.tagName==='SUMMARY'&&el.parentElement?.tagName==='DETAILS'?el.parentElement:null);
    return {
      identifier:el.getAttribute('data-fission-semantics')||el.getAttribute('data-testid')||el.id||null, widget_id:el.getAttribute('data-fission-node')||el.id||'', stable_node_id:nodeId(el),
      parent:parent?nodeId(parent):null, children, role:role(el), label:label(el)||null, value:value(el), value_present:value(el)!==null,
      focusable, sequential_focusable:focusable&&el.tabIndex>=0, text_editable:editable, disabled, read_only:readOnly,
      checked:'checked' in el?Boolean(el.checked):(el.hasAttribute('aria-checked')?el.getAttribute('aria-checked')==='true':null),
      selected:'selected' in el?Boolean(el.selected):(el.hasAttribute('aria-selected')?el.getAttribute('aria-selected')==='true':null),
      expanded:el.hasAttribute('aria-expanded')?el.getAttribute('aria-expanded')==='true':(disclosure?disclosure.open:null),
      has_popup:el.getAttribute('aria-haspopup'), orientation:el.getAttribute('aria-orientation'), modal:el.getAttribute('aria-modal')==='true',
      required:!!el.required||el.getAttribute('aria-required')==='true', invalid:el.getAttribute('aria-invalid')==='true',
      controls:(el.getAttribute('aria-controls')||'').split(/\s+/).filter(Boolean), labelled_by:(el.getAttribute('aria-labelledby')||'').split(/\s+/).filter(Boolean),
      described_by:(el.getAttribute('aria-describedby')||'').split(/\s+/).filter(Boolean), active_descendant:el.getAttribute('aria-activedescendant'),
      actions, text_selection:typeof el.selectionStart==='number'?[el.selectionStart,el.selectionEnd]:null, masked:el.type==='password',
      scrollable_x:el.scrollWidth>el.clientWidth, scrollable_y:el.scrollHeight>el.clientHeight, logical_bounds:r,
      visible_bounds:shown?clippedRect(el):null, visibility:shown?'FullyVisible':'Hidden', x:r.x,y:r.y,width:r.width,height:r.height
    };
  };
  const candidates = query => {
    let root=document;
    if(query.scope){const scoped=resolve(query.scope);if(scoped.element)root=scoped.element;else return [];}
    const wanted=query.selector, norm=s=>String(s||'').trim().toLowerCase();
    return all().filter(el=>root===document||root.contains(el)).filter(el=>{
      switch(wanted.kind){
        case 'semantic_identifier': return el.getAttribute('data-fission-semantics')===wanted.identifier;
        case 'test_id': return el.getAttribute('data-testid')===wanted.test_id||el.getAttribute('data-fission-semantics')===wanted.test_id;
        case 'accessibility_identifier': return el.id===wanted.identifier||el.getAttribute('data-fission-semantics')===wanted.identifier;
        case 'widget_id': return el.getAttribute('data-fission-node')===wanted.widget_id||el.id===wanted.widget_id;
        case 'role_label': return norm(role(el))===norm(wanted.role)&&norm(label(el))===norm(wanted.label);
        case 'label': return norm(label(el))===norm(wanted.label);
        default:return false;
      }
    }).filter(el=>query.include_hidden||visible(el));
  };
  const resolve = query => {
    const found=candidates(query), index=query.index;
    if(index!=null)return found[index]?{element:found[index]}:{failure:'NoMatch',found};
    if(found.length===1)return {element:found[0]};
    return {failure:found.length?'Ambiguous':'NoMatch',found};
  };
  const selectorResponse = query => {
    const result=resolve(query);
    if(result.element)return {status:'SelectorResolved',node:node(result.element)};
    return {status:'SelectorError',failure:{kind:result.failure,selector:query,candidates:result.found.map(el=>({node:node(el),rejected_reason:null})),message:result.failure==='Ambiguous'?`selector matched ${result.found.length} DOM elements`:'selector matched no DOM element'}};
  };
  const resolved = query => resolve(query).element;
  switch(command.cmd){
    case 'ResolveSelector': return selectorResponse(command.query);
    case 'ScrollIntoView': {const query={...command.query,include_hidden:true};const el=resolved(query);if(!el)return selectorResponse(query);el.scrollIntoView({block:'center',inline:'nearest',behavior:'instant'});void el.getBoundingClientRect();return selectorResponse(command.query);}
    case 'FocusSelector': {const el=resolved(command.query);if(!el)return selectorResponse(command.query);el.focus();return {status:'Ok'};}
    case 'Toggle': {const el=resolved(command.query);if(!el)return selectorResponse(command.query);el.click();return {status:'Ok'};}
    case 'SelectOption': {const el=resolved(command.query);if(!el)return selectorResponse(command.query);if(el.tagName!=='OPTION')return {status:'Error',message:'SelectOption requires an option element'};el.parentElement.value=el.value;el.parentElement.dispatchEvent(new Event('input',{bubbles:true}));el.parentElement.dispatchEvent(new Event('change',{bubbles:true}));return {status:'Ok'};}
    case 'GetTree': return {status:'Tree',nodes:all().map(node)};
    case 'GetText': return {status:'Text',items:Array.from(document.querySelectorAll('body *')).filter(el=>visible(el)&&el.children.length===0&&(el.innerText||el.textContent||'').trim()).map(el=>{const r=rect(el);return {text:(el.innerText||el.textContent||'').trim(),...r};})};
    case 'TapText': {const el=all().find(el=>visible(el)&&label(el).includes(command.text));if(!el)return {status:'Error',message:`text \`${command.text}\` was not found`};el.click();return {status:'Ok'};}
    case 'Pump': return {status:'Ok'};
    case 'WaitForIdle': return {status:'MotionStatus',finite:0,repeating:0,ripples:0};
    default:return {status:'Error',message:`unsupported_site_command: ${command.cmd}`};
  }
})"#;

impl BrowserController {
    pub(super) fn send_site_command(&mut self, command: TestCommand) -> Result<TestResponse> {
        let command = serde_json::to_string(&command)?;
        let expression = format!("{SITE_COMMAND}({command})");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let result = self.client.send(
                "Runtime.evaluate",
                json!({"expression":expression,"returnByValue":true,"awaitPromise":true}),
            )?;
            if let Err(error) = runtime_exception(&result) {
                let message = format!("{error:#}");
                if Instant::now() < deadline
                    && (message.contains("Execution context was destroyed")
                        || message.contains("Cannot find context"))
                {
                    self.client.drain_events(Duration::from_millis(10))?;
                    std::thread::sleep(Duration::from_millis(25));
                    continue;
                }
                return Err(error);
            }
            let value = result
                .pointer("/result/value")
                .cloned()
                .context("website command returned no value")?;
            return serde_json::from_value(value)
                .context("failed to decode website command response");
        }
    }

    pub(super) fn send_site_test_command(&mut self, command: TestCommand) -> Result<TestResponse> {
        match command {
            TestCommand::SimulateResize { width, height } => {
                self.resize_viewport(width, height)?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::Wait { ms } => {
                std::thread::sleep(Duration::from_millis(ms));
                Ok(TestResponse::Ok {})
            }
            TestCommand::WaitForSelector { query, timeout_ms } => {
                self.site_wait_selector(query, timeout_ms, SiteWait::Present)
            }
            TestCommand::WaitForVisible { query, timeout_ms } => {
                self.site_wait_selector(query, timeout_ms, SiteWait::Visible)
            }
            TestCommand::WaitForEnabled { query, timeout_ms } => {
                self.site_wait_selector(query, timeout_ms, SiteWait::Enabled)
            }
            TestCommand::WaitForDisabled { query, timeout_ms } => {
                self.site_wait_selector(query, timeout_ms, SiteWait::Disabled)
            }
            TestCommand::WaitForValue {
                query,
                value,
                timeout_ms,
            } => self.site_wait_selector(query, timeout_ms, SiteWait::Value(value)),
            TestCommand::WaitForGone { query, timeout_ms } => {
                self.site_wait_selector(query, timeout_ms, SiteWait::Gone)
            }
            TestCommand::WaitForText { text, timeout_ms } => self.site_wait_text(&text, timeout_ms),
            TestCommand::WaitForIdle { timeout_ms, .. } => self.site_wait_idle(timeout_ms),
            TestCommand::TapSelector { query } | TestCommand::ActivateSelector { query } => {
                self.site_click(query, "left")
            }
            TestCommand::Toggle { query } => self.send_site_command(TestCommand::Toggle { query }),
            TestCommand::HoverSelector { query } => self.site_pointer(query, None),
            TestCommand::RightClickSelector { query } => self.site_click(query, "right"),
            TestCommand::FillText { query, text } => self.site_fill(query, &text),
            TestCommand::ClearText { query } => self.site_fill(query, ""),
            TestCommand::Tap { x, y } => {
                self.site_mouse_click(x, y, "left")?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::Scroll { x, y, dx, dy } => {
                self.client.send(
                    "Input.dispatchMouseEvent",
                    json!({"type":"mouseWheel","x":x,"y":y,"deltaX":dx,"deltaY":dy}),
                )?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::TypeText { text } | TestCommand::ImeCommit { text } => {
                self.client.send("Input.insertText", json!({"text":text}))?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::PressKey { key, modifiers } => self.press_dom_key(&key, modifiers),
            TestCommand::Screenshot { path } => {
                let bytes = self.capture_page_screenshot()?;
                super::write_screenshot(&path.into(), &bytes)?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::CaptureScreenshot {} => self.capture_page_response(),
            TestCommand::Quit {} => Ok(TestResponse::Ok {}),
            command => self.send_site_command(command),
        }
    }

    fn site_node(
        &mut self,
        query: SelectorQuery,
    ) -> Result<Result<crate::SemanticNode, TestResponse>> {
        Ok(
            match self.send_site_command(TestCommand::ResolveSelector { query })? {
                TestResponse::SelectorResolved { node } => Ok(node),
                response => Err(response),
            },
        )
    }

    fn site_pointer(&mut self, query: SelectorQuery, button: Option<&str>) -> Result<TestResponse> {
        let node = match self.site_node(query)? {
            Ok(node) => node,
            Err(response) => return Ok(response),
        };
        let b = node.visible_bounds.unwrap_or(node.logical_bounds);
        let x = b.x + b.width / 2.0;
        let y = b.y + b.height / 2.0;
        self.client.send(
            "Input.dispatchMouseEvent",
            json!({"type":"mouseMoved","x":x,"y":y}),
        )?;
        if let Some(button) = button {
            self.site_mouse_click(x, y, button)?;
        }
        Ok(TestResponse::Ok {})
    }

    fn site_click(&mut self, query: SelectorQuery, button: &str) -> Result<TestResponse> {
        let scroll = self.send_site_command(TestCommand::ScrollIntoView {
            query: query.clone().include_hidden(),
        })?;
        if !matches!(scroll, TestResponse::SelectorResolved { .. }) {
            return Ok(scroll);
        }
        // Chromium can acknowledge the DOM scroll before the compositor's hit-test
        // data reflects it. Wait for the scroll to cross a paint boundary, then
        // resolve the target again before dispatching native pointer input.
        self.paint_boundary()?;
        self.site_pointer(query, Some(button))
    }

    fn site_mouse_click(&mut self, x: f32, y: f32, button: &str) -> Result<()> {
        let buttons = if button == "right" { 2 } else { 1 };
        self.client.send("Input.dispatchMouseEvent",json!({"type":"mousePressed","x":x,"y":y,"button":button,"buttons":buttons,"clickCount":1}))?;
        self.client.send(
            "Input.dispatchMouseEvent",
            json!({"type":"mouseReleased","x":x,"y":y,"button":button,"buttons":0,"clickCount":1}),
        )?;
        Ok(())
    }

    fn site_fill(&mut self, query: SelectorQuery, text: &str) -> Result<TestResponse> {
        let response = self.site_click(query.clone(), "left")?;
        if !matches!(response, TestResponse::Ok {}) {
            return Ok(response);
        }
        let response = self.send_site_command(TestCommand::FocusSelector {
            query: query.clone(),
        })?;
        if !matches!(response, TestResponse::Ok {}) {
            return Ok(response);
        }
        self.client.send("Input.dispatchKeyEvent",json!({"type":"keyDown","key":"a","code":"KeyA","modifiers":2,"commands":["selectAll"]}))?;
        self.client.send(
            "Input.dispatchKeyEvent",
            json!({"type":"keyUp","key":"a","code":"KeyA","modifiers":2}),
        )?;
        if text.is_empty() {
            self.client.send(
                "Input.dispatchKeyEvent",
                json!({"type":"keyDown","key":"Backspace","code":"Backspace"}),
            )?;
            self.client.send(
                "Input.dispatchKeyEvent",
                json!({"type":"keyUp","key":"Backspace","code":"Backspace"}),
            )?;
        } else {
            self.client.send("Input.insertText", json!({"text":text}))?;
        }
        Ok(TestResponse::Ok {})
    }

    fn site_wait_selector(
        &mut self,
        query: SelectorQuery,
        timeout_ms: u64,
        wait: SiteWait,
    ) -> Result<TestResponse> {
        let start = Instant::now();
        loop {
            let response = self.send_site_command(TestCommand::ResolveSelector {
                query: query.clone(),
            })?;
            if wait.matches(&response) {
                return Ok(TestResponse::Ok {});
            }
            if start.elapsed() >= Duration::from_millis(timeout_ms) {
                return Ok(TestResponse::SelectorError {
                    failure: SelectorFailure {
                        kind: SelectorFailureKind::Timeout,
                        selector: query,
                        candidates: vec![],
                        message: format!(
                            "timed out after {timeout_ms}ms waiting for website selector"
                        ),
                    },
                });
            }
            self.client.drain_events(Duration::from_millis(10))?;
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn site_wait_text(&mut self, text: &str, timeout_ms: u64) -> Result<TestResponse> {
        let start = Instant::now();
        loop {
            if let TestResponse::Text { items } = self.send_site_command(TestCommand::GetText {})? {
                if items.iter().any(|item| item.text.contains(text)) {
                    return Ok(TestResponse::Ok {});
                }
            }
            if start.elapsed() >= Duration::from_millis(timeout_ms) {
                return Ok(TestResponse::Error {
                    message: format!(
                        "timed out after {timeout_ms}ms waiting for browser text `{text}`"
                    ),
                });
            }
            self.client.drain_events(Duration::from_millis(10))?;
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn site_wait_idle(&mut self, timeout_ms: u64) -> Result<TestResponse> {
        let start = Instant::now();
        let timeout = Duration::from_millis(timeout_ms);
        loop {
            self.client.drain_events(Duration::from_millis(10))?;
            let ready = match self.evaluate_json("document.readyState === 'complete'") {
                Ok(value) => value.as_bool().unwrap_or(false),
                Err(_) if start.elapsed() < timeout => false,
                Err(error) => return Err(error),
            };
            if ready
                && self.client.network_requests.is_empty()
                && self.client.network_last_activity.elapsed() >= Duration::from_millis(100)
            {
                return Ok(TestResponse::Ok {});
            }
            if start.elapsed() >= timeout {
                return Ok(TestResponse::Error {
                    message: format!(
                        "timed out after {timeout_ms}ms waiting for website navigation and network activity to settle"
                    ),
                });
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

enum SiteWait {
    Present,
    Visible,
    Enabled,
    Disabled,
    Value(String),
    Gone,
}
impl SiteWait {
    fn matches(&self, response: &TestResponse) -> bool {
        match (self, response) {
            (Self::Gone, TestResponse::SelectorError { failure }) => {
                failure.kind == SelectorFailureKind::NoMatch
            }
            (Self::Present, TestResponse::SelectorResolved { .. }) => true,
            (Self::Visible, TestResponse::SelectorResolved { node }) => {
                node.visibility != crate::VisibilityState::Hidden
            }
            (Self::Enabled, TestResponse::SelectorResolved { node }) => !node.disabled,
            (Self::Disabled, TestResponse::SelectorResolved { node }) => node.disabled,
            (Self::Value(value), TestResponse::SelectorResolved { node }) => {
                node.value.as_ref() == Some(value)
            }
            _ => false,
        }
    }
}
