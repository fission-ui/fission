import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';

class MockControl {
  constructor(node, value = '') {
    this.attributes = new Map([
      ['data-fission-node', node],
      ['data-fission-action-target', node],
      ['data-fission-browser-text-action', 'true'],
    ]);
    this.value = value;
    this.selectionStart = value.length;
    this.selectionEnd = value.length;
    this.selectionDirection = 'none';
    this.listeners = new Map();
    this.focused = false;
    this.selectionSet = null;
  }

  addEventListener(kind, listener) {
    const listeners = this.listeners.get(kind) || [];
    listeners.push(listener);
    this.listeners.set(kind, listeners);
  }

  emit(kind, event = {}) {
    for (const listener of this.listeners.get(kind) || []) {
      listener({ type: kind, isComposing: false, preventDefault() {}, ...event });
    }
  }

  getAttribute(name) {
    return this.attributes.get(name) ?? null;
  }

  matches(selector) {
    if (selector === 'input[type="checkbox"],input[type="radio"]') {
      const type = this.getAttribute('type');
      return type === 'checkbox' || type === 'radio';
    }
    return selector.includes('input[data-fission-node]');
  }

  focus() {
    this.focused = true;
    globalThis.document.activeElement = this;
  }

  setSelectionRange(start, end, direction) {
    this.selectionStart = start;
    this.selectionEnd = end;
    this.selectionDirection = direction;
    this.selectionSet = [start, end, direction];
  }
}

class MockRoot {
  constructor(controls) {
    this.controls = controls;
    this.innerHTML = '';
  }

  querySelectorAll(selector) {
    if (selector === '[data-fission-browser-text-action="true"]') {
      return this.controls.filter((control) => control.getAttribute('data-fission-browser-text-action') === 'true');
    }
    if (selector === '[data-fission-browser-range-action="true"]') {
      return this.controls.filter((control) => control.getAttribute('data-fission-browser-range-action') === 'true');
    }
    if (selector === '[data-fission-browser-action="true"]') {
      return this.controls.filter((control) => control.getAttribute('data-fission-browser-action') === 'true');
    }
    if (selector === '[data-fission-node]') return this.controls;
    if (selector.includes('input[data-fission-node]')) return this.controls;
    return [];
  }

  contains(element) {
    return this.controls.includes(element);
  }
}

class MockMenuNode {
  constructor(attributes = {}) {
    this.attributes = new Map(Object.entries(attributes));
    this.listeners = new Map();
    this.children = [];
    this.parent = null;
    this.disabled = false;
    this.clicks = 0;
    this.id = attributes.id || '';
  }

  addEventListener(kind, listener) {
    const listeners = this.listeners.get(kind) || [];
    listeners.push(listener);
    this.listeners.set(kind, listeners);
  }

  emit(kind, event = {}) {
    for (const listener of this.listeners.get(kind) || []) {
      listener({ key: '', shiftKey: false, preventDefault() {}, ...event });
    }
  }

  getAttribute(name) { return this.attributes.get(name) ?? null; }
  matches(selector) {
    return selector === '[aria-haspopup="menu"][aria-controls]'
      ? this.getAttribute('aria-haspopup') === 'menu' && this.getAttribute('aria-controls') !== null
      : selector === '[role="menuitem"]' && this.getAttribute('role') === 'menuitem';
  }
  closest(selector) {
    return selector === '[role="menu"]' && this.parent?.getAttribute('role') === 'menu'
      ? this.parent
      : null;
  }
  querySelectorAll(selector) {
    return selector === '[role="menuitem"]'
      ? this.children.filter((child) => child.getAttribute('role') === 'menuitem')
      : [];
  }
  focus() { document.activeElement = this; }
  click() { this.clicks += 1; this.emit('click'); }
}

