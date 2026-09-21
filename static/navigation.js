let pendingNavigation = null;

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

  if (!pendingNavigation) return;

  if (pendingNavigation.pushHistory) {
    window.history.pushState({}, "", pendingNavigation.path);
  }
  pendingNavigation = null;
});

window.addEventListener("popstate", () => {
  navigate(window.location.pathname + window.location.search + window.location.hash, false);
});
