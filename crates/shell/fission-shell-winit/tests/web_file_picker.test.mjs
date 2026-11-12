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