const roots = new Map();
globalThis.document = {
  activeElement: null,
  body: {},
  documentElement: { getAttribute() { return null; }, lang: 'en' },
  addEventListener() {},
  getElementById(id) { return roots.get(id) ?? null; },
  querySelector(selector) {
    const match = selector.match(/^\[data-fission-semantics="(.+)"\]$/);
    return match ? roots.get(match[1]) ?? null : null;
  },
  querySelectorAll() { return []; },
};
globalThis.window = {
  CSS: { escape(value) { return String(value); } },
  history: { pushState() {}, replaceState() {} },
  location: { href: 'https://example.test/', pathname: '/' },
  innerWidth: 1200,
  innerHeight: 800,
  addEventListener() {},
};
globalThis.__FISSION_SERVER_RUNTIME_TEST_HOOK__ = true;

const runtimeSource = readFileSync(new URL('./server-runtime.js', import.meta.url), 'utf8');
vm.runInThisContext(runtimeSource, { filename: 'server-runtime.js' });
const hooks = globalThis.__FISSION_SERVER_RUNTIME_TEST_HOOK__;

function boundBridge(root, control) {
  roots.set('mount', root);
  const posted = [];
  const bridge = {
    id: 'test-island',
    kind: 'island',
    config: { mount_id: 'mount' },
    worker: { postMessage(message) { posted.push(message); } },
    sequence: 0,
    boundEvents: Object.create(null),
  };
  hooks.bindFissionBrowserActions(bridge);
  return { bridge, control, posted };
}

function postedPayload(posted, index = 0) {
  assert.equal(posted[index]?.kind, 'event');
  return posted[index].payload;
}

test('text input sends value and UTF-8 selection without action metadata', () => {
  const control = new MockControl('901', 'café');
  control.selectionStart = 3;
  control.selectionEnd = 4;
  control.selectionDirection = 'backward';
  const { posted } = boundBridge(new MockRoot([control]), control);

  control.emit('beforeinput', {
    inputType: 'insertReplacementText',
    data: 'é',
    cancelable: false,
  });
  control.emit('input');

  const payload = postedPayload(posted);
  assert.equal(payload.value, 'café');
  assert.equal(payload.caret, 3);
  assert.equal(payload.anchor, 5);
  assert.equal(payload.input_type, 'insertReplacementText');
  assert.equal(payload.input_data, 'é');
  assert.equal(payload.input_cancelable, false);
  assert.deepEqual(payload.binding.message, {
    fission_browser_text_action: true,
    target_node: '901',
  });
  assert.equal(payload.binding.message.action_id, undefined);
  assert.equal(payload.binding.message.payload_hex, undefined);
});

test('number input falls back to the UTF-8 end when selection APIs are unavailable', () => {
  const control = new MockControl('902', '2525');
  control.selectionStart = null;
  control.selectionEnd = null;
  const { posted } = boundBridge(new MockRoot([control]), control);

  control.emit('input');

  const payload = postedPayload(posted);
  assert.equal(payload.value, '2525');
  assert.equal(payload.caret, 4);
  assert.equal(payload.anchor, 4);
});

test('native range input sends its live numeric value through the range action', () => {
  const control = new MockControl('910', '10');
  control.attributes.delete('data-fission-browser-text-action');
  control.attributes.set('data-fission-browser-range-action', 'true');
  const { posted } = boundBridge(new MockRoot([control]), control);

  control.value = '11';
  control.emit('input');

  const payload = postedPayload(posted);
  assert.equal(payload.value, '11');
  assert.equal(payload.binding.event, 'input');
  assert.deepEqual(payload.binding.message, {
    fission_browser_range_action: true,
    target_node: '910',
  });
  assert.equal(posted.length, 1);
});

test('native range replacement waits until a drag commits', async () => {
  const control = new MockControl('912', '10');
  control.attributes.delete('data-fission-browser-text-action');
  control.attributes.set('data-fission-browser-range-action', 'true');
  const root = new MockRoot([control]);
  const { bridge } = boundBridge(root, control);

  control.value = '11';
  control.emit('input');
  hooks.applyDomOp(bridge, {
    op: 'replace_children_html_by_semantics',
    semantics: 'mount',
    html: '<p>11</p>',
  });
  hooks.applyDomOp(bridge, {
    op: 'replace_children_html_by_semantics',
    semantics: 'mount',
    html: '<p>12</p>',
  });

  assert.equal(root.innerHTML, '', 'an active range must remain in the live DOM');
  control.emit('change');
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(root.innerHTML, '<p>12</p>', 'only the latest deferred render is committed');
});

