// Page glue for the toolkit demo page (DEMO_SHELL loads this as demo-page.js):
// opens/closes NavDropdown and Modal components, and copies CodeBlock snippets
// to the clipboard. AccordionItem needs none of this -- native <details>
// handles itself.
//
// Same division of labor as the design-system page's swatch click-to-copy: the
// component markup is real eona-x.eu-derived HTML rendered by Yew, this is just
// the interaction wiring, because every component in the toolkit is
// presentational and ships no event handlers of its own.
(function () {
  document.querySelectorAll("[data-dropdown-trigger]").forEach(function (trigger) {
    var id = trigger.getAttribute("data-dropdown-trigger");
    var panel = document.getElementById(id);
    if (!panel) return;
    trigger.addEventListener("click", function () {
      var open = trigger.getAttribute("aria-expanded") === "true";
      trigger.setAttribute("aria-expanded", String(!open));
      if (open) {
        panel.setAttribute("inert", "");
      } else {
        panel.removeAttribute("inert");
      }
    });
  });

  document.querySelectorAll(".submenu-close[data-close]").forEach(function (btn) {
    btn.addEventListener("click", function () {
      var id = btn.getAttribute("data-close");
      var panel = document.getElementById(id);
      var trigger = document.querySelector('[data-dropdown-trigger="' + id + '"]');
      if (panel) panel.setAttribute("inert", "");
      if (trigger) trigger.setAttribute("aria-expanded", "false");
    });
  });

  document.querySelectorAll("[data-open-modal]").forEach(function (btn) {
    btn.addEventListener("click", function () {
      var modal = document.getElementById(btn.getAttribute("data-open-modal"));
      if (modal) modal.removeAttribute("inert");
    });
  });

  document.querySelectorAll("[data-close-modal]").forEach(function (btn) {
    btn.addEventListener("click", function () {
      var modal = document.getElementById(btn.getAttribute("data-close-modal"));
      if (modal) modal.setAttribute("inert", "");
    });
  });

  // CodeBlock copy buttons. The snippet is read back out of the rendered
  // <code>, not held in a data attribute, so there is only ever one copy of
  // the text and the two cannot drift.
  document.querySelectorAll(".code-block-copy").forEach(function (btn) {
    btn.addEventListener("click", function () {
      var block = btn.closest(".code-block");
      var code = block && block.querySelector("code");
      if (!code || !navigator.clipboard || !navigator.clipboard.writeText) return;
      navigator.clipboard.writeText(code.textContent).then(function () {
        btn.classList.add("copied");
        btn.textContent = "Copied";
        setTimeout(function () {
          btn.classList.remove("copied");
          btn.textContent = "Copy";
        }, 1200);
      }).catch(function () {});
    });
  });
})();
