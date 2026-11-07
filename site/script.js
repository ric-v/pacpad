// pacpad site: theme switch, copy buttons, the screen-capture switcher and
// the contents list's current-section highlight. No dependencies.
(function () {
  "use strict";
  var root = document.documentElement;

  var tb = document.querySelector(".tbtn");
  if (tb) tb.addEventListener("click", function () {
    var next = root.getAttribute("data-theme") === "light" ? "dark" : "light";
    root.setAttribute("data-theme", next);
    try { localStorage.setItem("pacpad-theme", next); } catch (e) {}
  });

  document.querySelectorAll(".copy").forEach(function (btn) {
    btn.addEventListener("click", function () {
      var el = document.getElementById(btn.getAttribute("data-copy"));
      if (!el) return;
      // copy commands without the "$ " prompts or trailing comments
      var text = el.textContent.split("\n").map(function (l) {
        return l.replace(/^\$ /, "").replace(/\s+#.*$/, "");
      }).join("\n").trim();
      var done = function () {
        btn.textContent = "copied"; btn.classList.add("ok");
        setTimeout(function () { btn.textContent = "copy"; btn.classList.remove("ok"); }, 1500);
      };
      if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(text).then(done, function () {});
    });
  });

  var tabs = Array.prototype.slice.call(document.querySelectorAll(".views [role=tab]"));
  function show(i, focus) {
    tabs.forEach(function (t, j) {
      var on = i === j;
      t.setAttribute("aria-selected", String(on));
      t.tabIndex = on ? 0 : -1;
      document.getElementById(t.getAttribute("aria-controls")).hidden = !on;
    });
    if (focus) tabs[i].focus();
  }
  tabs.forEach(function (t, i) {
    t.addEventListener("click", function () { show(i, false); });
    t.addEventListener("keydown", function (e) {
      if (e.key === "ArrowRight") { e.preventDefault(); show((i + 1) % tabs.length, true); }
      if (e.key === "ArrowLeft") { e.preventDefault(); show((i + tabs.length - 1) % tabs.length, true); }
    });
  });

  var links = Array.prototype.slice.call(document.querySelectorAll(".toc a"));
  if ("IntersectionObserver" in window && links.length) {
    var byId = {};
    links.forEach(function (a) { byId[a.getAttribute("href").slice(1)] = a; });
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (en) {
        if (!en.isIntersecting) return;
        links.forEach(function (a) { a.classList.remove("on"); });
        var a = byId[en.target.id];
        if (a) a.classList.add("on");
      });
    }, { rootMargin: "-20% 0px -70% 0px" });
    document.querySelectorAll("main section[id]").forEach(function (s) { io.observe(s); });
  }
})();
