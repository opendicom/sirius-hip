var U = window.SiriusHipUrlBuilder;

function updateBuiltUrlBarPadding() {
  var bar = document.querySelector("nav.navbar.fixed-bottom");
  if (!bar) return;

  var h = bar.offsetHeight || 0;
  if (h > 0) {
    document.documentElement.style.setProperty("--built-url-bar-height", h + "px");
  }
}

function flashCopied(btn) {
  if (!btn) return;
  var oldTitle = btn.getAttribute("title") || "";
  btn.setAttribute("title", "Copied");
  btn.classList.add("btn-success");
  btn.classList.remove("btn-outline-secondary");
  window.setTimeout(function () {
    btn.setAttribute("title", oldTitle);
    btn.classList.remove("btn-success");
    btn.classList.add("btn-outline-secondary");
  }, 900);
}

function setValidationAlert(messages) {
  var alertEl = document.getElementById("validation-alert");
  if (!alertEl) return;

  if (!messages || messages.length === 0) {
    alertEl.classList.add("d-none");
    alertEl.textContent = "";
    return;
  }

  alertEl.classList.remove("d-none");
  alertEl.textContent = messages.join("  ");
}

function validateInputs() {
  var errors = [];

  var limit = U.readValue("in-limit");
  if (!U.isPositiveIntString(limit)) errors.push("limit must be a positive integer.");

  var offset = U.readValue("in-offset");
  if (!U.isUnsignedIntString(offset)) errors.push("offset must be an unsigned integer.");

  errors = errors.concat(U.validateUidListBackslash(U.readValue("in-StudyInstanceUID"), "StudyInstanceUID"));
  errors = errors.concat(U.validateStudyDateDICOMRange(U.readValue("in-StudyDate")));
  errors = errors.concat(U.validateStudyTimeDigits(U.readValue("in-StudyTime")));

  return errors;
}

function buildUrl() {
  var protocol = window.location.protocol;
  var hostname = window.location.hostname;
  var defaultHost = protocol === "file:" || !hostname ? "http://localhost:5001" : protocol + "//" + hostname + ":5001";

  var siriusHipHost = U.normalizeBaseUrl(U.readValue("in-SiriusHipHost"), defaultHost);
  var url = U.urlWithPath(siriusHipHost, "qido/studies");

  function setParam(key, value) {
    if (!value) return;
    url.searchParams.set(key, value);
  }

  setParam("PatientID", U.readValue("in-PatientID"));
  setParam("PatientName", U.readValue("in-PatientName"));
  setParam("ReferringPhysicianName", U.readValue("in-ReferringPhysicianName"));
  setParam("AccessionNumber", U.readValue("in-AccessionNumber"));
  setParam("ModalitiesInStudy", U.readValue("in-ModalitiesInStudy"));
  setParam("StudyInstanceUID", U.readValue("in-StudyInstanceUID"));
  setParam("StudyID", U.readValue("in-StudyID"));
  setParam("StudyDate", U.readValue("in-StudyDate"));
  setParam("StudyTime", U.readValue("in-StudyTime"));

  var limit = U.readValue("in-limit");
  if (limit) setParam("limit", limit);

  var offset = U.readValue("in-offset");
  if (offset) setParam("offset", offset);

  if (U.readChecked("in-include-StudyDescription")) url.searchParams.append("includefield", "StudyDescription");
  if (U.readChecked("in-include-SOPClassesInStudy")) url.searchParams.append("includefield", "SOPClassesInStudy");
  if (U.readChecked("in-include-IssuerOfPatientID")) url.searchParams.append("includefield", "IssuerOfPatientID");

  var token = U.readValue("in-token");
  var tokenInQuery = U.readChecked("in-token-in-query");
  if (token && tokenInQuery) url.searchParams.set("token", token);

  return url.toString();
}

function updateOut() {
  var errors = validateInputs();
  var url = buildUrl();

  var outUrl = document.getElementById("url");
  if (outUrl) outUrl.value = url;

  setValidationAlert(errors);

  var btnCopy = document.getElementById("btn-copy-url");
  var btnOpen = document.getElementById("btn-launch-url");
  var disabled = errors.length > 0;
  if (btnCopy) btnCopy.disabled = disabled;
  if (btnOpen) btnOpen.disabled = disabled;
}

function shouldTriggerUpdate(target) {
  if (!target || !target.id) return false;
  return target.id.startsWith("in-");
}

document.addEventListener("input", function (e) {
  if (shouldTriggerUpdate(e.target)) updateOut();
});

document.addEventListener("change", function (e) {
  if (shouldTriggerUpdate(e.target)) updateOut();
});

document.addEventListener("DOMContentLoaded", function () {
  var protocol = window.location.protocol;
  var hostname = window.location.hostname;
  var defaultHost = protocol === "file:" || !hostname ? "http://localhost:5001" : protocol + "//" + hostname + ":5001";

  var siriusHipEl = document.getElementById("in-SiriusHipHost");
  if (siriusHipEl && !siriusHipEl.value) siriusHipEl.value = defaultHost;

  var btnCopy = document.getElementById("btn-copy-url");
  if (btnCopy) {
    btnCopy.addEventListener("click", async function () {
      updateOut();
      var ok = await U.copyToClipboard(U.readValue("url"));
      if (ok) flashCopied(btnCopy);
    });
  }

  var btnOpen = document.getElementById("btn-launch-url");
  if (btnOpen) {
    btnOpen.addEventListener("click", function () {
      updateOut();
      U.openInNewTab(U.readValue("url"));
    });
  }

  updateOut();

  updateBuiltUrlBarPadding();

  var resizeTimer;
  window.addEventListener("resize", function () {
    window.clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(updateBuiltUrlBarPadding, 100);
  });

  if (window.ResizeObserver) {
    var bar = document.querySelector("nav.navbar.fixed-bottom");
    if (bar) {
      try {
        new ResizeObserver(updateBuiltUrlBarPadding).observe(bar);
      } catch (_e) {
        // ignore
      }
    }
  }
});
