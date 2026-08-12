// Copyright 2026 The Fission Authors. All rights reserved.
// Use of this source code is governed by the license in the repository root.

// CanvasKit exposes LTR and RTL paragraph directions, but not SkParagraph's
// `TextDirection::kAuto`. These compact tables contain every Unicode 15.1
// codepoint whose Bidi_Class is L, R, or AL. Unicode 15.1 is the data version
// used by ICU 74.2 at Fission's pinned Skia revision. Keeping the resolution
// here makes Web's Auto contract identical to the native SkParagraph host
// without asking the browser's potentially different Unicode implementation.
//
// Each table is an unsigned-LEB128 stream of (delta, inclusive-length) pairs.
// `delta` is measured from the previous range's end plus one.

const LTR_RANGES_BASE64 =
  "QRkGGS8ACgAEAAUWAR4BwAMCBg4BDgQJAIEBAwIBAgMBAAYAAQIBAAETAVIBiwEHpQEBJQIw+QY2AQABAwgDAQIHCQIcAQEBBwIB" +
  "AhUBBgEAAwMDAwYBAgEBAAgABAEBAgQLAgYBAQUAAQUEAQIVAQYBAQEBAQEEAhgDAQAHCQICAQAMAAEIAQIBFQEGAQEBBAMDCAAB" +
  "AQMADwEECggACAEBBwIBAhUBBgEBAQQDAQEABgECAQoABAEBAgQRCwABBQMCAQMDAQEAAQEDAQMCAwsEAQEBAwIBAgMABgAODA4C" +
  "AQcBAgEWAQ8DAAMDEwICAAIBBAkHAAcBAQoBAgEWAQkBBAMHAQIBAQkBBgEBAQQJAQIOCgECASgCAwUCAQIBAQQNBBkCAQERAxcB" +
  "CAEAAgYIAgYHBgkCAgwvAQEMBggMJQEBAAEEARcBAAEJAQEJAAIEAQAJCQIDIBcCGgEAAQAFCQEjEgAFAAIEMQcBBQEMJSwEAAYA" +
  "AgECGAIDAw8EDAEBAgUBDgEnAQAFAAL4AgEDAgYBAAEDAigBAwIgAQMCBgEAAQMCDgE4AQMCQgUcAw8QVQIFA/4EARkFWAcRAwAJ" +
  "EgICCREODAECDzMCAAcHAQELBgEAAwkmCQZYBwQCIQEABUUKHgQDAgIEAQEFDScCBAsrBBkGCiUWAgEDNwEACQABAQgFDQkGCQYN" +
  "Vi8BAAUAAQQBCQMaCQoDHwQBAgADNwEAAgIBAAMBCC8IAQUOAzsHKgIKCwANAAcDAQUBAgIABb8BQJUCAgUCJQIFAgcBAAEAAQAB" +
  "HgI0AQYBAAMCAQYDAwIFBAwFAgEGEQBiAA0AEAxlAAQAAgkBAAMEBgABAAEAAQMBCgIDBQQEARAorQNEGgCGAk3CAwDTAv8BgAbk" +
  "AQYDAwEMJQEABQACNwcBDxYJBgEGAQYBBgEGAQYBBgEGpgQCGQgEAQEEAgQEVQYCAVkBAwUqAV0BLzAsAy8QGwMxDwsEpgEEYgIe" +
  "Ab8zQIytAUO8AgMbFC4RHQJPAgUqZQFBBQEBAAEEGA8BAgEDARgCAAgHCDMMQwoLGAwBJQgYCwELHQYvAQEEAQIPAQoEBgEYASgG" +
  "AQIBCwIBBwEAAgkCHwEyAQADAQIEAgABABgQAgcLBQIFAgUJBgEGATkGdAEBAQMDCQajVwwWBDAE7UQCaSYGDASJCBkGGQtYAwUC" +
  "BQIFAgIjCwEZARIBAQEOAg0iegUAAQAELAMITQFBLIMBHAMwLyMJHQUlCh0BJAQNKp0BAgkGIwQjBCcIMwsLAQ4BBgEBAQoBDgEG" +
  "AQFDtgIJFQoHGAUBKQEIxRAAATUPBhgJAQECAAwwBAECBgsAAhgHCQkjBQAJEQgiAQILMwkJBAEBDwETCxEBGwMBAQACBQEBPwYB" +
  "AAEDAQ4BCgYuAQINCQgBAQcCAQIVAQYBAQEEAwIBAwIBAgICAAYABQacATcIAQMAARQBAAECHjIGAAEDAgACAwgJpgExBgMCAAIa" +
  "JDIIAQEAAgMLCSYqAQABAQYAAQEGCTYaBQEEAAkWuQEuCQACAGRSDAcCAAIHAQEBHQEBBAABAwECCQlGBwIpCAMBAxsABgECJwYB" +
  "BAcJAAYBAy0NAAIIDUgHCfYBCAElDgcKHAMfGQAHAAIASwYBAQElFQAJCQYFAQEBJAQBAQABAAcJtgISAgMJDgEjCAEBAAEWVgAP" +
  "FCqaB2ZuAQQLwwHMFGINvwgBBbkfxgS5Q7gEBx4BCQRQAQkGHQcACi8HDgoJAQYBFAUSsAVaZUoFNwsMQAEBAAwBDvcvCNUJKgjn" +
  "RQMBBgEBAaICDwAdAgIADgMIiwOEEmoFDAMIBwkCAAIAsCVzPPUBCiYCPQMIEAEHHQQ61wETDBNsGIcBVAFGAQECAAIBAgMBCwEA" +
  "AQYBQAEDAgcBBgEbAQMBBAEAAwYB0wICMgE4ATgBOAE4AQc0/wM3AzIHAQ0BBvQIHgYFhQI9kgEsCgYCCQQBwAIdEisECdYDGwQJ" +
  "5gUGAQMBAQEOkRIeATkGPDkcDSsECAcBrhvfzQIguSAG3QECgS0OsDoP7QSiE50E4gvKJgXfINC4L/3/AwL9/wM=";

