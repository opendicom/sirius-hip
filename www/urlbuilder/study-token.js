var U = window.SiriusHipUrlBuilder;

function normalizeBaseUrl(raw, fallback) {
    return U.normalizeBaseUrl(raw, fallback);
}

function readValue(id) {
    return U.readValue(id);
}

function readChecked(id) {
    var el = document.getElementById(id);
    return !!(el && el.checked);
}

function openInNewTab(url) {
    return U.openInNewTab(url);
}

async function copyToClipboard(text) {
    return await U.copyToClipboard(text);
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

function setFieldValidState(id, errorMessage) {
    var el = document.getElementById(id);
    if (!el) return;
    if (!errorMessage) {
        el.classList.remove("is-invalid");
        el.removeAttribute("aria-invalid");
        el.removeAttribute("title");
        return;
    }
    el.classList.add("is-invalid");
    el.setAttribute("aria-invalid", "true");
    el.setAttribute("title", errorMessage);
}

function splitBackslashList(raw) {
    return U.splitBackslashList(raw);
}

function isValidUidToken(token) {
    return U.isValidUidToken(token);
}

function isValidModalityToken(token) {
    return U.isValidModalityToken(token);
}

function isValidStudyDate(raw) {
    // Accept:
    //  YYYY-MM-DD
    //  YYYY-MM-DD|
    //  |YYYY-MM-DD
    //  YYYY-MM-DD|YYYY-MM-DD
    var value = (raw || "").trim();
    if (!value) return true;

    if (value.indexOf("|") === -1) {
        return /^\d{4}-\d{2}-\d{2}$/.test(value);
    }

    var parts = value.split("|");
    if (parts.length !== 2) return false;

    var left = (parts[0] || "").trim();
    var right = (parts[1] || "").trim();

    var dateRe = /^\d{4}-\d{2}-\d{2}$/;
    if (left && !dateRe.test(left)) return false;
    if (right && !dateRe.test(right)) return false;

    // Disallow just "|" with both sides empty.
    if (!left && !right) return false;
    return true;
}

function isValidHttpUrl(raw) {
    return U.isValidHttpUrl(raw);
}

function validateInputs() {
    var errors = [];

    // Clear previous invalid states
    var idsToValidate = [
        "in-accessType",
        "in-proxyURI",
        "in-max",
        "in-StudyDate",
        "in-StudyInstanceUID",
        "in-SeriesInstanceUID",
        "in-cuidsInStudy",
        "in-Modality",
        "in-ModalityOff",
        "in-SOPClass",
        "in-SOPClassOff",
    ];
    idsToValidate.forEach(function (id) { setFieldValidState(id, ""); });

    // accessType (server accepted) vs UI-only osirix
    var uiAccessType = readValue("in-accessType") || "ohif";
    var serverAccessType = uiAccessType === "osirix" ? "dicom.zip" : uiAccessType;
    var allowedAccessTypes = {
        "ohif": true,
        "weasis.xml": true,
        "dicom.zip": true,
        "cornerstone.json": true,
    };
    if (!allowedAccessTypes[serverAccessType]) {
        var msg = "Invalid accessType. Allowed values: ohif, weasis.xml, dicom.zip, cornerstone.json";
        setFieldValidState("in-accessType", msg);
        errors.push(msg);
    }

    // proxyURI
    var proxyURI = readValue("in-proxyURI");
    if (proxyURI && !isValidHttpUrl(proxyURI)) {
        var msgProxy = "proxyURI must be a valid http(s) URL (e.g. https://host:5001)";
        setFieldValidState("in-proxyURI", msgProxy);
        errors.push(msgProxy);
    }

    // max
    var maxRaw = readValue("in-max");
    if (maxRaw) {
        if (!/^\d+$/.test(maxRaw)) {
            var msgMax = "max must be an integer (u64)";
            setFieldValidState("in-max", msgMax);
            errors.push(msgMax);
        } else if (parseInt(maxRaw, 10) <= 0) {
            var msgMax2 = "max must be > 0 (or empty to use the default)";
            setFieldValidState("in-max", msgMax2);
            errors.push(msgMax2);
        }
    }

    // StudyDate
    var studyDate = readValue("in-StudyDate");
    if (studyDate && !isValidStudyDate(studyDate)) {
        var msgDate = "Invalid StudyDate. Use YYYY-MM-DD, YYYY-MM-DD|, |YYYY-MM-DD, or YYYY-MM-DD|YYYY-MM-DD";
        setFieldValidState("in-StudyDate", msgDate);
        errors.push(msgDate);
    }

    // UID lists and UID-like fields
    var studyIuids = splitBackslashList(readValue("in-StudyInstanceUID"));
    if (studyIuids.some(function (x) { return !isValidUidToken(x); })) {
        var msgUid1 = "StudyInstanceUID must be UID(s) containing digits and dots, separated by \"\\\" (backslash)";
        setFieldValidState("in-StudyInstanceUID", msgUid1);
        errors.push(msgUid1);
    }

    var seriesIuids = splitBackslashList(readValue("in-SeriesInstanceUID"));
    if (seriesIuids.some(function (x) { return !isValidUidToken(x); })) {
        var msgUid2 = "SeriesInstanceUID must be UID(s) containing digits and dots, separated by \"\\\" (backslash)";
        setFieldValidState("in-SeriesInstanceUID", msgUid2);
        errors.push(msgUid2);
    }

    var cuids = splitBackslashList(readValue("in-cuidsInStudy"));
    if (cuids.length > 0 && cuids.some(function (x) { return !isValidUidToken(x); })) {
        var msgCuid = "cuidsInStudy must be OID/UID(s) containing digits and dots, separated by \"\\\" (backslash)";
        setFieldValidState("in-cuidsInStudy", msgCuid);
        errors.push(msgCuid);
    }

    var modality = readValue("in-Modality");
    if (modality && !isValidModalityToken(modality)) {
        var msgMod = "Invalid Modality (use a short token like CT, MR, US)";
        setFieldValidState("in-Modality", msgMod);
        errors.push(msgMod);
    }

    var modalityOff = splitBackslashList(readValue("in-ModalityOff"));
    if (modalityOff.some(function (x) { return !isValidModalityToken(x); })) {
        var msgModOff = "ModalityOff must be modality token(s) separated by \"\\\" (backslash)";
        setFieldValidState("in-ModalityOff", msgModOff);
        errors.push(msgModOff);
    }

    var sopClass = readValue("in-SOPClass");
    if (sopClass && !isValidUidToken(sopClass)) {
        var msgSop = "SOPClass must be a UID (digits and dots)";
        setFieldValidState("in-SOPClass", msgSop);
        errors.push(msgSop);
    }

    var sopClassOff = splitBackslashList(readValue("in-SOPClassOff"));
    if (sopClassOff.some(function (x) { return !isValidUidToken(x); })) {
        var msgSopOff = "SOPClassOff must be UID(s) separated by \"\\\" (backslash)";
        setFieldValidState("in-SOPClassOff", msgSopOff);
        errors.push(msgSopOff);
    }

    return {
        ok: errors.length === 0,
        messages: errors,
    };
}

function applyValidationUi(validation) {
    setValidationAlert(validation.messages);

    var btnIds = ["btn-launch-sirius-hip", "btn-copy-sirius-hip", "btn-launch-url", "btn-copy-url"];
    btnIds.forEach(function (id) {
        var btn = document.getElementById(id);
        if (btn) btn.disabled = !validation.ok;
    });
}

function validateOrFocusFirstInvalid() {
    var validation = validateInputs();
    applyValidationUi(validation);

    if (!validation.ok) {
        var firstInvalid = document.querySelector(".is-invalid");
        if (firstInvalid && typeof firstInvalid.focus === "function") {
            firstInvalid.focus();
        }
    }
    return validation.ok;
}

function buildSiriusHipUrl() {
    var protocol = window.location.protocol;
    var hostname = window.location.hostname;
    var defaultSiriusHipHost = protocol === "file:" || !hostname ? "http://localhost:5001" : protocol + "//" + hostname + ":5001";

    var siriusHipHost = normalizeBaseUrl(readValue("in-SiriusHipHost"), defaultSiriusHipHost);
    var uiAccessType = readValue("in-accessType") || "ohif";
    var serverAccessType = uiAccessType === "osirix" ? "dicom.zip" : uiAccessType;

    var url = new URL("/studyToken", siriusHipHost);
    url.searchParams.set("accessType", serverAccessType);

    var session = readValue("in-session");
    if (session) url.searchParams.set("session", session);

    var proxyURI = readValue("in-proxyURI");
    if (proxyURI) url.searchParams.set("proxyURI", proxyURI);

    var institution = readValue("in-institution");
    if (institution) url.searchParams.set("institution", institution);

    var max = readValue("in-max");
    if (max) url.searchParams.set("max", max);

    var AccessionNumber = readValue("in-AccessionNumber");
    if (AccessionNumber) url.searchParams.set("AccessionNumber", AccessionNumber);

    var PatientID = readValue("in-PatientID");
    if (PatientID) url.searchParams.set("PatientID", PatientID);

    var patient = readValue("in-patient");
    if (patient) url.searchParams.set("patient", patient);

    var StudyInstanceUID = readValue("in-StudyInstanceUID");
    if (StudyInstanceUID) url.searchParams.set("StudyInstanceUID", StudyInstanceUID);

    var StudyID = readValue("in-StudyID");
    if (StudyID) url.searchParams.set("StudyID", StudyID);

    var StudyDate = readValue("in-StudyDate");
    if (StudyDate) url.searchParams.set("StudyDate", StudyDate);

    var ModalityInStudy = readValue("in-ModalityInStudy");
    if (ModalityInStudy) url.searchParams.set("ModalityInStudy", ModalityInStudy);

    var cuidsInStudy = readValue("in-cuidsInStudy");
    if (cuidsInStudy) url.searchParams.set("cuidsInStudy", cuidsInStudy);

    var SeriesInstanceUID = readValue("in-SeriesInstanceUID");
    if (SeriesInstanceUID) url.searchParams.set("SeriesInstanceUID", SeriesInstanceUID);

    var SeriesDescription = readValue("in-SeriesDescription");
    if (SeriesDescription) url.searchParams.set("SeriesDescription", SeriesDescription);

    var SeriesNumber = readValue("in-SeriesNumber");
    if (SeriesNumber) url.searchParams.set("SeriesNumber", SeriesNumber);

    var Modality = readValue("in-Modality");
    if (Modality) url.searchParams.set("Modality", Modality);

    var ModalityOff = readValue("in-ModalityOff");
    if (ModalityOff) url.searchParams.set("ModalityOff", ModalityOff);

    var SOPClass = readValue("in-SOPClass");
    if (SOPClass) url.searchParams.set("SOPClass", SOPClass);

    var SOPClassOff = readValue("in-SOPClassOff");
    if (SOPClassOff) url.searchParams.set("SOPClassOff", SOPClassOff);

    var token = readValue("in-token");
    var curlUrl = url.toString();
    if (token && readChecked("in-token-in-query")) url.searchParams.set("token", token);

    return {
        uiAccessType: uiAccessType,
        siriusHipUrl: url.toString(),
        curlUrl: curlUrl,
        token: token,
    };
}

function update_url() {
    var ohifProtocol = window.location.protocol;
    var ohifHostname = window.location.hostname;
    var defaultOhifHost = ohifProtocol === "file:" || !ohifHostname ? "http://localhost:3000" : ohifProtocol + "//" + ohifHostname + ":3000";

    var ohifHost = normalizeBaseUrl(readValue("in-OhifHost"), defaultOhifHost);
    var built = buildSiriusHipUrl();

    applyValidationUi(validateInputs());

    var value;
    switch (built.uiAccessType) {
        case "weasis.xml":
            value = "weasis://" + encodeURIComponent("$dicom:get -w \"" + built.siriusHipUrl + "\"");
            break;

        case "osirix":
            value = "osirix://?methodName=downloadURL&URL=" + encodeURIComponent(built.siriusHipUrl);
            break;

        case "ohif":
            value = ohifHost + "/viewer/dicomjson?url=" + encodeURIComponent(built.siriusHipUrl);
            break;

        default:
            value = built.siriusHipUrl;
            break;
    }

    var siriusHipUrlEl = document.getElementById("sirius-hip-url");
    if (siriusHipUrlEl) siriusHipUrlEl.value = built.siriusHipUrl;

    var curlEl = document.getElementById("curl");
    if (curlEl) {
        var curl = "curl -G \"" + built.curlUrl + "\"";
        if (built.token) curl += " -H \"Authorization: Bearer " + built.token + "\"";
        curlEl.value = curl;
    }

    var urlEl = document.getElementById("url");
    if (urlEl) urlEl.value = value;
}

function shouldTriggerUpdate(target) {
    if (!target || !target.id) return false;
    return target.id.startsWith("in-");
}

document.addEventListener("input", function (e) {
    if (shouldTriggerUpdate(e.target)) update_url();
});

document.addEventListener("change", function (e) {
    if (shouldTriggerUpdate(e.target)) update_url();
});

document.addEventListener("DOMContentLoaded", function () {
    var protocol = window.location.protocol;
    var hostname = window.location.hostname;
    var defaultOhifHost = protocol === "file:" || !hostname ? "http://localhost:3000" : protocol + "//" + hostname + ":3000";
    var defaultSiriusHipHost = protocol === "file:" || !hostname ? "http://localhost:5001" : protocol + "//" + hostname + ":5001";

    var ohifEl = document.getElementById("in-OhifHost");
    if (ohifEl && !ohifEl.value) ohifEl.value = defaultOhifHost;

    var siriusHipEl = document.getElementById("in-SiriusHipHost");
    if (siriusHipEl && !siriusHipEl.value) siriusHipEl.value = defaultSiriusHipHost;

    var btnLaunchSiriusHip = document.getElementById("btn-launch-sirius-hip");
    if (btnLaunchSiriusHip) {
        btnLaunchSiriusHip.addEventListener("click", function () {
            update_url();
            if (!validateOrFocusFirstInvalid()) return;
            openInNewTab(readValue("sirius-hip-url"));
        });
    }

    var btnCopySiriusHip = document.getElementById("btn-copy-sirius-hip");
    if (btnCopySiriusHip) {
        btnCopySiriusHip.addEventListener("click", async function () {
            update_url();
            if (!validateOrFocusFirstInvalid()) return;
            var ok = await copyToClipboard(readValue("sirius-hip-url"));
            if (ok) flashCopied(btnCopySiriusHip);
        });
    }

    var btnLaunchUrl = document.getElementById("btn-launch-url");
    if (btnLaunchUrl) {
        btnLaunchUrl.addEventListener("click", function () {
            update_url();
            if (!validateOrFocusFirstInvalid()) return;
            openInNewTab(readValue("url"));
        });
    }

    var btnCopyUrl = document.getElementById("btn-copy-url");
    if (btnCopyUrl) {
        btnCopyUrl.addEventListener("click", async function () {
            update_url();
            if (!validateOrFocusFirstInvalid()) return;
            var ok = await copyToClipboard(readValue("url"));
            if (ok) flashCopied(btnCopyUrl);
        });
    }

    var btnCopyCurl = document.getElementById("btn-copy-curl");
    if (btnCopyCurl) {
        btnCopyCurl.addEventListener("click", async function () {
            update_url();
            if (!validateOrFocusFirstInvalid()) return;
            var curlEl = document.getElementById("curl");
            var ok = await copyToClipboard(curlEl ? curlEl.value : "");
            if (ok) flashCopied(btnCopyCurl);
        });
    }

    update_url();
});

function launch() {
    var url = readValue("url");
    if (!url) return;
    openInNewTab(url);
}