test('number input replacement waits until editing commits', async () => {
  const control = new MockControl('913', '370');
  control.selectionStart = null;
  control.selectionEnd = null;
  const root = new MockRoot([control]);
  const { bridge } = boundBridge(root, control);

  control.value = '3700';
  control.emit('input');
  hooks.applyDomOp(bridge, {
    op: 'replace_children_html_by_semantics',
    semantics: 'mount',
    html: '<input type="number" value="3700">',
  });

  assert.equal(root.innerHTML, '', 'a number input without selection APIs must not be rebuilt mid-edit');
  control.emit('blur');
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(root.innerHTML, '<input type="number" value="3700">');
});

test('native radio dispatches once after the browser updates its checked state', () => {
  const control = new MockControl('911');
  control.attributes.delete('data-fission-browser-text-action');
  control.attributes.set('data-fission-browser-action', 'true');
  control.attributes.set('data-fission-action-id', '73');
  control.attributes.set('data-fission-action-payload', 'dead');
  control.attributes.set('type', 'radio');
  const { posted } = boundBridge(new MockRoot([control]), control);

  let prevented = false;
  control.emit('change', { preventDefault() { prevented = true; } });

  const payload = postedPayload(posted);
  assert.equal(prevented, false);
  assert.equal(payload.binding.event, 'change');
  assert.deepEqual(payload.binding.message, {
    fission_browser_action: true,
    action_id: '73',
    target_node: '911',
    payload_hex: 'dead',
  });
  assert.equal(posted.length, 1);
});

test('IME commit dispatches once and does not swallow the next real edit', () => {
  const control = new MockControl('903');
  const { posted } = boundBridge(new MockRoot([control]), control);

  control.emit('compositionstart');
  control.value = 'に';
  control.selectionStart = 1;
  control.selectionEnd = 1;
  control.emit('input', { isComposing: true });
  assert.equal(posted.length, 0);

  control.emit('compositionend');
  assert.equal(posted.length, 1);
  assert.equal(postedPayload(posted).value, 'に');

  control.emit('input');
  assert.equal(posted.length, 1, 'duplicate post-composition input must be ignored');

  control.value = '日本';
  control.selectionStart = 2;
  control.selectionEnd = 2;
  control.emit('input');
  assert.equal(posted.length, 2, 'next distinct edit must not be swallowed');
  assert.equal(postedPayload(posted, 1).value, '日本');
  assert.equal(postedPayload(posted, 1).caret, 6);
});

test('renderer replacement restores focused text selection by retained node', () => {
  const original = new MockControl('904', 'abcdef');
  original.selectionStart = 2;
  original.selectionEnd = 5;
  original.selectionDirection = 'backward';
  const root = new MockRoot([original]);
  document.activeElement = original;

  const focused = hooks.captureFocusedTextControl(root);
  const replacement = new MockControl('904', 'abcdef');
  root.controls = [replacement];
  hooks.restoreFocusedTextControl(root, focused);

  assert.equal(replacement.focused, true);
  assert.deepEqual(replacement.selectionSet, [2, 5, 'backward']);
});

test('UTF-16 DOM offsets convert to UTF-8 byte offsets', () => {
  assert.equal(hooks.utf8Offset('a😀b', 0), 0);
  assert.equal(hooks.utf8Offset('a😀b', 1), 1);
  assert.equal(hooks.utf8Offset('a😀b', 2), 1);
  assert.equal(hooks.utf8Offset('a😀b', 3), 5);
  assert.equal(hooks.utf8Offset('a😀b', 4), 6);
});

