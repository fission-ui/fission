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
  }

  querySelectorAll(selector) {
    if (selector === '[data-fission-browser-text-action="true"]') return this.controls;
    if (selector.includes('input[data-fission-node]')) return this.controls;
    return [];
  }

  contains(element) {
    return this.controls.includes(element);
  }
}

const roots = new Map();
globalThis.document = {
  activeElement: null,
  body: {},
  documentElement: { getAttribute() { return null; }, lang: 'en' },
  addEventListener() {},
  getElementById(id) { return roots.get(id) ?? null; },
  querySelector() { return null; },
  querySelectorAll() { return []; },
};
globalThis.window = {
  CSS: { escape(value) { return String(value); } },
  history: { pushState() {}, replaceState() {} },
  location: { href: 'https://example.test/', pathname: '/' },
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
