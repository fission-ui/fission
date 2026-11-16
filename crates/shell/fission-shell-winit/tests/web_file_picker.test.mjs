import assert from "node:assert/strict";
import test from "node:test";

import {
  fissionAbortPickedFileWriter,
  fissionClosePickedFileWriter,
  fissionOpenPickedFileWriter,
  fissionPickSaveFile,
  fissionReleaseWritableFile,
  fissionWritePickedFileChunk,
} from "../src/web_file_picker.mjs";

test("a picked Web save destination remains usable after writes and failures", async () => {
  const originalPicker = globalThis.showSaveFilePicker;
  let savedText = "";
  let failNextOpen = false;
  const handle = {
    name: "report.txt",
    async createWritable() {
      if (failNextOpen) {
        failNextOpen = false;
        throw new DOMException("permission denied", "NotAllowedError");
      }

      const chunks = [];
      return {
        async write(bytes) {
          chunks.push(new TextDecoder().decode(bytes));
        },
        async close() {
          savedText = chunks.join("");
        },
        async abort() {
          chunks.length = 0;
        },
      };
    },
  };

  try {
    globalThis.showSaveFilePicker = async () => handle;
    const selected = await fissionPickSaveFile("report.txt", "[]");
    assert.equal(selected.name, "report.txt");

    const firstWriter = await fissionOpenPickedFileWriter(selected.id);
    await fissionWritePickedFileChunk(firstWriter, new TextEncoder().encode("first draft"));
    await fissionClosePickedFileWriter(firstWriter);
    assert.equal(savedText, "first draft");

    const secondWriter = await fissionOpenPickedFileWriter(selected.id);
    await fissionWritePickedFileChunk(secondWriter, new TextEncoder().encode("revised draft"));
    await fissionClosePickedFileWriter(secondWriter);
    assert.equal(savedText, "revised draft");

    const abortedWriter = await fissionOpenPickedFileWriter(selected.id);
    await fissionAbortPickedFileWriter(abortedWriter);
    assert.equal(savedText, "revised draft");

    failNextOpen = true;
    await assert.rejects(fissionOpenPickedFileWriter(selected.id), { name: "NotAllowedError" });
    const recoveredWriter = await fissionOpenPickedFileWriter(selected.id);
    await fissionWritePickedFileChunk(recoveredWriter, new TextEncoder().encode("saved after retry"));
    await fissionClosePickedFileWriter(recoveredWriter);
    assert.equal(savedText, "saved after retry");

    fissionReleaseWritableFile(selected.id);
    await assert.rejects(fissionOpenPickedFileWriter(selected.id), { name: "invalid_handle" });
    assert.doesNotThrow(() => fissionReleaseWritableFile(selected.id));

    globalThis.showSaveFilePicker = async () => {
      throw new DOMException("user cancelled", "AbortError");
    };
    assert.equal(await fissionPickSaveFile("cancelled.txt", "[]"), null);
    await assert.rejects(fissionOpenPickedFileWriter(selected.id + 100), { name: "invalid_handle" });
  } finally {
    if (originalPicker === undefined) {
      delete globalThis.showSaveFilePicker;
    } else {
      globalThis.showSaveFilePicker = originalPicker;
    }
  }
});

test("a browser without the save picker downloads a streamed file synchronously on close", async () => {
  const pickerDescriptor = Object.getOwnPropertyDescriptor(globalThis, "showSaveFilePicker");
  const documentDescriptor = Object.getOwnPropertyDescriptor(globalThis, "document");
  const createObjectUrl = URL.createObjectURL;
  const revokeObjectUrl = URL.revokeObjectURL;
  const blobs = new Map();
  const downloads = [];
  let nextUrl = 1;
  let userActivation = true;
  const body = {
    children: [],
    appendChild(anchor) {
      this.children.push(anchor);
      return anchor;
    },
  };

  try {
    delete globalThis.showSaveFilePicker;
    Object.defineProperty(globalThis, "document", {
      configurable: true,
      value: {
        body,
        createElement(tagName) {
          assert.equal(tagName, "a");
          return {
            style: {},
            click() {
              downloads.push({
                active: userActivation,
                name: this.download,
                blob: blobs.get(this.href),
              });
            },
            remove() {
              const index = body.children.indexOf(this);
              if (index >= 0) body.children.splice(index, 1);
            },
          };
        },
      },
    });
    URL.createObjectURL = (blob) => {
      const url = `blob:fission-test-${nextUrl++}`;
      blobs.set(url, blob);
      return url;
    };
    URL.revokeObjectURL = (url) => blobs.delete(url);

    const selected = await fissionPickSaveFile(
      "report.pdf",
      JSON.stringify([["PDF", { "application/pdf": [".pdf"] }]]),
    );
    assert.equal(selected.name, "report.pdf");
    assert.equal(selected.permission, "unknown");

    const writer = await fissionOpenPickedFileWriter(selected.id);
    await fissionWritePickedFileChunk(writer, new Uint8Array([0x25, 0x50]));
    await fissionWritePickedFileChunk(writer, new Uint8Array([0x44, 0x46, 0x2d]));
    const closing = fissionClosePickedFileWriter(writer);
    assert.equal(downloads.length, 1, "close should click the anchor before returning its promise");
    assert.equal(downloads[0].active, true, "the synchronous anchor click should retain activation");
    userActivation = false;
    await closing;

    assert.equal(downloads[0].name, "report.pdf");
    assert.equal(downloads[0].blob.type, "application/pdf");
    assert.deepEqual(
      new Uint8Array(await downloads[0].blob.arrayBuffer()),
      new Uint8Array([0x25, 0x50, 0x44, 0x46, 0x2d]),
    );
    assert.equal(body.children.length, 0, "temporary anchors should be removed immediately");
  } finally {
    URL.createObjectURL = createObjectUrl;
    URL.revokeObjectURL = revokeObjectUrl;
    if (documentDescriptor) Object.defineProperty(globalThis, "document", documentDescriptor);
    else delete globalThis.document;
    if (pickerDescriptor) Object.defineProperty(globalThis, "showSaveFilePicker", pickerDescriptor);
    else delete globalThis.showSaveFilePicker;
  }
});