test('clipboard bridge requests complete through the worker event channel', async () => {
  let written = null;
  Object.defineProperty(globalThis, 'navigator', {
    configurable: true,
    value: {
      clipboard: {
        async writeText(value) { written = value; },
        async readText() { return 'copied value'; },
      },
    },
  });
  const posted = [];
  const bridge = {
    kind: 'island',
    id: 'clipboard',
    worker: { postMessage(message) { posted.push(message); } },
  };

  hooks.handleBridgeRequest(bridge, {
    type: 'request',
    request_id: 7,
    kind: 'clipboard_write_text',
    payload: { text: 'new value' },
  });
  await Promise.resolve();
  hooks.handleBridgeRequest(bridge, {
    type: 'request',
    request_id: 8,
    kind: 'clipboard_read_text',
    payload: null,
  });
  await Promise.resolve();

  assert.equal(written, 'new value');
  assert.deepEqual(posted[0], {
    kind: 'event',
    payload: {
      type: 'response',
      request_id: 7,
      ok: true,
      payload: {},
      error: null,
    },
  });
  assert.deepEqual(posted[1].payload, {
    type: 'response',
    request_id: 8,
    ok: true,
    payload: { text: 'copied value' },
    error: null,
  });
});

test('menu keyboard navigation enters, wraps and dismisses the menu', () => {
  const trigger = new MockMenuNode({
    'aria-haspopup': 'menu',
    'aria-controls': 'account-menu',
    'aria-expanded': 'true',
  });
  const menu = new MockMenuNode({ id: 'account-menu', role: 'menu' });
  const first = new MockMenuNode({ role: 'menuitem' });
  const disabled = new MockMenuNode({ role: 'menuitem', 'aria-disabled': 'true' });
  const last = new MockMenuNode({ role: 'menuitem' });
  for (const item of [first, disabled, last]) item.parent = menu;
  menu.children = [first, disabled, last];
  roots.set('account-menu', menu);
  const root = {
    contains(element) { return [trigger, menu, first, disabled, last].includes(element); },
    querySelectorAll(selector) {
      if (selector === '[aria-haspopup="menu"][aria-controls]') return [trigger];
      if (selector === '[role="menu"]') return [menu];
      return [];
    },
  };
  const bridge = {};
  hooks.bindMenuKeyboard(bridge, root);

  trigger.emit('keydown', { key: 'ArrowDown' });
  assert.equal(document.activeElement, first);
  menu.emit('keydown', { key: 'ArrowUp' });
  assert.equal(document.activeElement, last);
  menu.emit('keydown', { key: 'Home' });
  assert.equal(document.activeElement, first);
  menu.emit('keydown', { key: 'Escape' });
  assert.equal(trigger.clicks, 1);
  assert.deepEqual(bridge.pendingMenuFocus, {
    controls: 'account-menu',
    edge: 'first',
    returnToTrigger: true,
  });
});

test('flyout placement follows its anchor and avoids the viewport edge', () => {
  const anchor = {
    getBoundingClientRect() {
      return { left: 900, right: 1000, top: 730, bottom: 770, width: 100, height: 40 };
    },
  };
  const content = {
    getBoundingClientRect() {
      return { left: 0, right: 180, top: 0, bottom: 120, width: 180, height: 120 };
    },
  };
  const attributes = new Map([
    ['data-fission-flyout-anchor', 'anchor'],
    ['data-fission-flyout-content', 'content'],
    ['data-fission-flyout-alignment', 'End'],
    ['data-fission-flyout-placement', 'Auto'],
    ['data-fission-flyout-width', 'Content'],
    ['data-fission-flyout-gap', '4'],
  ]);
  const flyout = {
    style: {},
    getAttribute(name) { return attributes.get(name) ?? ''; },
    getBoundingClientRect: content.getBoundingClientRect,
    querySelector(selector) { return selector.includes('content') ? content : null; },
  };
  const root = {
    querySelector(selector) { return selector.includes('anchor') ? anchor : null; },
  };
  document.documentElement.clientWidth = 1024;
  document.documentElement.clientHeight = 800;

  hooks.positionFlyout(flyout, root);

  assert.equal(flyout.style.position, 'fixed');
  assert.equal(flyout.style.left, '820px');
  assert.equal(flyout.style.top, '606px');
});
