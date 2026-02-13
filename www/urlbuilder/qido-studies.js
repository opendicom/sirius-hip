var U = window.SiriusHipUrlBuilder;

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
  var url = new URL("/qido/studies", siriusHipHost);

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
  var token = U.readValue("in-token");

  var outUrl = document.getElementById("out-url");
  if (outUrl) outUrl.value = url;

  var curlUrl = url;
  if (token) {
    try {
      var u = new URL(url);
      u.searchParams.delete("token");
      curlUrl = u.toString();
    } catch (_e) {
      // keep original
    }
  }

  var curl = "curl -G \"" + curlUrl + "\" -H \"content-type: application/json\"";
  if (token) curl += " -H \"Authorization: Bearer " + token + "\"";
  var outCurl = document.getElementById("out-curl");
  if (outCurl) outCurl.textContent = curl;

  var outErrors = document.getElementById("out-errors");
  if (outErrors) {
    if (errors.length) {
      outErrors.classList.remove("d-none");
      outErrors.textContent = errors.join(" ");
    } else {
      outErrors.classList.add("d-none");
      outErrors.textContent = "";
    }
  }

  var btnCopy = document.getElementById("btn-copy");
  var btnOpen = document.getElementById("btn-open");
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

  var btnCopy = document.getElementById("btn-copy");
  if (btnCopy) {
    btnCopy.addEventListener("click", async function () {
      updateOut();
      await U.copyToClipboard(U.readValue("out-url"));
    });
  }

  var btnOpen = document.getElementById("btn-open");
  if (btnOpen) {
    btnOpen.addEventListener("click", function () {
      updateOut();
      U.openInNewTab(U.readValue("out-url"));
    });
  }

  updateOut();
});