test("aborting a browser download writer drops buffered chunks without downloading", async () => {
  const pickerDescriptor = Object.getOwnPropertyDescriptor(globalThis, "showSaveFilePicker");
  const documentDescriptor = Object.getOwnPropertyDescriptor(globalThis, "document");
  let clicks = 0;

  try {
    delete globalThis.showSaveFilePicker;
    Object.defineProperty(globalThis, "document", {
      configurable: true,
      value: {
        body: {
          appendChild(anchor) { return anchor; },
        },
        createElement() {
          return {
            style: {},
            click() { clicks += 1; },
            remove() {},
          };
        },
      },
    });

    const selected = await fissionPickSaveFile("cancelled.txt", "[]");
    const writer = await fissionOpenPickedFileWriter(selected.id);
    await fissionWritePickedFileChunk(writer, new TextEncoder().encode("do not save"));
    await fissionAbortPickedFileWriter(writer);
    assert.equal(clicks, 0);
    await assert.rejects(fissionClosePickedFileWriter(writer), { name: "InvalidStateError" });
    assert.equal(clicks, 0, "an aborted writer must never start a download");
  } finally {
    if (documentDescriptor) Object.defineProperty(globalThis, "document", documentDescriptor);
    else delete globalThis.document;
    if (pickerDescriptor) Object.defineProperty(globalThis, "showSaveFilePicker", pickerDescriptor);
    else delete globalThis.showSaveFilePicker;
  }
});

test("the browser download fallback rejects output above its memory cap", async () => {
  const pickerDescriptor = Object.getOwnPropertyDescriptor(globalThis, "showSaveFilePicker");

  try {
    delete globalThis.showSaveFilePicker;
    const selected = await fissionPickSaveFile("large.bin", "[]");
    const writer = await fissionOpenPickedFileWriter(selected.id);
    await assert.rejects(
      fissionWritePickedFileChunk(writer, { byteLength: 128 * 1024 * 1024 + 1 }),
      {
        name: "QuotaExceededError",
        message: "browser download fallback buffers files in memory and is limited to 128 MiB",
      },
    );
    await fissionAbortPickedFileWriter(writer);
  } finally {
    if (pickerDescriptor) Object.defineProperty(globalThis, "showSaveFilePicker", pickerDescriptor);
    else delete globalThis.showSaveFilePicker;
  }
});

test("browser download setup failures reject close with a useful host error", async () => {
  const pickerDescriptor = Object.getOwnPropertyDescriptor(globalThis, "showSaveFilePicker");
  const documentDescriptor = Object.getOwnPropertyDescriptor(globalThis, "document");
  const createObjectUrl = URL.createObjectURL;

  try {
    delete globalThis.showSaveFilePicker;
    Object.defineProperty(globalThis, "document", {
      configurable: true,
      value: { body: { appendChild() {} }, createElement: () => ({ style: {}, remove() {} }) },
    });
    URL.createObjectURL = undefined;

    const selected = await fissionPickSaveFile("unavailable.txt", "[]");
    const writer = await fissionOpenPickedFileWriter(selected.id);
    await fissionWritePickedFileChunk(writer, new TextEncoder().encode("data"));
    await assert.rejects(fissionClosePickedFileWriter(writer), {
      name: "NotSupportedError",
      message: "browser Blob downloads are unavailable",
    });
  } finally {
    URL.createObjectURL = createObjectUrl;
    if (documentDescriptor) Object.defineProperty(globalThis, "document", documentDescriptor);
    else delete globalThis.document;
    if (pickerDescriptor) Object.defineProperty(globalThis, "showSaveFilePicker", pickerDescriptor);
    else delete globalThis.showSaveFilePicker;
  }
});

