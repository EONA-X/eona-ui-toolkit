// Page glue for the live charts of the components page (the `live-charts` section).
//
// Each `<div data-cube-chart="BarChart" data-src="samples/x.jsonld">` is an empty
// container. When one comes near the viewport this loads the charts WebAssembly
// module (once, for all of them), fetches `data-src`, and mounts the component into
// the container with the text of the document. Nothing is rendered on the server,
// so without JavaScript or WebAssembly the containers stay empty.
//
// The JSON-LD is parsed in Rust, by the module. This script only fetches the text
// and hands it over; a fetch that fails is handed over as `fetchError`, so the chart
// renders every error state itself.
//
// Optional attributes on the container, passed to `mount` as props:
//   data-lang    -> lang      data-select  -> select (CubeScoreDonut)
//   data-help    -> help (CubeMetricTile)
// Buttons `<button data-chart-for="<container id>" data-chart-src="samples/y.jsonld">`
// switch the dataset: a mounted chart is fetched again and updated in place (the
// last click wins), an unmounted one just loads the new file when it becomes visible.
//
// The module is looked up beside this script, at `wasm/eona_ui_toolkit_charts.js`
// (`wasm-pack build --target web`). A host that serves it elsewhere defines
// `window.eonaChartsLoader = () => Promise<{ mount }>` before this script runs.
(function () {
  var scriptUrl = (document.currentScript && document.currentScript.src) || window.location.href;
  var modulePromise = null;
  var charts = new WeakMap(); // container -> { handle, run, controller }

  function load() {
    if (!modulePromise) {
      modulePromise = typeof window.eonaChartsLoader === "function"
        ? Promise.resolve().then(window.eonaChartsLoader)
        : import(new URL("wasm/eona_ui_toolkit_charts.js", scriptUrl).href).then(function (mod) {
            return Promise.resolve(mod.default()).then(function () {
              return mod;
            });
          });
    }
    return modulePromise;
  }

  // The props for one container: the document, or why there is none.
  function fetchProps(el, controller) {
    var src = el.getAttribute("data-src");
    return window
      .fetch(src, { signal: controller.signal })
      .then(function (res) {
        if (!res.ok) throw new Error("HTTP " + res.status + " for " + src);
        return res.text();
      })
      .then(
        function (text) {
          return { jsonld: text };
        },
        function (error) {
          if (controller.signal.aborted) throw error;
          return { fetchError: String((error && error.message) || error) };
        }
      );
  }

  function withOptions(el, props) {
    [["data-lang", "lang"], ["data-select", "select"], ["data-help", "help"]].forEach(function (pair) {
      var value = el.getAttribute(pair[0]);
      if (value !== null) props[pair[1]] = value;
    });
    return props;
  }

  function showError(el, message) {
    el.innerHTML = "";
    var box = document.createElement("div");
    box.className = "chart chart--error";
    box.setAttribute("role", "alert");
    box.textContent = message;
    el.appendChild(box);
  }

  // Fetch the container's data-src and mount (first time) or update (after).
  function render(el) {
    var state = charts.get(el) || { handle: null, run: 0, controller: null };
    charts.set(el, state);
    if (state.controller) state.controller.abort();
    var controller = new AbortController();
    state.controller = controller;
    var run = ++state.run;
    return Promise.all([load(), fetchProps(el, controller)]).then(
      function (done) {
        if (run !== state.run) return;
        var props = withOptions(el, done[1]);
        if (state.handle) {
          state.handle.update(props);
        } else {
          state.handle = done[0].mount(el, el.getAttribute("data-cube-chart"), props);
        }
      },
      function (error) {
        if (run !== state.run || controller.signal.aborted) return;
        showError(el, "The chart could not be shown: " + String((error && error.message) || error));
      }
    );
  }

  var containers = Array.prototype.slice.call(document.querySelectorAll("[data-cube-chart]"));

  if ("IntersectionObserver" in window) {
    var observer = new window.IntersectionObserver(
      function (entries) {
        entries.forEach(function (entry) {
          if (!entry.isIntersecting) return;
          observer.unobserve(entry.target);
          render(entry.target);
        });
      },
      { rootMargin: "200px" }
    );
    containers.forEach(function (el) {
      observer.observe(el);
    });
  } else {
    containers.forEach(render);
  }

  // Dataset switcher.
  document.querySelectorAll(".chart-switch").forEach(function (group) {
    group.hidden = false;
  });
  document.querySelectorAll("[data-chart-for][data-chart-src]").forEach(function (button) {
    button.addEventListener("click", function () {
      var el = document.getElementById(button.getAttribute("data-chart-for"));
      if (!el) return;
      document.querySelectorAll('[data-chart-for="' + el.id + '"]').forEach(function (b) {
        b.setAttribute("aria-pressed", String(b === button));
      });
      el.setAttribute("data-src", button.getAttribute("data-chart-src"));
      // A chart still waiting for the viewport loads the new file when it gets there.
      if (charts.has(el)) render(el);
    });
  });
})();
