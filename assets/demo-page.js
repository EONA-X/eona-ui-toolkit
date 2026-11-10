// Page glue for the toolkit demo page (DEMO_SHELL loads this as demo-page.js):
// opens/closes NavDropdown and Modal components, switches the CodeTabs stack,
// and copies CodeBlock and CodeTabs snippets to the clipboard. AccordionItem needs none of this -- native <details>
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

  // CodeTabs: YEW | SVELTE | REACT over one code block.
  //
  // The markup ships with the tablist `hidden` (a block without this script is a
  // plain Yew snippet), so unhiding it is what turns the tabs on. WAI-ARIA tabs
  // pattern with automatic activation: roving tabindex, arrows, Home and End.
  // The selection is one for the whole page -- picking SVELTE in one block shows
  // the Svelte version in every block -- and is remembered across visits.
  var STACKS = ["yew", "svelte", "react"];
  var STACK_KEY = "eona-ui-toolkit:stack";

  function storedStack() {
    try {
      var value = window.localStorage.getItem(STACK_KEY);
      return STACKS.indexOf(value) >= 0 ? value : "yew";
    } catch (e) {
      return "yew";
    }
  }

  function rememberStack(stack) {
    try {
      window.localStorage.setItem(STACK_KEY, stack);
    } catch (e) {}
  }

  function selectStack(stack) {
    document.querySelectorAll(".code-tabs").forEach(function (block) {
      block.querySelectorAll('[role="tab"]').forEach(function (tab) {
        var on = tab.getAttribute("data-stack") === stack;
        tab.setAttribute("aria-selected", String(on));
        tab.setAttribute("tabindex", on ? "0" : "-1");
      });
      var hasCode = false;
      block.querySelectorAll('[role="tabpanel"]').forEach(function (panel) {
        var on = panel.getAttribute("data-stack") === stack;
        panel.hidden = !on;
        if (on) hasCode = !!panel.querySelector("code");
      });
      // An Unavailable panel has nothing to copy.
      var copy = block.querySelector(".code-block-copy");
      if (!copy) return;
      if (hasCode) {
        copy.removeAttribute("aria-disabled");
      } else {
        copy.setAttribute("aria-disabled", "true");
      }
    });
  }

  document.querySelectorAll('.code-tabs [role="tablist"]').forEach(function (list) {
    list.hidden = false;
    var tabs = Array.prototype.slice.call(list.querySelectorAll('[role="tab"]'));
    tabs.forEach(function (tab, i) {
      tab.addEventListener("click", function () {
        var stack = tab.getAttribute("data-stack");
        selectStack(stack);
        rememberStack(stack);
      });
      tab.addEventListener("keydown", function (event) {
        var next;
        if (event.key === "ArrowRight") next = (i + 1) % tabs.length;
        else if (event.key === "ArrowLeft") next = (i + tabs.length - 1) % tabs.length;
        else if (event.key === "Home") next = 0;
        else if (event.key === "End") next = tabs.length - 1;
        else return;
        event.preventDefault();
        var stack = tabs[next].getAttribute("data-stack");
        selectStack(stack);
        rememberStack(stack);
        tabs[next].focus();
      });
    });
  });
  selectStack(storedStack());

  // CodeBlock and CodeTabs copy buttons. The snippet is read back out of the
  // rendered <code> (of the active panel, for CodeTabs), not held in a data
  // attribute, so there is only ever one copy of the text and the two cannot
  // drift.
  document.querySelectorAll(".code-block-copy").forEach(function (btn) {
    btn.addEventListener("click", function () {
      if (btn.getAttribute("aria-disabled") === "true") return;
      var block = btn.closest(".code-block");
      var code = block && (block.querySelector('[role="tabpanel"]:not([hidden]) code') || block.querySelector("code"));
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