test("a Web save handle restores from IndexedDB with its current permission", async () => {
  const pickerDescriptor = Object.getOwnPropertyDescriptor(globalThis, "showSaveFilePicker");
  const indexedDbDescriptor = Object.getOwnPropertyDescriptor(globalThis, "indexedDB");
  const records = new Map();
  const databaseKey = (key) => JSON.stringify(key);
  let permissionState = "prompt";
  let savedText = "";
  const database = {
    objectStoreNames: { contains: () => true },
    createObjectStore() {},
    close() {},
    transaction(_name, mode) {
      const transaction = {};
      const makeRequest = (operation) => {
        const request = {};
        queueMicrotask(() => {
          try {
            request.result = operation();
            request.onsuccess?.();
            queueMicrotask(() => transaction.oncomplete?.());
          } catch (error) {
            request.error = error;
            request.onerror?.();
            transaction.error = error;
            transaction.onerror?.();
          }
        });
        return request;
      };
      const store = {
        get: (key) => makeRequest(() => records.get(databaseKey(key))),
        put: (value, key) => {
          assert.equal(mode, "readwrite");
          return makeRequest(() => records.set(databaseKey(key), value));
        },
        delete: (key) => {
          assert.equal(mode, "readwrite");
          return makeRequest(() => records.delete(databaseKey(key)));
        },
      };
      transaction.objectStore = () => store;
      return transaction;
    },
  };
  const indexedDb = {
    open(name, version) {
      assert.equal(name, "fission-file-system-handles");
      assert.equal(version, 1);
      const request = { result: database };
      queueMicrotask(() => {
        request.onupgradeneeded?.();
        request.onsuccess?.();
      });
      return request;
    },
  };
  records.set(databaseKey("save-file:monthly-report"), "existing-directory-grant");
  const handle = {
    name: "report.txt",
    async queryPermission({ mode }) {
      assert.equal(mode, "readwrite");
      return permissionState;
    },
    async requestPermission({ mode }) {
      assert.equal(mode, "readwrite");
      permissionState = "granted";
      return permissionState;
    },
    async createWritable() {
      const chunks = [];
      return {
        async write(bytes) { chunks.push(new TextDecoder().decode(bytes)); },
        async close() { savedText = chunks.join(""); },
        async abort() { chunks.length = 0; },
      };
    },
  };

  try {
    Object.defineProperty(globalThis, "indexedDB", { configurable: true, value: indexedDb });
    globalThis.showSaveFilePicker = async () => handle;
    const firstSession = await import(`../src/web_file_picker.mjs?session=${Date.now()}-pick`);
    const selected = await firstSession.fissionPickSaveFile("report.txt", "[]", "monthly-report");
    assert.equal(selected.permission, "prompt");
    assert.equal(
      records.get(databaseKey(["fission.save-file", "monthly-report"])),
      handle,
    );
    assert.equal(
      records.get(databaseKey("save-file:monthly-report")),
      "existing-directory-grant",
    );

    const restoredSession = await import(`../src/web_file_picker.mjs?session=${Date.now()}-restore`);
    const restored = await restoredSession.fissionRestoreSaveFile("monthly-report");
    assert.equal(restored.name, "report.txt");
    assert.equal(restored.permission, "prompt");
    assert.equal(
      await restoredSession.fissionWritableFilePermission(restored.id, false),
      "prompt",
    );
    assert.equal(
      await restoredSession.fissionWritableFilePermission(restored.id, true),
      "granted",
    );

    const writer = await restoredSession.fissionOpenPickedFileWriter(restored.id);
    await restoredSession.fissionWritePickedFileChunk(writer, new TextEncoder().encode("saved"));
    await restoredSession.fissionClosePickedFileWriter(writer);
    assert.equal(savedText, "saved");

    restoredSession.fissionReleaseWritableFile(restored.id);
    await assert.rejects(restoredSession.fissionOpenPickedFileWriter(restored.id), { name: "invalid_handle" });
    const restoredAgain = await restoredSession.fissionRestoreSaveFile("monthly-report");
    assert.equal(restoredAgain.name, "report.txt");

    await restoredSession.fissionForgetSaveFile("monthly-report");
    restoredSession.fissionReleaseWritableFile(restoredAgain.id);
    firstSession.fissionReleaseWritableFile(selected.id);
    const afterForget = await restoredSession.fissionRestoreSaveFile("monthly-report");
    assert.equal(afterForget, null);
    assert.equal(records.has(databaseKey(["fission.save-file", "monthly-report"])), false);
    assert.equal(
      records.get(databaseKey("save-file:monthly-report")),
      "existing-directory-grant",
    );

    Object.defineProperty(globalThis, "indexedDB", { configurable: true, value: {
      open() {
        const request = {};
        queueMicrotask(() => {
          request.error = new DOMException("database transaction aborted", "AbortError");
          request.onerror?.();
        });
        return request;
      },
    } });
    await assert.rejects(
      restoredSession.fissionPickSaveFile("report.txt", "[]", "abort-key"),
      { name: "AbortError" },
    );
  } finally {
    if (pickerDescriptor) Object.defineProperty(globalThis, "showSaveFilePicker", pickerDescriptor);
    else delete globalThis.showSaveFilePicker;
    if (indexedDbDescriptor) Object.defineProperty(globalThis, "indexedDB", indexedDbDescriptor);
    else delete globalThis.indexedDB;
  }
});
