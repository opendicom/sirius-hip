function normalizeBaseUrl(raw, fallback) {
    var trimmed = (raw || "").trim();
    if (!trimmed) return fallback;
    if (/^https?:\/\//i.test(trimmed)) return trimmed;
    if (trimmed.startsWith("//")) return window.location.protocol + trimmed;
    return window.location.protocol + "//" + trimmed;
}

function readValue(id) {
    var el = document.getElementById(id);
    if (!el) return "";
    return (el.value || "").trim();
}

function openInNewTab(url) {
    if (!url) return;
    window.open(url, "_blank", "noopener");
}

async function copyToClipboard(text) {
    if (!text) return false;
    try {
        if (navigator.clipboard && navigator.clipboard.writeText) {
            await navigator.clipboard.writeText(text);
            return true;
        }
    } catch (_e) {
        // fallback below
    }

    try {
        var ta = document.createElement("textarea");
        ta.value = text;
        ta.setAttribute("readonly", "");
        ta.style.position = "absolute";
        ta.style.left = "-9999px";
        document.body.appendChild(ta);
        ta.select();
        var ok = document.execCommand("copy");
        document.body.removeChild(ta);
        return ok;
    } catch (_e2) {
        return false;
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

function buildSiriusHipUrl() {
    var protocol = window.location.protocol;
    var hostname = window.location.hostname;
    var defaultSiriusHipHost = protocol + "//" + hostname + ":5001";

    var siriusHipHost = normalizeBaseUrl(readValue("in-SiriusHipHost"), defaultSiriusHipHost);
    var uiAccessType = readValue("in-accessType") || "ohif";
    var serverAccessType = uiAccessType === "osirix" ? "dicom.zip" : uiAccessType;

    var url = new URL("/studyToken", siriusHipHost);
    url.searchParams.set("accessType", serverAccessType);

    var token = readValue("in-token");
    if (token) url.searchParams.set("token", token);

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

    return {
        uiAccessType: uiAccessType,
        siriusHipUrl: url.toString(),
    };
}

function update_url() {
    var ohifProtocol = window.location.protocol;
    var ohifHostname = window.location.hostname;
    var defaultOhifHost = ohifProtocol + "//" + ohifHostname + ":3000";

    var ohifHost = normalizeBaseUrl(readValue("in-OhifHost"), defaultOhifHost);
    var built = buildSiriusHipUrl();

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

    var ohifEl = document.getElementById("in-OhifHost");
    if (ohifEl && !ohifEl.value) ohifEl.value = protocol + "//" + hostname + ":3000";

    var siriusHipEl = document.getElementById("in-SiriusHipHost");
    if (siriusHipEl && !siriusHipEl.value) siriusHipEl.value = protocol + "//" + hostname + ":5001";

    var btnLaunchSiriusHip = document.getElementById("btn-launch-sirius-hip");
    if (btnLaunchSiriusHip) {
        btnLaunchSiriusHip.addEventListener("click", function () {
            openInNewTab(readValue("sirius-hip-url"));
        });
    }

    var btnCopySiriusHip = document.getElementById("btn-copy-sirius-hip");
    if (btnCopySiriusHip) {
        btnCopySiriusHip.addEventListener("click", async function () {
            var ok = await copyToClipboard(readValue("sirius-hip-url"));
            if (ok) flashCopied(btnCopySiriusHip);
        });
    }

    var btnLaunchUrl = document.getElementById("btn-launch-url");
    if (btnLaunchUrl) {
        btnLaunchUrl.addEventListener("click", function () {
            openInNewTab(readValue("url"));
        });
    }

    var btnCopyUrl = document.getElementById("btn-copy-url");
    if (btnCopyUrl) {
        btnCopyUrl.addEventListener("click", async function () {
            var ok = await copyToClipboard(readValue("url"));
            if (ok) flashCopied(btnCopyUrl);
        });
    }

    update_url();
});

function launch() {
    var url = readValue("url");
    if (!url) return;
    openInNewTab(url);
}