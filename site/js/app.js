// The site shell: tab bar + scrollspy, pane focus, the global keyboard layer,
// command palette, help, demo focus model, shell wiring, the How-it-works
// flow, the REMOVE guard, the keys board, FAQ, reveals and the boot screen.
(function () {
  "use strict";

  var $ = function (s, r) { return (r || document).querySelector(s); };
  var $$ = function (s, r) { return Array.prototype.slice.call((r || document).querySelectorAll(s)); };
  var reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
  var smooth = reduced ? "auto" : "smooth";
  var store = {
    get: function (k) { try { return sessionStorage.getItem(k); } catch (e) { return null; } },
    set: function (k, v) { try { sessionStorage.setItem(k, v); } catch (e) {} }
  };

  // ------------------------------------------------------------- pane titles
  var panes = $$(".pane");
  panes.forEach(function (p) {
    if (p.dataset.title) {
      var h = document.createElement(p.closest(".hero") && !p.classList.contains("demo-pane") ? "div" : "h2");
      h.className = "pane-ttl"; h.textContent = " " + p.dataset.title + " ";
      h.dataset.text = h.textContent;
      p.insertBefore(h, p.firstChild);
    }
    if (p.dataset.titleR) {
      var r = document.createElement("span");
      r.className = "pane-ttl-r"; r.textContent = " " + p.dataset.titleR + " ";
      r.setAttribute("aria-hidden", "true");
      p.insertBefore(r, p.firstChild.nextSibling);
    }
  });

  // title scramble, run once as a pane comes into view
  var GLYPH = "▖▗▘▙▚▛▜▝▞▟░▒─│┼╳#$%&*+=<>/\\";
  function scramble(el) {
    var final = el.dataset.text || el.textContent;
    if (reduced) { el.textContent = final; return; }
    var start = performance.now(), dur = 520;
    (function frame(t) {
      var k = Math.min(1, (t - start) / dur), n = Math.floor(final.length * k), s = final.slice(0, n);
      for (var i = n; i < final.length; i++) s += final[i] === " " ? " " : GLYPH[(Math.random() * GLYPH.length) | 0];
      el.textContent = s;
      if (k < 1) requestAnimationFrame(frame); else el.textContent = final;
    })(start);
  }

  // ------------------------------------------------------------- reveals
  panes.forEach(function (p) { $$(".rv", p).forEach(function (el, i) { el.style.setProperty("--i", Math.min(i, 8)); }); });
  var revealIO = new IntersectionObserver(function (ents) {
    ents.forEach(function (e) {
      if (!e.isIntersecting) return;
      var el = e.target;
      el.classList.add("in");
      if (el.classList.contains("pane")) { var t = $(".pane-ttl", el); if (t) scramble(t); }
      revealIO.unobserve(el);
    });
  }, { rootMargin: "0px 0px -8% 0px", threshold: 0.06 });
  $$(".rv, .pane").forEach(function (el) { revealIO.observe(el); });

  // ------------------------------------------------------------- toast + copy
  var toastEl = $("#toast"), toastT = 0;
  function toast(text, err) {
    toastEl.textContent = text;
    toastEl.classList.toggle("err", !!err);
    toastEl.classList.add("show");
    clearTimeout(toastT);
    toastT = setTimeout(function () { toastEl.classList.remove("show"); }, 2200);
  }
  function copy(text, btn) {
    var done = function () {
      toast("ᗧ copied: " + text);
      if (btn && btn.classList.contains("cp")) {
        var i = $(".cpi", btn); btn.classList.add("ok"); i.textContent = "copied";
        setTimeout(function () { btn.classList.remove("ok"); i.textContent = "copy"; }, 1600);
      }
    };
    if (navigator.clipboard && window.isSecureContext) navigator.clipboard.writeText(text).then(done, fallback);
    else fallback();
    function fallback() {
      var ta = document.createElement("textarea");
      ta.value = text; ta.style.position = "fixed"; ta.style.opacity = "0";
      document.body.appendChild(ta); ta.select();
      try { document.execCommand("copy"); done(); } catch (e) { toast("couldn't copy. select it by hand", true); }
      ta.remove();
    }
  }
  var BUILD = "git clone https://github.com/ric-v/pacpad && cd pacpad && cargo build --release";

  // ------------------------------------------------------------- tabs + scrollspy
  var tabs = $("#tabs"), hl = $(".tab-hl", tabs), tabLinks = $$("a", tabs);
  var secs = $$(".sec");
  var posName = $("#pos-name"), posPct = $("#pos-pct");
  var activeSec = null;
  function padTop() { return parseFloat(getComputedStyle(document.documentElement).scrollPaddingTop) || 80; }
  function moveHL(a, instant) {
    if (!a) return;
    if (instant) hl.style.transition = "none";
    hl.style.left = a.offsetLeft + "px";
    hl.style.width = a.offsetWidth + "px";
    if (instant) { hl.offsetWidth; hl.style.transition = ""; }
    var tl = tabs.scrollLeft, w = tabs.clientWidth;
    if (a.offsetLeft < tl || a.offsetLeft + a.offsetWidth > tl + w) tabs.scrollTo({ left: a.offsetLeft - 20, behavior: smooth });
  }
  function spy() {
    var line = padTop() + innerHeight * 0.25, cur = secs[0];
    secs.forEach(function (s) { if (s.getBoundingClientRect().top <= line) cur = s; });
    if (innerHeight + scrollY >= document.documentElement.scrollHeight - 4) cur = secs[secs.length - 1];
    if (cur !== activeSec) {
      activeSec = cur;
      tabLinks.forEach(function (a) { var on = a.dataset.sec === cur.id; a.classList.toggle("on", on); if (on) { a.setAttribute("aria-current", "true"); moveHL(a); } else a.removeAttribute("aria-current"); });
      posName.textContent = cur.dataset.name;
    }
    var max = document.documentElement.scrollHeight - innerHeight;
    posPct.textContent = scrollY <= 2 ? "top" : scrollY >= max - 2 ? "bot" : Math.round((scrollY / max) * 100) + "%";
  }
  var spyQ = false;
  addEventListener("scroll", function () { if (!spyQ) { spyQ = true; requestAnimationFrame(function () { spyQ = false; spy(); }); } }, { passive: true });
  addEventListener("resize", function () { moveHL($("a.on", tabs), true); spy(); });
  document.fonts && document.fonts.ready.then(function () { moveHL($("a.on", tabs), true); });

  function goSec(id) {
    var el = document.getElementById(id);
    if (!el) return;
    setFocus(null);
    if (id === "top") scrollTo({ top: 0, behavior: smooth });
    else el.scrollIntoView({ behavior: smooth, block: "start" });
    history.replaceState(null, "", id === "top" ? location.pathname : "#" + id);
  }
  tabLinks.forEach(function (a) { a.addEventListener("click", function (e) { e.preventDefault(); goSec(a.dataset.sec); }); });
  $$('a[href^="#"]').forEach(function (a) {
    if (a.closest("#tabs") || a.dataset.act) return;
    a.addEventListener("click", function (e) { var id = a.getAttribute("href").slice(1); if (document.getElementById(id)) { e.preventDefault(); goSec(id); } });
  });

  // ------------------------------------------------------------- pane focus (j / k)
  var focusPane = null;
  var navPanes = panes;
  function setFocus(p, scroll) {
    if (focusPane) focusPane.classList.remove("focus");
    focusPane = p;
    if (!p) { updateHints(); return; }
    p.classList.add("focus");
    if (scroll) {
      var top = p.getBoundingClientRect().top + scrollY - padTop();
      scrollTo({ top: Math.max(0, top), behavior: smooth });
    }
    updateHints();
  }
  function stepPane(dir) {
    var tops = navPanes.map(function (p) { return p.getBoundingClientRect().top - padTop(); });
    var cur = focusPane ? navPanes.indexOf(focusPane) : -1, i;
    if (cur >= 0 && Math.abs(tops[cur]) < 40) i = cur + dir;
    else if (dir > 0) { i = tops.findIndex(function (t) { return t > 10; }); if (i < 0) i = navPanes.length - 1; }
    else { i = -1; tops.forEach(function (t, k) { if (t < -10) i = k; }); if (i < 0) i = 0; }
    i = Math.max(0, Math.min(navPanes.length - 1, i));
    setFocus(navPanes[i], true);
  }
  function activatePane(p) {
    if (!p) return;
    if (p.id === "demo") takeControl();
    else if (p.closest("#install")) focusShell();
    else if (p.closest("#how")) { var n = $(".node.on", p) || $(".node", p); n.focus(); }
    else if (p.closest("#safety")) $("#guard-in").focus();
    else if (p.closest("#faq")) $(".q", p).focus();
  }

  // ------------------------------------------------------------- status line
  var modeEl = $("#mode"), hintsEl = $("#hints");
  var mode = "normal";
  var HINTS = {
    normal: [["1-7", "sections"], ["j/k", "panels"], ["/", "search"], ["d", "drive demo"], ["i", "shell"], ["y", "copy build"], ["?", "keys"]],
    demo: [["keys", "go to pacpad"], ["esc", "let go"], ["F1", "pacpad help"], ["q", "quit pacpad"]],
    shell: [["enter", "run"], ["↑↓", "history"], ["tab", "complete"], ["esc", "back to page"]],
    search: [["↑↓", "nav"], ["enter", "run"], ["esc", "close"]]
  };
  function setMode(m) {
    mode = m;
    modeEl.textContent = { normal: "NORMAL", demo: "DEMO", shell: "SHELL", search: "SEARCH" }[m];
    modeEl.className = "mode m-" + m;
    updateHints();
  }
  function updateHints() {
    var h = HINTS[mode].slice();
    if (mode === "normal" && focusPane) h.unshift(["enter", "open panel"]);
    hintsEl.innerHTML = h.map(function (x) { return "<b>" + x[0] + "</b> " + x[1]; }).join('<span class="sp">·</span>');
  }

  // ------------------------------------------------------------- demo
  var demoEl = $("#demo-el"), demoPane = $("#demo"), driving = false;
  var autoChip = $('[data-act="autoplay"]'), demoMode = $("#demo-mode"), keyviz = $("#keyviz");
  var PRETTY = { up: "↑", down: "↓", left: "←", right: "→", enter: "enter", esc: "esc", space: "space", tab: "tab", backtab: "⇧tab", backspace: "⌫", F1: "F1" };
  var demo = window.PPDemo.mount(demoEl, {
    onKey: function (k) {
      lightKey(k.key);
      var kb = document.createElement("kbd");
      kb.className = k.auto ? "auto" : "you";
      kb.textContent = PRETTY[k.key] || k.key;
      keyviz.appendChild(kb);
      while (keyviz.children.length > 9) keyviz.firstChild.remove();
      setTimeout(function () { kb.classList.add("out"); setTimeout(function () { kb.remove(); }, 600); }, 1700);
    },
    onAutoplay: function (on) { autoChip.setAttribute("aria-pressed", on ? "true" : "false"); syncDemoMode(); }
  });
  function syncDemoMode() {
    demoMode.textContent = driving ? "you're driving · esc lets go" : demo.auto ? "autoplay · press d to drive" : "paused · press d to drive";
    demoMode.classList.toggle("on", driving);
  }
  function takeControl(scroll) {
    if (scroll !== false) {
      var r = demoEl.getBoundingClientRect();
      if (r.top < padTop() || r.bottom > innerHeight - 40) demoPane.scrollIntoView({ behavior: smooth, block: innerHeight > r.height + 160 ? "center" : "start" });
    }
    demo.setAutoplay(false);
    if (driving) return;
    driving = true;
    demo.focused = true;
    demoEl.classList.add("driving"); demoPane.classList.add("driving");
    demoEl.focus({ preventScroll: true });
    setMode("demo"); syncDemoMode(); demo.draw();
  }
  function release() {
    if (!driving) return;
    driving = false;
    demo.focused = false;
    demoEl.classList.remove("driving"); demoPane.classList.remove("driving");
    if (document.activeElement === demoEl) demoEl.blur();
    setMode("normal"); syncDemoMode(); demo.draw();
  }
  demoEl.addEventListener("click", function () { if (!driving) takeControl(false); });
  demoEl.addEventListener("keydown", function (e) {
    if (!driving && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); e.stopPropagation(); takeControl(false); }
  });
  document.addEventListener("pointerdown", function (e) { if (driving && !demoPane.contains(e.target)) release(); });
  demoEl.addEventListener("focusout", function () { setTimeout(function () { if (driving && !demoPane.contains(document.activeElement)) release(); }, 0); });
  function demoKey(e) {
    var k = e.key, map = { ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right", Enter: "enter", Escape: "esc", Backspace: "backspace", " ": "space", Tab: e.shiftKey ? "backtab" : "tab", F1: "F1" };
    if (map[k]) return map[k];
    if (k.length === 1) return k;
    return null;
  }
  $$("#touchkeys button").forEach(function (b) {
    b.addEventListener("click", function () { if (!driving) takeControl(false); demo.key(b.dataset.k); demoEl.focus({ preventScroll: true }); });
  });
  var demoVis = false;
  new IntersectionObserver(function (e) { demoVis = e[0].isIntersecting; demo.paused = !demoVis || document.hidden; if (!demoVis) release(); }, { threshold: 0.25 }).observe(demoEl);
  document.addEventListener("visibilitychange", function () { demo.paused = !demoVis || document.hidden; });
  demo.setAutoplay(true);
  syncDemoMode();

  // ------------------------------------------------------------- shell
  var shellEl = $("#shell"), shellWrap = shellEl.parentNode, wantShellFocus = false;
  var shell = window.PPShell.mount(shellEl, {
    onLaunch: function () { setTimeout(function () { takeControl(); }, 250); },
    onReady: function () { if (wantShellFocus) { wantShellFocus = false; shell.focus(); } },
    onBlur: function () { shellEl.blur(); }
  });
  var shellIn = $(".sh-in", shellEl);
  shellIn.addEventListener("focus", function () { shellWrap.classList.add("focus"); setMode("shell"); });
  shellIn.addEventListener("blur", function () { shellWrap.classList.remove("focus"); if (mode === "shell") setMode("normal"); });
  new IntersectionObserver(function (e, o) { if (e[0].isIntersecting) { shell.start(); o.disconnect(); } }, { threshold: 0.35 }).observe(shellEl);
  function focusShell() {
    var r = shellEl.getBoundingClientRect();
    if (r.top < padTop() || r.bottom > innerHeight) shellWrap.scrollIntoView({ behavior: smooth, block: "center" });
    if (shell.state === "idle") shell.start();
    if (shell.state === "intro") { wantShellFocus = true; shell.skip(); }
    else shell.focus();
  }
  window.ppRunInShell = function (cmd) { focusShell(); var go = function () { if (shell.state === "ready") shell.runTyped(cmd); else setTimeout(go, 120); }; setTimeout(go, reduced ? 0 : 450); };

  // ------------------------------------------------------------- how it works
  var NODES = {
    conf: ["pacman.conf", "read · 1 of 4", "pacpad starts where pacman does. It reads DBPath, RootDir and every [repo] section from /etc/pacman.conf, expanding Include lines inline the way pacman does, so third-party repos are searched too.", "pacpad --pacman-conf ./fixture/pacman.conf   # point it at a fixture, no root needed"],
    dbs: ["local + sync databases", "read · 2 of 4", "Installed packages come from <DBPath>/local/*/desc and repos from <DBPath>/sync/<repo>.db. pacpad parses them itself instead of linking libalpm, and sniffs each .db's magic bytes because repos ship both gzip and zstd.", "/var/lib/pacman/local/ripgrep-14.1.1-1/desc\n/var/lib/pacman/sync/extra.db   # 1f 8b → gzip · 28 b5 2f fd → zstd"],
    cache: ["the merged index", "read · 3 of 4", "Local and sync entries merge into one index, so every package knows if it's installed, where it came from, and whether it's explicit, a dependency, an orphan or foreign. It's cached with a fingerprint of each source file's path, mtime and size, so an unchanged system starts warm.", "~/.cache/pacpad/index.bin"],
    tui: ["the TUI", "read · 4 of 4", "ratatui on crossterm. Four tabs, five filters, three sorts, fuzzy search and a detail pane. Below 60×12 it asks for a bigger window instead of drawing a broken layout.", "1 Installed · 2 Search · 3 Apps · 4 Updates"],
    action: ["an action", "write · 1 of 4", "Install, remove, reinstall, mark explicit, mark as dependency, update, list files. Each one applies to your selection, or to the row under the cursor when nothing is selected.", "i  d  r  e  D  u  f"],
    plan: ["the plan", "write · 2 of 4", "Every action becomes a literal argv. Repo packages go to sudo pacman. If any target is foreign, the plan goes to your AUR helper instead: the configured one if it's installed, then yay, then paru.", "sudo pacman -S zellij\nyay -S spotify\nsudo pacman -Rns go\nsudo pacman -D --asdeps python-pip"],
    confirm: ["the confirm modal", "write · 3 of 4", "The exact command is shown before anything runs. Removal defaults to -Rns and r toggles -R. Removing pacman, sudo, systemd, glibc, linux* or anything in base needs REMOVE typed out.", "type REMOVE to confirm: █"],
    pty: ["the embedded terminal", "write · 4 of 4", "The command runs on a pseudo-terminal pacpad owns (portable-pty + vt100), drawn inside pacpad's own screen. sudo's password, pacman's [Y/n] and yay's PKGBUILD review all work, and you never leave the app.", "ᗧ Running: sudo pacman -S zellij\nyour keystrokes go to the running command -- Ctrl+C interrupts it, not pacpad"],
    form: ["name + url, or name + command", "apps · 1 of 3", "A webapp is a name and a URL. The icon is a URL, a path, or blank to fetch the site's favicon. A TUI app is a name, a command and a float or tile window style.", "pacpad webapp add Linear https://linear.app\npacpad tui add btop btop --float"],
    resolve: ["browser or terminal", "apps · 2 of 3", "Webapps need Chromium's --app mode: your configured browser, else your default if it's Chromium-based, else Chrome, Chromium, Brave or Vivaldi. TUI apps use your configured terminal, then xdg-terminal-exec, $TERMINAL, Ghostty, kitty, WezTerm, Alacritty or foot.", "chromium --app=https://linear.app --class=pacpad-linear"],
    desktop: ["a .desktop file pacpad owns", "apps · 3 of 3", "Launchers land in ~/.local/share/applications as pacpad-<name>.desktop with an X-PacPad-Managed=true marker. Only files carrying that marker are listed, edited or removed, so launchers you made some other way are never touched.", "~/.local/share/applications/pacpad-linear.desktop\nX-PacPad-Managed=true"]
  };
  var flow = $("#flow"), det = $("#flow-det"), nodes = $$(".node", flow);
  function pickNode(n, user) {
    nodes.forEach(function (x) { x.classList.toggle("on", x === n); x.setAttribute("aria-pressed", x === n ? "true" : "false"); });
    var d = NODES[n.dataset.n];
    $(".fd-t", det).textContent = d[0];
    $(".fd-p", det).textContent = d[1];
    $(".fd-b", det).textContent = d[2];
    $(".fd-c", det).textContent = d[3] || "";
    det.classList.remove("swap"); det.offsetWidth; det.classList.add("swap");
  }
  nodes.forEach(function (n) {
    n.addEventListener("click", function () { pickNode(n, true); });
    n.addEventListener("keydown", function (e) {
      var i = nodes.indexOf(n), lane = n.closest(".lane"), ln = $$(".node", lane), li = ln.indexOf(n), lanes = $$(".lane", flow), lx = lanes.indexOf(lane), t = null;
      if (e.key === "ArrowRight" || e.key === "l") t = nodes[Math.min(nodes.length - 1, i + 1)];
      else if (e.key === "ArrowLeft" || e.key === "h") t = nodes[Math.max(0, i - 1)];
      else if (e.key === "ArrowDown" && lanes[lx + 1]) { var nl = $$(".node", lanes[lx + 1]); t = nl[Math.min(li, nl.length - 1)]; }
      else if (e.key === "ArrowUp" && lanes[lx - 1]) { var pl = $$(".node", lanes[lx - 1]); t = pl[Math.min(li, pl.length - 1)]; }
      if (t) { e.preventDefault(); e.stopPropagation(); t.focus(); pickNode(t, true); t.scrollIntoView({ block: "nearest", inline: "nearest", behavior: smooth }); }
    });
  });
  pickNode(nodes[0]);

  // packets travel each lane, lighting the nodes they reach
  var flowVis = false;
  new IntersectionObserver(function (e) { flowVis = e[0].isIntersecting; }, { threshold: 0.2 }).observe(flow);
  function runLane(lane, delay) {
    var pkt = $(".pkt", lane), ln = $$(".node", lane);
    var hop = function (i) {
      if (!flowVis || reduced || document.hidden) { setTimeout(function () { hop(0); }, 900); return; }
      var x = function (n) { return n.offsetLeft + n.offsetWidth / 2; };
      ln[i].classList.add("lit");
      setTimeout(function () { ln[i].classList.remove("lit"); }, 900);
      if (i === ln.length - 1) { pkt.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 400, fill: "forwards" }); setTimeout(function () { hop(0); }, 2600 + Math.random() * 1400); return; }
      var a = pkt.animate([{ left: x(ln[i]) + "px", opacity: 1 }, { left: x(ln[i + 1]) + "px", opacity: 1 }], { duration: 650, easing: "cubic-bezier(.5,0,.3,1)", fill: "forwards" });
      a.onfinish = function () { setTimeout(function () { hop(i + 1); }, 260); };
    };
    setTimeout(function () { hop(0); }, delay);
  }
  $$(".lane", flow).forEach(function (l, i) { runLane(l, 400 + i * 900); });

  // ------------------------------------------------------------- the REMOVE guard
  var gIn = $("#guard-in"), gOk = $("#guard-ok"), gm = $(".gm"), gMsg = $("#guard-msg");
  var gMsgs = ["nice. on a real system, that's how much it takes to remove glibc.", "confirmed. this is where pacman would ask for your password.", "you'd want a live USB nearby for this one."], gCount = 0;
  gIn.addEventListener("input", function () {
    var v = gIn.value, ok = v === "REMOVE";
    gOk.disabled = !ok; gm.classList.toggle("armed", ok);
    gMsg.className = "gm-msg " + (ok ? "c-g" : "c-f");
    gMsg.textContent = ok ? "enter is armed." : v && "REMOVE".indexOf(v) !== 0 ? "it has to be REMOVE, in capitals." : "try it. this one's safe: it's a website.";
  });
  gIn.addEventListener("keydown", function (e) {
    e.stopPropagation();
    if (e.key === "Enter") { e.preventDefault(); if (gIn.value === "REMOVE") guardOk(); else { gm.classList.remove("shake"); gm.offsetWidth; gm.classList.add("shake"); } }
    if (e.key === "Escape") guardCancel();
  });
  function guardOk() { gMsg.className = "gm-msg c-y"; gMsg.textContent = gMsgs[gCount++ % gMsgs.length]; gIn.value = ""; gOk.disabled = true; gm.classList.remove("armed"); }
  function guardCancel() { gIn.value = ""; gOk.disabled = true; gm.classList.remove("armed"); gMsg.className = "gm-msg c-f"; gMsg.textContent = "cancelled. nothing ran, which is the point."; gIn.blur(); }

  // ------------------------------------------------------------- keys board
  var keysGrid = $("#keys-grid");
  function lightKey(k) {
    $$('kbd[data-k="' + (k === '"' ? '\\"' : k) + '"]', keysGrid).forEach(function (el) {
      el.classList.remove("hit"); el.offsetWidth; el.classList.add("hit");
      clearTimeout(el._t); el._t = setTimeout(function () { el.classList.remove("hit"); }, 450);
    });
  }

  // ------------------------------------------------------------- faq
  var qas = $$(".qa");
  qas.forEach(function (qa) {
    var q = $(".q", qa);
    q.addEventListener("click", function () { toggleQA(qa); });
    q.addEventListener("keydown", function (e) {
      var i = qas.indexOf(qa), t = null;
      if (e.key === "ArrowDown") t = qas[i + 1]; else if (e.key === "ArrowUp") t = qas[i - 1];
      if (t) { e.preventDefault(); e.stopPropagation(); $(".q", t).focus(); }
    });
  });
  function toggleQA(qa, force) {
    var open = force !== undefined ? force : !qa.classList.contains("open");
    qa.classList.toggle("open", open);
    $(".q", qa).setAttribute("aria-expanded", open ? "true" : "false");
  }

  // ------------------------------------------------------------- overlays
  var palOv = $("#palette"), helpOv = $("#help"), palQ = $("#pal-q"), palList = $("#pal-list"), palN = $("#pal-n");
  var lastFocus = null;
  function openOv(ov) {
    lastFocus = document.activeElement;
    if (driving) release();
    ov.hidden = false; ov.classList.remove("show"); ov.offsetWidth; ov.classList.add("show");
  }
  function closeOv(ov) {
    ov.hidden = true;
    if (mode === "search") setMode("normal");
    if (lastFocus && lastFocus.focus && lastFocus !== document.body) lastFocus.focus({ preventScroll: true });
  }
  [palOv, helpOv].forEach(function (ov) { ov.addEventListener("pointerdown", function (e) { if (e.target === ov) closeOv(ov); }); });

  function openHelp() { if (!helpOv.hidden) return closeOv(helpOv); openOv(helpOv); $(".helpbox", helpOv).setAttribute("tabindex", "-1"); $(".helpbox", helpOv).focus(); }

  var CMDS = [
    ["1", "go to pacpad", "section", function () { goSec("top"); }],
    ["2", "go to why", "section", function () { goSec("why"); }],
    ["3", "go to how it works", "section", function () { goSec("how"); }],
    ["4", "go to safety", "section", function () { goSec("safety"); }],
    ["5", "go to install", "section", function () { goSec("install"); }],
    ["6", "go to keys", "section", function () { goSec("keys"); }],
    ["7", "go to faq", "section", function () { goSec("faq"); }],
    ["d", "drive the demo", "demo", function () { takeControl(); }],
    ["a", "toggle demo autoplay", "demo", function () { toggleAuto(); }],
    ["", "reset the demo", "demo", function () { resetDemo(); }],
    ["i", "open the shell", "shell", function () { focusShell(); }],
    ["", "run: pacpad --help", "shell", function () { window.ppRunInShell("pacpad --help"); }],
    ["", "run: pacpad search editor", "shell", function () { window.ppRunInShell("pacpad search editor"); }],
    ["", "run: pacpad webapp add Notion https://notion.so", "shell", function () { window.ppRunInShell("pacpad webapp add Notion https://notion.so"); }],
    ["", "run: pacpad --dry-run tui add htop htop", "shell", function () { window.ppRunInShell("pacpad --dry-run tui add htop htop"); }],
    ["", "run: cat ~/.config/pacpad/config.toml", "shell", function () { window.ppRunInShell("cat ~/.config/pacpad/config.toml"); }],
    ["y", "copy build command", "action", function () { copy(BUILD); }],
    ["?", "keyboard shortcuts", "help", function () { openHelp(); }],
    ["", "open github.com/ric-v/pacpad", "link", function () { location.href = "https://github.com/ric-v/pacpad"; }],
    ["", "report an issue", "link", function () { location.href = "https://github.com/ric-v/pacpad/issues"; }]
  ];
  qas.forEach(function (qa) { CMDS.push(["", "faq: " + $(".q", qa).textContent.replace("▸", "").trim(), "faq", function () { goSec("faq"); toggleQA(qa, true); setTimeout(function () { $(".q", qa).focus({ preventScroll: true }); }, 500); }]); });

  function pkgCmds() {
    return window.PP.packages.concat(window.PP.available).map(function (p) {
      return ["", "pkg: " + p.name + " — " + p.desc, p.installed ? "installed" : "repo", function () { showInDemo(p.name); }];
    });
  }
  function showInDemo(name) {
    takeControl();
    demo.S.modal = null; demo.S.form = null; demo.S.help = false;
    if (demo.S.run && demo.S.run.status !== "running") demo.S.run = null;
    if (demo.S.screen !== "main" || demo.S.run) { toast("the demo is busy. try again in a moment", true); return; }
    demo.setTab(1); demo.S.search.q = name; demo.S.search.cursor = 0; demo.draw();
  }

  function fuzzy(text, q) {
    if (!q) return { s: 0, html: esc(text) };
    var t = text.toLowerCase(), i = 0, s = 0, last = -2, marks = [];
    for (var j = 0; j < t.length && i < q.length; j++) {
      if (t[j] === q[i]) { marks.push(j); s += last === j - 1 ? 6 : 1; if (j === 0 || t[j - 1] === " " || t[j - 1] === ":") s += 4; last = j; i++; }
    }
    if (i < q.length) return null;
    var sub = t.indexOf(q); if (sub >= 0) s += 30 - Math.min(sub, 20);
    var html = "", set = {}; marks.forEach(function (m) { set[m] = 1; });
    for (var k = 0; k < text.length; k++) html += set[k] ? "<mark>" + esc(text[k]) + "</mark>" : esc(text[k]);
    return { s: s - text.length * 0.02, html: html };
  }
  function esc(s) { return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;"); }

  var palRows = [], palCur = 0;
  function renderPal() {
    var q = palQ.value.trim().toLowerCase(), pool = q ? CMDS.concat(pkgCmds()) : CMDS;
    palRows = [];
    pool.forEach(function (c) { var m = fuzzy(c[1], q); if (m) palRows.push({ c: c, m: m }); });
    if (q) palRows.sort(function (a, b) { return b.m.s - a.m.s; });
    palRows = palRows.slice(0, 40);
    palCur = Math.min(palCur, Math.max(0, palRows.length - 1));
    palList.innerHTML = palRows.length ? palRows.map(function (r, i) {
      return '<li role="option" id="po-' + i + '" data-i="' + i + '"' + (i === palCur ? ' class="on" aria-selected="true"' : "") + '><span class="k">' + (r.c[0] ? "<kbd>" + esc(r.c[0]) + "</kbd>" : "") + '</span><span class="t">' + r.m.html + '</span><span class="h">' + r.c[2] + "</span></li>";
    }).join("") : '<li class="empty">no matches for "' + esc(palQ.value) + '"</li>';
    palQ.setAttribute("aria-activedescendant", palRows.length ? "po-" + palCur : "");
    palN.textContent = palRows.length + " match(es)";
    var on = $("li.on", palList); if (on) on.scrollIntoView({ block: "nearest" });
  }
  function openPal() { openOv(palOv); setMode("search"); palQ.value = ""; palCur = 0; renderPal(); palQ.focus(); }
  function runPal(i) { var r = palRows[i]; if (!r) return; closeOv(palOv); lastFocus = null; r.c[3](); }
  palQ.addEventListener("input", function () { palCur = 0; renderPal(); });
  palQ.addEventListener("keydown", function (e) {
    e.stopPropagation();
    if (e.key === "ArrowDown" || (e.key === "n" && e.ctrlKey) || (e.key === "Tab" && !e.shiftKey)) { e.preventDefault(); palCur = Math.min(palRows.length - 1, palCur + 1); renderPal(); }
    else if (e.key === "ArrowUp" || (e.key === "p" && e.ctrlKey) || (e.key === "Tab" && e.shiftKey)) { e.preventDefault(); palCur = Math.max(0, palCur - 1); renderPal(); }
    else if (e.key === "Enter") { e.preventDefault(); runPal(palCur); }
    else if (e.key === "Escape") { e.preventDefault(); closeOv(palOv); }
  });
  palList.addEventListener("pointermove", function (e) { var li = e.target.closest("li[data-i]"); if (li && +li.dataset.i !== palCur) { palCur = +li.dataset.i; $$("li", palList).forEach(function (x) { x.classList.toggle("on", x === li); }); } });
  palList.addEventListener("click", function (e) { var li = e.target.closest("li[data-i]"); if (li) runPal(+li.dataset.i); });

  // ------------------------------------------------------------- actions
  function toggleAuto() { if (demo.auto) { demo.setAutoplay(false); } else { release(); demo.setAutoplay(true); } syncDemoMode(); }
  function resetDemo() { if (demo.auto) { demo.setAutoplay(false); demo.setAutoplay(true); } else demo.reset(); toast("demo reset"); }
  var ACTS = {
    palette: openPal, help: openHelp,
    drive: function () { takeControl(); },
    install: function () { goSec("install"); setTimeout(focusShell, reduced ? 0 : 500); },
    copy: function (b) { copy(b.dataset.copy, b); },
    autoplay: toggleAuto, reset: resetDemo,
    "demo-help": function () { takeControl(false); demo.key("F1"); },
    "guard-ok": guardOk, "guard-cancel": guardCancel
  };
  document.addEventListener("click", function (e) {
    var b = e.target.closest("[data-act]");
    if (!b || !ACTS[b.dataset.act]) return;
    e.preventDefault();
    ACTS[b.dataset.act](b);
  });

  // ------------------------------------------------------------- the keyboard layer
  function typing(t) { return t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable); }
  document.addEventListener("keydown", function (e) {
    if (booting) { e.preventDefault(); endBoot(); return; }
    if (!helpOv.hidden) { if (e.key === "Escape" || e.key === "?" || e.key === "q") { e.preventDefault(); closeOv(helpOv); } return; }
    if (!palOv.hidden) return;

    if (driving) {
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      var k = demoKey(e);
      if (!k) return;
      e.preventDefault();
      if (k === "esc" && !demo.canEscape()) { release(); return; }
      demo.key(k);
      return;
    }
    if (typing(e.target)) return;
    if ((e.ctrlKey || e.metaKey) && (e.key === "k" || e.key === "K")) { e.preventDefault(); openPal(); return; }
    if (e.ctrlKey || e.metaKey || e.altKey) return;

    var key = e.key, onBtn = e.target.closest && e.target.closest("button, a, [tabindex]");
    if (key !== "Enter" && key !== " " && key !== "Escape") lightKey(key);
    if (/^[1-7]$/.test(key)) { e.preventDefault(); goSec(["top", "why", "how", "safety", "install", "keys", "faq"][+key - 1]); return; }
    switch (key) {
      case "/": e.preventDefault(); openPal(); return;
      case "?": e.preventDefault(); openHelp(); return;
      case "j": e.preventDefault(); stepPane(1); return;
      case "k": e.preventDefault(); stepPane(-1); return;
      case "g": e.preventDefault(); scrollTo({ top: 0, behavior: smooth }); setFocus(null); return;
      case "G": e.preventDefault(); scrollTo({ top: document.documentElement.scrollHeight, behavior: smooth }); setFocus(null); return;
      case "d": e.preventDefault(); takeControl(); return;
      case "i": e.preventDefault(); focusShell(); return;
      case "y": e.preventDefault(); copy(BUILD); return;
      case "a": e.preventDefault(); toggleAuto(); toast(demo.auto ? "autoplay on" : "autoplay off"); return;
      case "Escape": setFocus(null); if (document.activeElement && document.activeElement !== document.body) document.activeElement.blur(); return;
      case "Enter": if (focusPane && (!onBtn || onBtn === document.body)) { e.preventDefault(); activatePane(focusPane); } return;
    }
  });

  // ------------------------------------------------------------- boot
  var boot = $("#boot"), booting = false, bootTimer = [];
  function endBoot() {
    if (!booting) return;
    booting = false;
    bootTimer.forEach(clearTimeout);
    boot.classList.add("out");
    setTimeout(function () { boot.classList.remove("on", "out"); }, 620);
  }
  if (!reduced && !store.get("pp-booted") && !location.hash) {
    store.set("pp-booted", "1");
    booting = true;
    boot.classList.add("on");
    var log = $(".boot-log", boot), c = window.PP.counts();
    var lines = [
      ["", ":: starting pacpad web"],
      ["ok", "Mounted site/ (static, no build step)"],
      ["ok", "Loaded fonts: JetBrains Mono"],
      ["ok", "Read pacman.conf: core, extra"],
      ["ok", "Indexed " + c.total + " installed + " + window.PP.available.length + " available packages"],
      ["ok", "Started live demo (autoplay)"],
      ["ok", "Reached target: pacpad"],
      ["", ""],
      ["", "press any key to skip"]
    ];
    lines.forEach(function (l, i) {
      bootTimer.push(setTimeout(function () {
        log.innerHTML += (l[0] === "ok" ? '[  <span class="ok">OK</span>  ] ' : "") + esc(l[1]) + "\n";
      }, 90 + i * 115));
    });
    bootTimer.push(setTimeout(endBoot, 90 + lines.length * 115 + 380));
    boot.addEventListener("pointerdown", endBoot);
  }

  // ------------------------------------------------------------- go
  setMode("normal");
  spy();
  moveHL($("a.on", tabs), true);
})();
