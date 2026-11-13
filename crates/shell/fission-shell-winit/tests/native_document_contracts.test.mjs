// Source-level platform contracts; device qualification still exercises UIKit,
// Android's real PrintSpooler and File Providers.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const ios = await readFile(new URL("../src/ios_file_picker.rs", import.meta.url), "utf8");
const android = await readFile(new URL("../../../tools/fission-command-core/assets/android/rs/fission/runtime/FissionAndroidCapabilities.java", import.meta.url), "utf8");
const nativePrint = await readFile(new URL("../src/android_print.rs", import.meta.url), "utf8");

test("iOS save returns a writable moved document rather than an inaccessible export copy", () => {
  assert.match(ios, /initWithURL: url inMode: 3i64/);
  assert.doesNotMatch(ios, /initWithURL: url inMode: 2i64/);
  assert.match(ios, /move \|url: \*mut Object\|[\s\S]*?coordinated_path\(url\)/);
  assert.doesNotMatch(ios, /move \|_url: \*mut Object\|/);
});

test("Android print preserves the original PDF and lets the spooler apply selected pages", () => {
  assert.match(android, /FISSION_BRIDGE_API_VERSION = 1/);
  assert.match(android, /public static String printPdf\(Activity activity, String pdfPath/);
  assert.match(android, /onWrite[\s\S]*PRINT_WRITER\.execute[\s\S]*new FileInputStream\(document\)/);
  assert.match(android, /onWriteFinished\(new PageRange\[\] \{ PageRange\.ALL_PAGES \}\)/);
  assert.doesNotMatch(android, /cannot filter the selected PDF page range/);
  assert.doesNotMatch(android, /PdfRenderer|Bitmap\.createBitmap|pdf\.clone\(\)/);
  assert.match(android, /new ArrayBlockingQueue<Runnable>\(4\)/);
  assert.match(android, /while \(!cancellationSignal\.isCanceled\(\)/);
  assert.match(android, /setOnCancelListener\(\(\) -> \{[\s\S]*?destination\.close\(\)/);
  assert.match(android, /onFinish\(\) \{\s*document\.delete\(\)/);
  assert.doesNotMatch(nativePrint, /collect_data_stream|byte_array_from_slice/);
});

test("Android print job diagnostics use only public SDK methods", () => {
  assert.match(android, /jobId\.set\(job\.getId\(\)\.toString\(\)\)/);
  assert.doesNotMatch(android, /job\.getId\(\)\.flattenToString\(/);
});
