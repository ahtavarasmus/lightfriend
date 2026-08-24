(function () {
  "use strict";

  const websiteId = "dfid_ICHRky5CwoxQQSthciEQz";
  const domain = "lightfriend.ai";
  const trackerId = "lightfriend-datafast-analytics";
  const optOutKey = "lightfriend_analytics_optout";
  const settingsButtonId = "lightfriend-analytics-settings";
  const panelHostId = "lightfriend-analytics-panel";
  const completedGoalPrefix = "lightfriend_datafast_goal_completed:";

  function isOptedOut() {
    try {
      return window.localStorage.getItem(optOutKey) === "true";
    } catch (_) {
      return false;
    }
  }

  function setOptOut(value) {
    try {
      if (value) {
        window.localStorage.setItem(optOutKey, "true");
      } else {
        window.localStorage.removeItem(optOutKey);
      }
    } catch (_) {
      // Best effort; the choice applies to this page only when storage is unavailable.
    }
  }

  function ensureDataFastQueue() {
    window.datafast = window.datafast || function () {
      window.datafast.q = window.datafast.q || [];
      window.datafast.q.push(arguments);
    };
  }

  function loadDataFast() {
    if (isOptedOut()) return;
    ensureDataFastQueue();
    if (document.getElementById(trackerId)) return;

    const tracker = document.createElement("script");
    tracker.id = trackerId;
    tracker.defer = true;
    tracker.dataset.websiteId = websiteId;
    tracker.dataset.domain = domain;
    // Server-side Stripe metadata attribution is used; disable automatic
    // URL-parameter payment detection to avoid duplicate payment events.
    tracker.dataset.disablePayments = "true";
    tracker.src = "https://datafa.st/js/script.js";
    document.head.appendChild(tracker);
  }

  function clearDataFastCookie() {
    const expires = "expires=Thu, 01 Jan 1970 00:00:00 GMT";
    ["datafast_visitor_id", "datafast_session_id"].forEach(function (name) {
      document.cookie = `${name}=; ${expires}; path=/; SameSite=Lax`;
      document.cookie = `${name}=; ${expires}; path=/; domain=${domain}; SameSite=Lax`;
      document.cookie = `${name}=; ${expires}; path=/; domain=.${domain}; SameSite=Lax`;
    });
  }

  function trackPayment(email) {
    if (typeof email !== "string" || email.trim() === "") {
      return false;
    }
    if (isOptedOut()) {
      return false;
    }
    loadDataFast();
    window.datafast("payment", { email: email.trim() });
    return true;
  }

  function normalizeGoal(name, metadata) {
    if (typeof name !== "string" || !/^[a-z][a-z0-9_:-]{0,63}$/.test(name)) {
      return null;
    }

    const normalizedMetadata = {};
    if (metadata && typeof metadata === "object" && !Array.isArray(metadata)) {
      Object.entries(metadata).slice(0, 10).forEach(function ([key, value]) {
        if (/^[a-z][a-z0-9_]{0,63}$/.test(key) && value != null) {
          normalizedMetadata[key] = String(value).slice(0, 200);
        }
      });
    }

    return { name: name, metadata: normalizedMetadata };
  }

  function goalWasCompleted(storageKey) {
    try {
      return window.localStorage.getItem(storageKey) === "true";
    } catch (_) {
      return false;
    }
  }

  function markGoalCompleted(storageKey) {
    try {
      window.localStorage.setItem(storageKey, "true");
    } catch (_) {
      // The current event was still delivered; only refresh deduplication is lost.
    }
  }

  function emitGoal(goal) {
    loadDataFast();
    window.datafast(goal.name, goal.metadata);
    if (goal.storageKey) {
      markGoalCompleted(goal.storageKey);
    }
  }

  function trackGoal(name, metadata) {
    const goal = normalizeGoal(name, metadata);
    if (!goal || isOptedOut()) {
      return false;
    }
    emitGoal(goal);
    return true;
  }

  function trackGoalOnce(name, dedupeKey, metadata) {
    const goal = normalizeGoal(name, metadata);
    if (!goal || typeof dedupeKey !== "string" || dedupeKey.trim() === "" || isOptedOut()) {
      return false;
    }

    goal.storageKey = `${completedGoalPrefix}${goal.name}:${dedupeKey.trim()}`;
    if (goalWasCompleted(goal.storageKey)) {
      return true;
    }

    emitGoal(goal);
    return true;
  }

  function trackHighIntentLink(event) {
    const target = event.target instanceof Element ? event.target : null;
    const link = target?.closest("a[href]");
    if (!link) return;

    let destination;
    try {
      destination = new URL(link.href, window.location.href);
    } catch (_) {
      return;
    }
    if (destination.origin !== window.location.origin) return;

    if (destination.hash === "#plans") {
      trackGoal("plans_click", {
        source_path: window.location.pathname,
        target_path: `${destination.pathname}#plans`,
      });
    }

    if (destination.pathname === "/supported-countries") {
      trackGoal("supported_country_click", {
        source_path: window.location.pathname,
        interaction: "link_click",
      });
    }
  }

  window.lightfriendTrackDataFastPayment = trackPayment;
  window.lightfriendTrackDataFastGoal = trackGoal;
  window.lightfriendTrackDataFastGoalOnce = trackGoalOnce;
  document.addEventListener("click", trackHighIntentLink);

  // --- Quiet opt-out control ("Privacy choices" button + small panel) ---

  function removePanel() {
    document.getElementById(panelHostId)?.remove();
  }

  function showSettingsButton() {
    if (document.getElementById(settingsButtonId)) return;

    const button = document.createElement("button");
    button.id = settingsButtonId;
    button.className = "lf-analytics-settings";
    button.type = "button";
    button.textContent = "Privacy choices";
    button.addEventListener("click", showPanel);
    document.body.appendChild(button);
  }

  function showPanel() {
    if (document.getElementById(panelHostId)) return;

    const host = document.createElement("div");
    host.id = panelHostId;
    host.className = "lf-optout-host";

    const panel = document.createElement("section");
    panel.className = "lf-optout-panel";
    panel.setAttribute("role", "dialog");
    panel.setAttribute("aria-labelledby", "lf-optout-title");

    const title = document.createElement("h2");
    title.id = "lf-optout-title";
    title.textContent = "Privacy choices";

    const copy = document.createElement("p");
    copy.textContent =
      "We use privacy-friendly analytics to understand which pages are useful and how visitors reach us. You can disable this at any time.";

    const actions = document.createElement("div");
    actions.className = "lf-consent-actions";

    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "lf-consent-button lf-consent-reject";
    closeBtn.textContent = "Close";
    closeBtn.addEventListener("click", removePanel);

    const toggleBtn = document.createElement("button");
    toggleBtn.type = "button";
    toggleBtn.className = "lf-consent-button lf-consent-accept";
    if (isOptedOut()) {
      toggleBtn.textContent = "Enable analytics";
      toggleBtn.addEventListener("click", function () {
        setOptOut(false);
        removePanel();
        window.location.reload();
      });
    } else {
      toggleBtn.textContent = "Disable analytics";
      toggleBtn.addEventListener("click", function () {
        setOptOut(true);
        clearDataFastCookie();
        removePanel();
        window.location.reload();
      });
    }

    actions.appendChild(closeBtn);
    actions.appendChild(toggleBtn);
    panel.appendChild(title);
    panel.appendChild(copy);
    panel.appendChild(actions);
    host.appendChild(panel);
    document.body.appendChild(host);
  }

  function initialize() {
    if (isOptedOut()) {
      clearDataFastCookie();
    } else {
      loadDataFast();
    }
    showSettingsButton();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initialize, { once: true });
  } else {
    initialize();
  }
})();
