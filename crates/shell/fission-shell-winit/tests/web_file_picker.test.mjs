import assert from "node:assert/strict";
import test from "node:test";

import {
  fissionAbortPickedFileWriter,
  fissionClosePickedFileWriter,
  fissionOpenPickedFileWriter,
  fissionPickSaveFile,
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

    await restoredSession.fissionForgetSaveFile("monthly-report");
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
