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

document.addEventListener("DOMContentLoaded", () => {
  initializeProviderCardGrids();
  renderLucideIcons();
});
document.addEventListener("htmx:afterSettle", () => {
  initializeProviderCardGrids();
  renderLucideIcons();
});
