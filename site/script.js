// pacpad site: theme toggle, mobile menu, copy buttons, the TUI preview's
// tabs (also on keys 1-4, like the real thing), the splash chomp, card
// spotlight and scroll reveals. No dependencies.
(function () {
  "use strict";
  var root = document.documentElement;
  var reduce = window.matchMedia && matchMedia("(prefers-reduced-motion: reduce)").matches;

  // ---- theme ----
  var toggle = document.querySelector(".theme-toggle");
  if (toggle) {
    toggle.addEventListener("click", function () {
      var next = root.getAttribute("data-theme") === "light" ? "dark" : "light";
      root.setAttribute("data-theme", next);
      try { localStorage.setItem("pacpad-theme", next); } catch (e) {}
    });
  }

  // ---- header ----
  var header = document.querySelector(".site-header");
  var onScroll = function () { header.classList.toggle("stuck", window.scrollY > 8); };
  window.addEventListener("scroll", onScroll, { passive: true });
  onScroll();

  var menu = document.querySelector(".menu-btn");
  if (menu) {
    menu.addEventListener("click", function () {
      var open = header.classList.toggle("open");
      menu.setAttribute("aria-expanded", String(open));
    });
    header.querySelectorAll(".nav a").forEach(function (a) {
      a.addEventListener("click", function () {
        header.classList.remove("open");
        menu.setAttribute("aria-expanded", "false");
      });
    });
  }

  // ---- copy ----
  function copyText(text, btn) {
    var done = function () {
      var label = btn.querySelector("span");
      var prev = label ? label.textContent : "";
      btn.classList.add("done");
      if (label) label.textContent = "copied";
      setTimeout(function () { btn.classList.remove("done"); if (label) label.textContent = prev; }, 1600);
    };
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(done, function () {});
    } else {
      var ta = document.createElement("textarea");
      ta.value = text; ta.style.position = "fixed"; ta.style.opacity = "0";
      document.body.appendChild(ta); ta.select();
      try { document.execCommand("copy"); done(); } catch (e) {}
      document.body.removeChild(ta);
    }
  }
  document.querySelectorAll("[data-copy], [data-copy-target]").forEach(function (btn) {
    btn.addEventListener("click", function () {
      var text = btn.getAttribute("data-copy");
      if (!text) {
        var el = document.getElementById(btn.getAttribute("data-copy-target"));
        text = el ? el.textContent : "";
      }
      copyText(text, btn);
    });
  });

  // ---- TUI preview tabs ----
  var tabs = Array.prototype.slice.call(document.querySelectorAll(".t-tab"));
  function select(i, focus) {
    tabs.forEach(function (t, j) {
      var on = i === j;
      t.setAttribute("aria-selected", String(on));
      t.tabIndex = on ? 0 : -1;
      document.getElementById(t.getAttribute("aria-controls")).hidden = !on;
    });
    if (focus) tabs[i].focus();
    if (tabs[i].dataset.view === "updates") chomp();
  }
  tabs.forEach(function (t, i) {
    t.addEventListener("click", function () { select(i, false); });
    t.addEventListener("keydown", function (e) {
      if (e.key === "ArrowRight") { e.preventDefault(); select((i + 1) % tabs.length, true); }
      if (e.key === "ArrowLeft") { e.preventDefault(); select((i - 1 + tabs.length) % tabs.length, true); }
    });
  });
  document.addEventListener("keydown", function (e) {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    var tag = (e.target && e.target.tagName) || "";
    if (/INPUT|TEXTAREA|SELECT/.test(tag)) return;
    var n = parseInt(e.key, 10);
    if (n >= 1 && n <= tabs.length) select(n - 1, false);
  });

  // ---- splash: the mascot eats a row of dots and reveals the name,
  //      the same animation as src/ui/splash.rs ----
  var splash = document.querySelector("[data-splash]");
  var timer = null;
  function chomp() {
    if (!splash) return;
    var DOTS = 10, WORD = "pacpad", total = DOTS + WORD.length + 1;
    var render = function (pos) {
      var html = "";
      for (var c = 0; c < total - 1; c++) {
        var ch = c < DOTS ? "·" : WORD[c - DOTS];
        if (c < pos) html += c < DOTS ? "&nbsp;" : '<span class="eaten">' + ch + "</span>";
        else if (c === pos) html += '<span class="c-yellow">' + (pos % 2 ? "ᗧ" : "●") + "</span>";
        else html += c < DOTS ? '<span class="c-faint">·</span>' : "&nbsp;";
      }
      if (pos >= total - 1) html += '<span class="c-yellow">ᗧ</span>';
      splash.innerHTML = html;
    };
    clearInterval(timer);
    if (reduce) { render(total - 1); return; }
    var pos = 0;
    render(pos);
    timer = setInterval(function () {
      pos++;
      render(pos);
      if (pos >= total - 1) clearInterval(timer);
    }, 110);
  }

  // ---- card spotlight ----
  document.querySelectorAll(".card").forEach(function (card) {
    card.addEventListener("pointermove", function (e) {
      var r = card.getBoundingClientRect();
      card.style.setProperty("--mx", e.clientX - r.left + "px");
      card.style.setProperty("--my", e.clientY - r.top + "px");
    });
  });

  // ---- reveal on scroll ----
  var items = document.querySelectorAll("[data-reveal]");
  if (reduce || !("IntersectionObserver" in window)) {
    items.forEach(function (el) { el.classList.add("in"); });
  } else {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (en) {
        if (en.isIntersecting) { en.target.classList.add("in"); io.unobserve(en.target); }
      });
    }, { rootMargin: "0px 0px -8% 0px", threshold: 0.08 });
    items.forEach(function (el) { io.observe(el); });
  }
})();