const RTL_RANGES_BASE64 =
  "vgsAAQACAAIACRoEBRMAAgABAA0vIgIBZA8BBwEKEwEBAR0dWAsADioJAQQAAxcEAAkAAwAHDgEYBQABCgUeESnFLgCNtgMAAQkB" +
  "DAEEAQABAQEBAXwQ6gISPwI1KAxzBAGGAYMSBQIAASsBAQMAAhYBRwgIMBIBAQUgBBkFAEA3BBMCLg8DAQIBHAoIBwgHPyAkBgsJ" +
  "NQoVAhoFGQcDDAZQSDcyDTIHKdwCKQMAAgFOJwgVCwgWEQQDJhsUFomwA8QBAggwQwcABAkEAZEGQ0w8wgEDARoBAQEAAgABCQED" +
  "AQABAAYABAABAAEAAQIBAQEAAgABAAEAAQABAAEBAQACAwEGAQMBAwEAAQkBEAUCAQQBEA==";

function base64Bytes(encoded) {
  if (typeof globalThis.atob !== "function") {
    throw new Error("Fission CanvasKit requires the standard atob() Web API");
  }
  const binary = globalThis.atob(encoded);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function readVarint(bytes, cursor) {
  let value = 0;
  let shift = 0;
  while (cursor.index < bytes.length && shift <= 28) {
    const byte = bytes[cursor.index];
    cursor.index += 1;
    value += (byte & 0x7f) * 2 ** shift;
    if ((byte & 0x80) === 0) return value;
    shift += 7;
  }
  throw new Error("Fission's embedded Unicode bidi table is corrupt");
}

function decodeRanges(encoded) {
  const bytes = base64Bytes(encoded);
  const cursor = { index: 0 };
  const ranges = [];
  let previousEnd = -1;
  while (cursor.index < bytes.length) {
    const start = previousEnd + 1 + readVarint(bytes, cursor);
    const end = start + readVarint(bytes, cursor);
    if (start <= previousEnd || end < start || end > 0x10ffff) {
      throw new Error("Fission's embedded Unicode bidi table is corrupt");
    }
    ranges.push(Object.freeze([start, end]));
    previousEnd = end;
  }
  return Object.freeze(ranges);
}

const LTR_RANGES = decodeRanges(LTR_RANGES_BASE64);
const RTL_RANGES = decodeRanges(RTL_RANGES_BASE64);

function contains(ranges, codepoint) {
  let low = 0;
  let high = ranges.length - 1;
  while (low <= high) {
    const middle = (low + high) >>> 1;
    const range = ranges[middle];
    if (codepoint < range[0]) high = middle - 1;
    else if (codepoint > range[1]) low = middle + 1;
    else return true;
  }
  return false;
}

/**
 * Resolve Fission's Auto paragraph direction from decoded request scalars.
 * Returns 0 for LTR and 1 for RTL, matching the paragraph wire enums.
 */
export function resolveParagraphDirection(scalars) {
  for (const scalar of scalars) {
    if (contains(RTL_RANGES, scalar.codepoint)) return 1;
    if (contains(LTR_RANGES, scalar.codepoint)) return 0;
  }
  return 0;
}
