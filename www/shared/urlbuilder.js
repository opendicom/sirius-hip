(function (global) {
  "use strict";

  var U = {};

  U.normalizeBaseUrl = function normalizeBaseUrl(raw, fallback) {
    var trimmed = (raw || "").trim();
    if (!trimmed) return fallback;
    if (/^https?:\/\//i.test(trimmed)) return trimmed;
    var currentProto = global.location && global.location.protocol ? global.location.protocol : "http:";
    var scheme = currentProto === "file:" ? "http:" : currentProto;
    if (trimmed.startsWith("//")) return scheme + trimmed;
    return scheme + "//" + trimmed;
  };

  U.readValue = function readValue(id) {
    var el = global.document.getElementById(id);
    if (!el) return "";
    return (el.value || "").trim();
  };

  U.readChecked = function readChecked(id) {
    var el = global.document.getElementById(id);
    return !!(el && el.checked);
  };

  U.openInNewTab = function openInNewTab(url) {
    if (!url) return;
    global.open(url, "_blank", "noopener");
  };

  U.copyToClipboard = async function copyToClipboard(text) {
    if (!text) return false;
    try {
      if (global.navigator.clipboard && global.navigator.clipboard.writeText) {
        await global.navigator.clipboard.writeText(text);
        return true;
      }
    } catch (_e) {
      // fallback below
    }

    try {
      var ta = global.document.createElement("textarea");
      ta.value = text;
      ta.setAttribute("readonly", "");
      ta.style.position = "absolute";
      ta.style.left = "-9999px";
      global.document.body.appendChild(ta);
      ta.select();
      var ok = global.document.execCommand("copy");
      global.document.body.removeChild(ta);
      return ok;
    } catch (_e2) {
      return false;
    }
  };

  U.splitBackslashList = function splitBackslashList(raw) {
    var value = (raw || "").trim();
    if (!value) return [];
    return value
      .split("\\")
      .map(function (x) {
        return (x || "").trim();
      })
      .filter(function (x) {
        return !!x;
      });
  };

  U.isUnsignedIntString = function isUnsignedIntString(value) {
    if (!value) return true;
    return /^\d+$/.test(value);
  };

  U.isPositiveIntString = function isPositiveIntString(value) {
    if (!value) return true;
    return /^\d+$/.test(value) && parseInt(value, 10) > 0;
  };

  U.isValidUidToken = function isValidUidToken(token) {
    // Practical DICOM UID validator: digits and dots.
    return /^[0-9]+(\.[0-9]+)*$/.test(token || "");
  };

  U.isValidModalityToken = function isValidModalityToken(token) {
    return /^[A-Za-z0-9]{1,16}$/.test(token || "");
  };

  U.isValidHttpUrl = function isValidHttpUrl(raw) {
    var value = (raw || "").trim();
    if (!value) return true;
    try {
      var url = new URL(value);
      return url.protocol === "http:" || url.protocol === "https:";
    } catch (_e) {
      return false;
    }
  };

  U.validateUidListBackslash = function validateUidListBackslash(raw, label) {
    var errors = [];
    if (!raw) return errors;

    var parts = U.splitBackslashList(raw);
    for (var i = 0; i < parts.length; i++) {
      if (!U.isValidUidToken(parts[i])) {
        errors.push(label + " must be UID(s) containing digits and dots, separated by \"\\\\\" (backslash). ");
        break;
      }
    }
    return errors;
  };

  U.validateStudyDatePipe = function validateStudyDatePipe(raw) {
    // Accept:
    //  YYYY-MM-DD
    //  YYYY-MM-DD|
    //  |YYYY-MM-DD
    //  YYYY-MM-DD|YYYY-MM-DD
    var value = (raw || "").trim();
    if (!value) return [];

    if (value.indexOf("|") === -1) {
      return /^\d{4}-\d{2}-\d{2}$/.test(value) ? [] : ["Invalid StudyDate."];
    }

    var parts = value.split("|");
    if (parts.length !== 2) return ["Invalid StudyDate."];

    var left = (parts[0] || "").trim();
    var right = (parts[1] || "").trim();

    var dateRe = /^\d{4}-\d{2}-\d{2}$/;
    if (left && !dateRe.test(left)) return ["Invalid StudyDate."];
    if (right && !dateRe.test(right)) return ["Invalid StudyDate."];

    if (!left && !right) return ["Invalid StudyDate."];
    return [];
  };

  U.validateStudyDateDICOMRange = function validateStudyDateDICOMRange(raw) {
    if (!raw) return [];
    var value = (raw || "").trim();
    if (!value) return [];

    // Accept: YYYYMMDD, YYYYMMDD-, -YYYYMMDD, YYYYMMDD-YYYYMMDD.
    // Also accept ISO-like strings by extracting digits.
    var digits = value.replace(/\D/g, "");
    if (digits.length !== 8 && digits.length !== 16) {
      return ["StudyDate must contain 8 digits (exact) or 16 digits (range). "];
    }
    if (value.indexOf("-") >= 0) return [];
    if (digits.length === 16) return ["StudyDate looks like a range but has no '-' delimiter. "];
    return [];
  };

  U.validateStudyTimeDigits = function validateStudyTimeDigits(raw) {
    if (!raw) return [];
    var digits = (raw || "").replace(/\D/g, "");
    if (digits.length !== 4 && digits.length !== 6) {
      return ["StudyTime must be HHMM or HHMMSS (digits only). "];
    }
    return [];
  };

  global.SiriusHipUrlBuilder = U;
})(window);
