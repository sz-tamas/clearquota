(() => {
  const reducedMotion = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  function animateNumberText(element) {
    if (element.dataset.motionNumberAnimated) return;
    element.dataset.motionNumberAnimated = "true";
    if (reducedMotion()) return;

    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
    const textNodes = [];
    while (walker.nextNode()) textNodes.push(walker.currentNode);

    textNodes.forEach((textNode) => {
      const text = textNode.textContent;
      const matches = [...text.matchAll(/\d[\d,]*(?:\.\d+)?/g)];
      if (!matches.length) return;

      const fragment = document.createDocumentFragment();
      let offset = 0;
      matches.forEach((match) => {
        fragment.append(text.slice(offset, match.index));
        const finalText = match[0];
        const value = Number(finalText.replaceAll(",", ""));
        const decimalPlaces = finalText.includes(".") ? finalText.split(".").at(-1).length : 0;
        const counter = document.createElement("span");
        counter.className = "tabular-nums";
        counter.textContent = value.toLocaleString("en-US", {
          minimumFractionDigits: decimalPlaces,
          maximumFractionDigits: decimalPlaces,
        });
        fragment.append(counter);
        offset = match.index + finalText.length;

        Motion.animate(0, value, {
          duration: 1.15,
          ease: "circOut",
          onUpdate: (latest) => {
            counter.textContent = latest.toLocaleString("en-US", {
              minimumFractionDigits: decimalPlaces,
              maximumFractionDigits: decimalPlaces,
            });
          },
        });
      });
      fragment.append(text.slice(offset));
      textNode.replaceWith(fragment);
    });
  }

  function animateProgressBar(bar) {
    if (bar.dataset.motionChartAnimated) return;
    bar.dataset.motionChartAnimated = "true";
    if (reducedMotion()) return;
    const targetWidth = bar.style.width;
    Motion.animate(bar, { width: ["0%", targetWidth] }, { duration: 0.9, ease: "circOut" });
  }

  function animateSparkline(line) {
    if (line.dataset.motionChartAnimated) return;
    line.dataset.motionChartAnimated = "true";
    if (reducedMotion()) return;
    line.style.strokeDasharray = "1";
    line.style.strokeDashoffset = "1";
    line.style.opacity = "0";
    line.setAttribute("pathLength", "1");
    Motion.animate(
      line,
      { strokeDashoffset: [1, 0], opacity: [0, 1] },
      { duration: 1.05, ease: "easeInOut" },
    );
  }

  function initializeDashboardMotion(root = document) {
    if (document.querySelector("#auth-validation, #auth-dialog")) return;
    root.querySelectorAll("[data-motion-number]").forEach(animateNumberText);
    root.querySelectorAll("[data-provider-card] [style*='width:'], [data-motion-progress]").forEach(animateProgressBar);
    root.querySelectorAll("[data-provider-card] svg polyline").forEach(animateSparkline);
  }

  window.initializeDashboardMotion = initializeDashboardMotion;
  document.addEventListener("DOMContentLoaded", () => initializeDashboardMotion());
  document.addEventListener("htmx:afterSettle", () => initializeDashboardMotion());
})();
