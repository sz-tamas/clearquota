let pendingNavigation = null;

function renderLucideIcons() {
  window.lucide?.createIcons({ attrs: { "stroke-width": 1.75 } });
}

function setSidebarOpen(open) {
  const sidebar = document.querySelector("[data-sidebar]");
  const backdrop = document.querySelector("[data-sidebar-backdrop]");
  const toggle = document.querySelector("[data-sidebar-toggle]");
  if (!sidebar || !backdrop || !toggle) return;
  const isMobile = window.innerWidth < 800;
  const showDrawer = open && isMobile;
  sidebar.classList.toggle("-translate-x-full", !showDrawer);
  sidebar.toggleAttribute("inert", isMobile && !showDrawer);
  sidebar.setAttribute("aria-hidden", String(isMobile && !showDrawer));
  backdrop.classList.toggle("hidden", !showDrawer);
  toggle.setAttribute("aria-expanded", String(showDrawer));
}

function showSettingsTab(tab) {
  const project = document.querySelector("#settings-project-panel");
  const privacy = document.querySelector("#settings-privacy-panel");
  if (!project || !privacy) return;
  const showPrivacy = tab === "privacy";
  project.classList.toggle("hidden", showPrivacy);
  privacy.classList.toggle("hidden", !showPrivacy);
  privacy.classList.toggle("grid", showPrivacy);
  document.querySelectorAll("[data-settings-tab]").forEach((button) => {
    const active = button.dataset.settingsTab === tab;
    button.setAttribute("aria-selected", String(active));
    button.classList.toggle("border-brand", active);
    button.classList.toggle("bg-brand-light", active);
    button.classList.toggle("text-brand", active);
    button.classList.toggle("border-transparent", !active);
    button.classList.toggle("text-slate-500", !active);
    button.classList.toggle("hover:border-slate-300", !active);
    button.classList.toggle("hover:bg-slate-50", !active);
    button.classList.toggle("hover:text-slate-700", !active);
  });
}

function initializeSettingsTabs() {
  const tab = window.sessionStorage.getItem("clearquota-settings-return-tab");
  window.sessionStorage.removeItem("clearquota-settings-return-tab");
  showSettingsTab(tab === "privacy" ? "privacy" : "project");
}

function layoutProviderCards(grid) {
  const styles = getComputedStyle(grid);
  const rowHeight = Number.parseFloat(styles.gridAutoRows);
  const rowGap = Number.parseFloat(styles.rowGap);
  if (!rowHeight || Number.isNaN(rowGap)) return;

  grid.querySelectorAll("[data-provider-card]").forEach((card) => {
    card.style.gridRowEnd = `span ${Math.ceil((card.offsetHeight + rowGap) / (rowHeight + rowGap))}`;
  });
}

function initializeProviderCardGrids() {
  document.querySelectorAll("[data-provider-grid]").forEach((grid) => {
    if (!grid.dataset.providerGridObserved) {
      grid.dataset.providerGridObserved = "true";
      new ResizeObserver(() => requestAnimationFrame(() => layoutProviderCards(grid))).observe(grid);
    }
    layoutProviderCards(grid);
  });
}

function setProviderMenuOpen(open) {
  const menu = document.querySelector("[data-provider-menu]");
  const toggle = document.querySelector("[data-provider-menu-toggle]");
  menu?.classList.toggle("hidden", !open);
  toggle?.setAttribute("aria-expanded", String(open));
}

function renderDashboardRefreshConfirmation(button) {
  const dialog = document.querySelector("#provider-dialog");
  if (!dialog) return;

  dialog.innerHTML = `<div class="fixed inset-0 z-30 grid place-items-center bg-slate-950/35 p-4"><section class="w-full max-w-md rounded-2xl bg-white p-7 shadow-2xl" role="dialog" aria-modal="true" aria-labelledby="refresh-dialog-title"><p class="text-xs font-bold tracking-[.15em] text-brand uppercase">Refresh providers</p><h2 id="refresh-dialog-title" class="mt-1 text-xl font-bold text-ink">Refresh ${button.dataset.refreshPeriod}?</h2><p class="mt-3 text-sm leading-6 text-slate-600">Fetch available usage for this month from every configured provider. Existing stored data for the month will be updated.</p><div class="mt-7 flex justify-end gap-3"><button class="rounded-lg px-4 py-2.5 text-sm font-semibold text-slate-600 hover:bg-slate-100" onclick="document.getElementById('provider-dialog').innerHTML=''">Cancel</button><button class="inline-flex items-center gap-2 rounded-lg bg-brand px-4 py-2.5 text-sm font-semibold text-white" data-refresh-submit hx-post="${button.dataset.refreshUrl}" hx-sync="this:drop" hx-target="#provider-dialog" hx-swap="innerHTML"><i class="size-4" data-lucide="refresh-cw" aria-hidden="true"></i>Start refresh</button></div></section></div>`;
  window.htmx?.process(dialog);
  renderLucideIcons();
}

window.openDashboardRefreshConfirmation = renderDashboardRefreshConfirmation;

