let pendingNavigation = null;

function renderLucideIcons() {
  window.lucide?.createIcons({ attrs: { "stroke-width": 1.75 } });
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

  const sidebar = document.querySelector("[data-sidebar]");
  const backdrop = document.querySelector("[data-sidebar-backdrop]");
  const toggle = document.querySelector("[data-sidebar-toggle]");
  const isMobile = window.innerWidth < 800;
  sidebar?.classList.toggle("-translate-x-full", isMobile);
  sidebar?.toggleAttribute("inert", isMobile);
  sidebar?.setAttribute("aria-hidden", String(isMobile));
  backdrop?.classList.toggle("hidden", true);
  toggle?.setAttribute("aria-expanded", "false");
  initializeProviderCardGrids();
  renderLucideIcons();

  if (!pendingNavigation) return;

  if (pendingNavigation.pushHistory) {
    window.history.pushState({}, "", pendingNavigation.path);
  }
  pendingNavigation = null;
});

window.addEventListener("popstate", () => {
  navigate(window.location.pathname + window.location.search + window.location.hash, false);
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") setProviderMenuOpen(false);
});

window.addEventListener("resize", () => setProviderMenuOpen(false));

document.addEventListener("DOMContentLoaded", () => {
  initializeProviderCardGrids();
  renderLucideIcons();
});
document.addEventListener("htmx:afterSettle", () => {
  initializeProviderCardGrids();
  renderLucideIcons();
});
