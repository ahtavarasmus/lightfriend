(function () {
  "use strict";

  const loaderScript = document.currentScript;
  if (!loaderScript) return;

  const websiteId = loaderScript.dataset.websiteId;
  const domain = loaderScript.dataset.domain;
  if (!websiteId || !domain) return;

  const consentKey = "lightfriend_analytics_consent";
  const consentGranted = "granted";
  const consentDenied = "denied";
  const trackerId = "lightfriend-datafast-analytics";
  const bannerHostId = "lightfriend-analytics-consent";
  const settingsButtonId = "lightfriend-analytics-settings";
  const pendingGoalsKey = "lightfriend_datafast_pending_goals";
  const completedGoalPrefix = "lightfriend_datafast_goal_completed:";
  let pendingPaymentEmail = null;

  function readConsent() {
    try {
      return window.localStorage.getItem(consentKey);
    } catch (_) {
      return null;
    }
  }

  function storeConsent(value) {
    try {
      window.localStorage.setItem(consentKey, value);
    } catch (_) {
      // If storage is unavailable, the choice applies to this page only.
    }
  }

  function ensureDataFastQueue() {
    window.datafast = window.datafast || function () {
      window.datafast.q = window.datafast.q || [];
      window.datafast.q.push(arguments);
    };
  }

  function loadDataFast() {
    ensureDataFastQueue();
    if (document.getElementById(trackerId)) return;

    const tracker = document.createElement("script");
    tracker.id = trackerId;
    tracker.defer = true;
    tracker.dataset.websiteId = websiteId;
    tracker.dataset.domain = domain;
    tracker.dataset.disablePayments = "true";
    tracker.src = "https://datafa.st/js/script.js";
    document.head.appendChild(tracker);
  }

  function trackPayment(email) {
    if (typeof email !== "string" || email.trim() === "") {
      return false;
    }

    if (readConsent() !== consentGranted) {
      pendingPaymentEmail = readConsent() === consentDenied ? null : email.trim();
      return false;
    }

    loadDataFast();
    window.datafast("payment", { email: email.trim() });
    pendingPaymentEmail = null;
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

  function readPendingGoals() {
    try {
      const goals = JSON.parse(window.localStorage.getItem(pendingGoalsKey) || "[]");
      return Array.isArray(goals) ? goals : [];
    } catch (_) {
      return [];
    }
  }

  function storePendingGoals(goals) {
    try {
      if (goals.length === 0) {
        window.localStorage.removeItem(pendingGoalsKey);
      } else {
        window.localStorage.setItem(pendingGoalsKey, JSON.stringify(goals.slice(-50)));
      }
    } catch (_) {
      // Goal delivery is best effort when storage is unavailable.
    }
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

  function queueGoal(goal) {
    const pendingGoals = readPendingGoals();
    if (goal.storageKey && pendingGoals.some(function (pending) {
      return pending.storageKey === goal.storageKey;
    })) {
      return;
    }
    pendingGoals.push(goal);
    storePendingGoals(pendingGoals);
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
    if (!goal) return false;

    const consent = readConsent();
    if (consent === consentDenied) return false;
    if (consent !== consentGranted) {
      queueGoal(goal);
      return false;
    }

    emitGoal(goal);
    return true;
  }

  function trackGoalOnce(name, dedupeKey, metadata) {
    const goal = normalizeGoal(name, metadata);
    if (!goal || typeof dedupeKey !== "string" || dedupeKey.trim() === "") {
      return false;
    }

    goal.storageKey = `${completedGoalPrefix}${goal.name}:${dedupeKey.trim()}`;
    if (goalWasCompleted(goal.storageKey)) return true;

    const consent = readConsent();
    if (consent === consentDenied) return false;
    if (consent !== consentGranted) {
      queueGoal(goal);
      return false;
    }

    emitGoal(goal);
    return true;
  }

  function flushPendingGoals() {
    const pendingGoals = readPendingGoals();
    storePendingGoals([]);
    pendingGoals.forEach(function (pending) {
      const goal = normalizeGoal(pending.name, pending.metadata);
      if (!goal) return;
      goal.storageKey = typeof pending.storageKey === "string" ? pending.storageKey : null;
      if (!goal.storageKey || !goalWasCompleted(goal.storageKey)) {
        emitGoal(goal);
      }
    });
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

    if (destination.pathname === "/can-i-leave-my-smartphone") {
      trackGoal("exit_planner_click", {
        source_path: window.location.pathname,
        interaction: "link_click",
      });
    }
  }

  window.lightfriendTrackDataFastPayment = trackPayment;
  window.lightfriendTrackDataFastGoal = trackGoal;
  window.lightfriendTrackDataFastGoalOnce = trackGoalOnce;
  document.addEventListener("click", trackHighIntentLink);

  function clearDataFastCookie() {
    const expires = "expires=Thu, 01 Jan 1970 00:00:00 GMT";
    ["datafast_visitor_id", "datafast_session_id"].forEach(function (name) {
      document.cookie = `${name}=; ${expires}; path=/; SameSite=Lax`;
      document.cookie = `${name}=; ${expires}; path=/; domain=${domain}; SameSite=Lax`;
      document.cookie = `${name}=; ${expires}; path=/; domain=.${domain}; SameSite=Lax`;
    });
  }

  function removeBanner() {
    document.getElementById(bannerHostId)?.remove();
  }

  function showSettingsButton() {
    if (document.getElementById(settingsButtonId)) return;

    const button = document.createElement("button");
    button.id = settingsButtonId;
    button.className = "lf-analytics-settings";
    button.type = "button";
    button.textContent = "Privacy choices";
    button.addEventListener("click", showBanner);
    document.body.appendChild(button);
  }

  function closeBanner() {
    removeBanner();
    showSettingsButton();
  }

  function chooseAnalytics(allowed) {
    if (allowed) {
      storeConsent(consentGranted);
      loadDataFast();
      if (pendingPaymentEmail) {
        trackPayment(pendingPaymentEmail);
      }
      flushPendingGoals();
      closeBanner();
      return;
    }

    const trackerWasLoaded = Boolean(document.getElementById(trackerId));
    storeConsent(consentDenied);
    pendingPaymentEmail = null;
    storePendingGoals([]);
    clearDataFastCookie();

    if (trackerWasLoaded) {
      window.location.reload();
    } else {
      closeBanner();
    }
  }

  function createButton(label, className, onClick) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = className;
    button.textContent = label;
    button.addEventListener("click", onClick);
    return button;
  }

  function showBanner() {
    if (document.getElementById(bannerHostId)) return;

    document.getElementById(settingsButtonId)?.remove();

    const host = document.createElement("div");
    host.id = bannerHostId;
    host.className = "lf-consent-host";

    const banner = document.createElement("section");
    banner.className = "lf-consent-banner";
    banner.setAttribute("role", "region");
    banner.setAttribute("aria-labelledby", "lf-consent-title");

    const copy = document.createElement("div");
    copy.className = "lf-consent-copy";

    const title = document.createElement("h2");
    title.id = "lf-consent-title";
    title.textContent = "Optional analytics";

    const description = document.createElement("p");
    description.append("Help us understand which pages are useful. DataFast only loads if you allow analytics. ");

    const privacyLink = document.createElement("a");
    privacyLink.href = "/privacy#cookies";
    privacyLink.textContent = "Learn more";
    description.appendChild(privacyLink);

    const actions = document.createElement("div");
    actions.className = "lf-consent-actions";
    actions.appendChild(
      createButton("Reject", "lf-consent-button lf-consent-reject", function () {
        chooseAnalytics(false);
      })
    );
    actions.appendChild(
      createButton("Allow analytics", "lf-consent-button lf-consent-accept", function () {
        chooseAnalytics(true);
      })
    );

    copy.appendChild(title);
    copy.appendChild(description);
    banner.appendChild(copy);
    banner.appendChild(actions);
    host.appendChild(banner);
    document.body.appendChild(host);
  }

  function initialize() {
    const consent = readConsent();
    if (consent === consentGranted) {
      loadDataFast();
      flushPendingGoals();
      showSettingsButton();
    } else if (consent === consentDenied) {
      clearDataFastCookie();
      showSettingsButton();
    } else {
      clearDataFastCookie();
      showBanner();
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initialize, { once: true });
  } else {
    initialize();
  }
})();