function renderDashboardRefreshStatus(title, body, spinning) {
  const dialog = document.querySelector("#provider-dialog");
  if (!dialog) return;
  dialog.innerHTML = `<div class="fixed inset-0 z-30 grid place-items-center bg-slate-950/35 p-4"><section class="w-full max-w-md rounded-2xl bg-white p-7 text-center shadow-2xl" role="dialog" aria-modal="true" aria-live="polite" aria-labelledby="refresh-dialog-title">${spinning ? '<span class="mx-auto grid size-14 place-items-center rounded-full bg-brand/10"><i class="size-7 animate-spin text-brand" data-lucide="loader-circle" aria-hidden="true"></i></span>' : ''}<p class="mt-5 text-xs font-bold tracking-[.15em] text-brand uppercase">Refreshing providers</p><h2 id="refresh-dialog-title" class="mt-1 text-xl font-bold text-ink">${title}</h2><p class="mt-3 text-sm leading-6 text-slate-600">${body}</p>${spinning ? '' : '<div class="mt-7"><button class="rounded-lg bg-brand px-4 py-2.5 text-sm font-semibold text-white" onclick="document.getElementById(\'provider-dialog\').innerHTML=\'\'">Close</button></div>'}</section></div>`;
  renderLucideIcons();
}

document.addEventListener("htmx:beforeRequest", (event) => {
  if (event.detail.elt.matches("[data-refresh-submit]")) {
    renderDashboardRefreshStatus("Refreshing…", "Fetching the selected month from your providers. This can take a moment.", true);
  }
});

function showDashboardRefreshError(event) {
  if (event.detail.elt.matches("[data-refresh-submit]")) {
    renderDashboardRefreshStatus("Provider refresh failed", "We could not complete the refresh. Please try again.", false);
  }
}

document.addEventListener("htmx:responseError", showDashboardRefreshError);
document.addEventListener("htmx:sendError", showDashboardRefreshError);

function navigate(path, pushHistory) {
  pendingNavigation = { path, pushHistory };
  window.htmx?.ajax("GET", path, {
    target: "#app",
    select: "#app",
    swap: "outerHTML",
    pushUrl: false,
  });
}

document.addEventListener("click", (event) => {
  const providerRow = event.target.closest("[data-provider-row]");
  if (providerRow && !event.target.closest("button:not([data-provider-row-toggle]), a")) {
    const toggle = providerRow.querySelector("[data-provider-row-toggle]");
    const details = document.getElementById(toggle.getAttribute("aria-controls"));
    const open = toggle.getAttribute("aria-expanded") !== "true";
    toggle.setAttribute("aria-expanded", String(open));
    details.hidden = !open;
  }

  const sidebarToggle = event.target.closest("[data-sidebar-toggle]");
  if (sidebarToggle) {
    setSidebarOpen(sidebarToggle.getAttribute("aria-expanded") !== "true");
  } else if (event.target.closest("[data-sidebar-backdrop], [data-sidebar-nav]")) {
    setSidebarOpen(false);
  }

  const settingsTab = event.target.closest("[data-settings-tab]");
  if (settingsTab) showSettingsTab(settingsTab.dataset.settingsTab);

  const providerToggle = event.target.closest("[data-provider-menu-toggle]");
  if (providerToggle) {
    setProviderMenuOpen(providerToggle.getAttribute("aria-expanded") !== "true");
  } else if (!event.target.closest("[data-provider-menu]")) {
    setProviderMenuOpen(false);
  } else if (event.target.closest("[data-provider-menu] a")) {
    setProviderMenuOpen(false);
  }

	const link = event.target.closest("[data-sidebar-nav][href^='/']");
  if (!link || event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;

  event.preventDefault();
  navigate(link.getAttribute("href"), true);
});

document.addEventListener("htmx:beforeSwap", (event) => {
  if (event.detail.xhr.status === 404 && pendingNavigation) {
    event.detail.shouldSwap = true;
    event.detail.isError = false;
  }
});

document.addEventListener("htmx:afterSwap", (event) => {
  if (event.detail.target.id !== "app") return;

  setSidebarOpen(false);
  initializeSettingsTabs();
  initializeProviderCardGrids();
  renderLucideIcons();

  if (!pendingNavigation) return;

  if (pendingNavigation.pushHistory) {
    window.history.pushState({}, "", pendingNavigation.path);
  }
  pendingNavigation = null;
});

document.addEventListener("change", (event) => {
  if (event.target.id !== "provider_type") return;
  const form = event.target.closest("form");
  form.querySelector("#display_name").value = event.target.selectedOptions[0].text;
  form.querySelectorAll("[data-provider-config]").forEach((section) => {
    const selected = section.dataset.providerConfig === event.target.value;
    section.hidden = !selected;
    section.querySelectorAll("input").forEach((input) => {
      input.disabled = !selected;
      input.required = selected;
    });
  });
});

window.addEventListener("popstate", () => {
  navigate(window.location.pathname + window.location.search + window.location.hash, false);
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    setProviderMenuOpen(false);
    setSidebarOpen(false);
  }
});

window.addEventListener("resize", () => {
  setProviderMenuOpen(false);
  setSidebarOpen(false);
});

document.addEventListener("submit", (event) => {
  if (event.target.matches("[data-settings-purge]")) {
    window.sessionStorage.setItem("clearquota-settings-return-tab", "privacy");
  }
});

document.addEventListener("DOMContentLoaded", () => {
  setSidebarOpen(false);
  initializeSettingsTabs();
  initializeProviderCardGrids();
  renderLucideIcons();
});
document.addEventListener("htmx:afterSettle", () => {
  initializeProviderCardGrids();
  renderLucideIcons();
});
