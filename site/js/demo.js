// A playable emulation of pacpad's TUI, drawn on a character grid the same
// way ratatui draws it: same tabs, filters, columns, footer hints, confirm
// modal, protected-package guard, embedded "terminal" and launcher forms.
// Nothing runs on a real machine; output is scripted from real pacman / yay
// transcripts.
(function () {
  "use strict";

  var W = 94, H = 24, BH = H - 8, DET = 30, LIST = W - 2 - DET - 1, BODY = 5;
  var TABS = ["1 Installed", "2 Search", "3 Apps", "4 Updates"];
  var FILTERS = ["All", "Explicit", "Dependencies", "Orphans", "Foreign (AUR)"];
  var SORTS = ["Name", "Size", "Install date"];

  // ---------------------------------------------------------------- grid
  function Grid() {
    this.rows = [];
    for (var y = 0; y < H; y++) {
      var r = [];
      for (var x = 0; x < W; x++) r.push({ ch: " ", fg: "", bg: "", b: false });
      this.rows.push(r);
    }
  }
  function parse(st) {
    var o = { fg: null, bg: null, b: false };
    (st || "").split(" ").forEach(function (t) {
      if (!t) return;
      if (t === "B") o.b = true;
      else if (t.indexOf("bg-") === 0) o.bg = t.slice(3);
      else o.fg = t;
    });
    return o;
  }
  Grid.prototype.put = function (x, y, str, st) {
    if (y < 0 || y >= H) return x;
    var s = parse(st), chars = Array.from(String(str));
    for (var i = 0; i < chars.length; i++) {
      var cx = x + i;
      if (cx < 0 || cx >= W) continue;
      var c = this.rows[y][cx];
      c.ch = chars[i];
      c.fg = s.fg || "";
      if (s.bg !== null) c.bg = s.bg;
      c.b = s.b;
    }
    return x + chars.length;
  };
  Grid.prototype.bg = function (x, y, w, bg) {
    for (var i = 0; i < w; i++) if (this.rows[y] && this.rows[y][x + i]) this.rows[y][x + i].bg = bg;
  };
  Grid.prototype.clear = function (x, y, w, h, bg) {
    for (var j = 0; j < h; j++) for (var i = 0; i < w; i++) {
      var c = this.rows[y + j] && this.rows[y + j][x + i];
      if (c) { c.ch = " "; c.fg = ""; c.bg = bg || ""; c.b = false; }
    }
  };
  Grid.prototype.box = function (x, y, w, h, st) {
    this.put(x, y, "┌" + rep("─", w - 2) + "┐", st);
    for (var j = 1; j < h - 1; j++) { this.put(x, y + j, "│", st); this.put(x + w - 1, y + j, "│", st); }
    this.put(x, y + h - 1, "└" + rep("─", w - 2) + "┘", st);
  };
  var WIDE = /[^\u0000-\u007f─-▟]/;
  Grid.prototype.html = function () {
    var out = [];
    for (var y = 0; y < H; y++) {
      var row = this.rows[y], line = "", run = "", key = null;
      var flush = function () {
        if (run === "") return;
        var k = key.split("|"), cls = [];
        if (k[0]) cls.push("c-" + k[0]);
        if (k[1]) cls.push("b-" + k[1]);
        if (k[2] === "1") cls.push("B");
        line += cls.length ? '<span class="' + cls.join(" ") + '">' + run + "</span>" : run;
        run = "";
      };
      for (var x = 0; x < W; x++) {
        var c = row[x], k2 = c.fg + "|" + c.bg + "|" + (c.b ? 1 : 0);
        if (k2 !== key) { flush(); key = k2; }
        var ch = c.ch === "<" ? "&lt;" : c.ch === ">" ? "&gt;" : c.ch === "&" ? "&amp;" : c.ch;
        run += WIDE.test(c.ch) ? '<i class="w1">' + ch + "</i>" : ch;
      }
      flush();
      out.push(line);
    }
    return out.join("\n");
  };

  function rep(s, n) { return n > 0 ? new Array(n + 1).join(s) : ""; }
  function fit(s, n) { s = String(s); var a = Array.from(s); return a.length > n ? a.slice(0, Math.max(0, n - 1)).join("") + "…" : s + rep(" ", n - a.length); }
  function rfit(s, n) { s = String(s); return s.length >= n ? fit(s, n) : rep(" ", n - s.length) + s; }
  function wrap(text, w) {
    var words = String(text).split(" "), lines = [], cur = "";
    words.forEach(function (wd) {
      if ((cur + (cur ? " " : "") + wd).length > w) { if (cur) lines.push(cur); cur = wd; }
      else cur += (cur ? " " : "") + wd;
    });
    if (cur) lines.push(cur);
    return lines;
  }
  function size(mb) { return mb < 1 ? Math.round(mb * 1024) + " KB" : mb >= 100 ? Math.round(mb) + " MB" : mb.toFixed(1) + " MB"; }
  function mib(mb) { return (mb * 0.953674).toFixed(2) + " MiB"; }

  // ---------------------------------------------------------------- state
  function freshState() {
    return {
      screen: "splash", splash: 0,
      tab: 0,
      inst: { filter: 0, sort: 0, cursor: 0, scroll: 0 },
      search: { q: "", cursor: 0, scroll: 0 },
      apps: { cursor: 0 },
      sel: {},
      modal: null, form: null, help: false, run: null, banner: null,
      tick: 0
    };
  }

  function Demo(el, opts) {
    this.el = el;
    this.opts = opts || {};
    this.pre = el.querySelector("pre");
    this.S = freshState();
    this.gen = 0;
    this.auto = false;
    var self = this;
    window.PP.onChange(function () { self.clampCursors(); self.draw(); });
    setInterval(function () { self.S.tick++; if (self.S.run && self.S.run.status === "running") self.draw(); if (self.S.banner && Date.now() > self.S.banner.until) { self.S.banner = null; self.draw(); } }, 280);
    this.pre.addEventListener("click", function (e) { self.click(e); });
    this.startSplash();
  }

  var D = Demo.prototype;

  D.emit = function (name, data) { if (this.opts[name]) this.opts[name](data); };

  D.startSplash = function () {
    var self = this, S = this.S, g = ++this.gen;
    S.screen = "splash"; S.splash = 0; this.draw();
    var total = 10 + 6 + 1;
    var step = function () {
      if (g !== self.gen) return;
      S.splash++;
      self.draw();
      if (S.splash < total - 1) setTimeout(step, 70);
      else setTimeout(function () { if (g !== self.gen) return; S.screen = "main"; self.draw(); self.emit("onReady"); }, 420);
    };
    setTimeout(step, 260);
  };

  // ---------------------------------------------------------------- data views
  D.installedList = function () {
    var S = this.S, f = S.inst.filter, list = window.PP.packages.filter(function (p) {
      if (f === 1) return p.reason === "explicit";
      if (f === 2) return p.reason === "dependency";
      if (f === 3) return p.reason === "dependency" && p.orphan;
      if (f === 4) return p.repo === "aur";
      return true;
    });
    var s = S.inst.sort;
    list.sort(function (a, b) {
      if (s === 1) return a.size - b.size;
      if (s === 2) return a.date < b.date ? -1 : a.date > b.date ? 1 : 0;
      return a.name < b.name ? -1 : 1;
    });
    return list;
  };
  D.searchList = function () { return window.PP.search(this.S.search.q); };
  D.appList = function () {
    var a = window.PP.apps;
    return a.filter(function (x) { return x.kind === "webapp"; }).concat(a.filter(function (x) { return x.kind === "tui"; }));
  };
  D.clampCursors = function () {
    var S = this.S;
    S.inst.cursor = Math.max(0, Math.min(S.inst.cursor, this.installedList().length - 1));
    S.search.cursor = Math.max(0, Math.min(S.search.cursor, this.searchList().length - 1));
    S.apps.cursor = Math.max(0, Math.min(S.apps.cursor, this.appList().length - 1));
  };
  D.current = function () {
    var S = this.S;
    if (S.tab === 0) return this.installedList()[S.inst.cursor];
    if (S.tab === 1) { var r = this.searchList()[S.search.cursor]; return r && r.p; }
    return null;
  };
  D.targets = function () {
    var names = Object.keys(this.S.sel).filter(function (k) { return this.S.sel[k]; }, this);
    if (names.length) return names;
    var c = this.current();
    return c ? [c.name] : [];
  };

  // ---------------------------------------------------------------- drawing
  D.draw = function () {
    var g = new Grid(), S = this.S;
    if (S.screen === "splash") this.drawSplash(g);
    else if (S.screen === "shell") this.drawShell(g);
    else {
      this.drawFrame(g);
      if (S.run) this.drawRun(g);
      else {
        if (S.tab === 0) this.drawInstalled(g);
        else if (S.tab === 1) this.drawSearch(g);
        else if (S.tab === 2) this.drawApps(g);
        else this.drawUpdates(g);
      }
      if (S.modal) this.drawModal(g);
      if (S.form) this.drawForm(g);
      if (S.help) this.drawHelp(g);
    }
    this.pre.innerHTML = g.html();
  };

  D.drawSplash = function (g) {
    var S = this.S, DOTS = 10, WORD = "pacpad", total = DOTS + WORD.length + 1;
    var pos = Math.min(S.splash, total - 1), x0 = Math.floor((W - total) / 2), y = Math.floor(H / 2) - 1;
    for (var c = 0; c < total; c++) {
      var x = x0 + c;
      if (c < pos) { if (c >= DOTS && c < DOTS + WORD.length) g.put(x, y, WORD[c - DOTS], c - DOTS < 3 ? "r B" : "a B"); }
      else if (c === pos) g.put(x, y, pos % 2 ? "ᗧ" : "●", "y B");
      else if (c < DOTS) g.put(x, y, "·", "f");
    }
    g.put(Math.floor((W - 26) / 2), y + 2, "reading the pacman index…", "f");
  };

  D.drawShell = function (g) {
    var p = window.PP.user + "@" + window.PP.host + " ~ $ ";
    g.put(1, 1, p, "g"); g.put(1 + p.length, 1, "pacpad", "");
    g.put(1, 2, p, "g"); g.put(1 + p.length, 2, " ", "bg-fg");
    g.put(1, 4, "pacpad exited. press enter to run it again.", "f");
  };

  D.drawFrame = function (g) {
    var S = this.S;
    g.box(0, 0, W, H, "b");
    g.put(1, 0, " pac", "r B"); g.put(5, 0, "pad ", "a B");
    var x = 1; this.hitTabs = [];
    TABS.forEach(function (t, i) {
      if (i > 0) x = g.put(x, 1, " │ ", "b");
      var x0 = x;
      x = g.put(x, 1, " " + t + " ", i === S.tab ? "ink bg-acc B" : "d");
      this.hitTabs.push([x0, x, i]);
    }, this);
    g.put(W - 2 - window.PP.host.length, 1, window.PP.host, "f");
    var split = !S.run && S.tab < 3;
    g.put(0, 2, "├" + rep("─", W - 2) + "┤", "b");
    g.put(0, 4, "├" + rep("─", W - 2) + "┤", "b");
    g.put(0, BODY + BH, "├" + rep("─", W - 2) + "┤", "b");
    if (split) {
      g.put(1 + LIST, 4, "┬", "b"); g.put(1 + LIST, BODY + BH, "┴", "b");
      for (var y = BODY; y < BODY + BH; y++) g.put(1 + LIST, y, "│", "b");
    }
    this.drawFooter(g);
  };

  D.drawFooter = function (g) {
    var S = this.S, y = BODY + BH + 1, x = 2;
    if (S.banner) {
      g.bg(1, y, W - 2, S.banner.err ? "red" : "grn");
      g.put(2, y, (S.banner.err ? "ᗤ " : "ᗧ ") + S.banner.text, "ink B");
      return;
    }
    var items;
    if (S.run) {
      g.put(2, y, S.run.status === "running" ? "your keystrokes go to the running command -- Ctrl+C interrupts it, not pacpad" : "press any key to return to pacpad", "d");
      return;
    }
    if (S.tab === 0) items = [["↑↓/jk", "f", "nav"], ["←→/hl", "f", "filter"], ["space", "f", "select"], ["i", "g", "install"], ["d", "r", "remove"], ["r", "a", "reinstall"], ["e/D", "f", "mark"], ["u", "m", "update"], ["f", "f", "files"], ["/", "f", "search"], ["q", "f", "quit"]];
    else if (S.tab === 1) items = [["↑↓", "f", "nav"], ["type", "f", "to search"], ["space", "f", "select"], ["enter", "g", "install"], ["esc", "f", "clear"], ["tab", "f", "switch"]];
    else if (S.tab === 2) items = [["↑↓/jk", "f", "nav"], ["a", "g", "add webapp"], ["t", "g", "add tui app"], ["enter", "a", "launch"], ["e", "f", "edit"], ["d", "r", "remove"], ["tab", "f", "switch"], ["q", "f", "quit"]];
    else items = [["1-4", "f", "jump tab"], ["F1", "f", "help"], ["q", "f", "quit"]];
    items.forEach(function (it) {
      if (x + it[0].length + it[2].length + 3 > W - 2) return;
      x = g.put(x, y, it[0], it[1] === "f" ? "fg" : it[1]);
      x = g.put(x, y, " " + it[2] + "  ", "d");
    });
  };

  D.drawDetail = function (g, p) {
    var x = 2 + LIST, y = BODY, w = DET - 1;
    if (!p) { g.put(x, y, "(nothing selected)", "f"); return; }
    var nx = g.put(x, y, fit(p.name, w - 9).trimEnd(), "fg B");
    g.put(nx + 1, y, p.repo === "aur" ? "[aur]" : "[" + p.repo + "]", p.repo === "aur" ? "m" : "d");
    y++;
    wrap(p.desc, w).slice(0, 2).forEach(function (l) { g.put(x, y++, l, "d"); });
    y++;
    var meta = [["VERSION", p.version, "fg"], ["SIZE", size(p.size), "fg"], ["LICENSE", p.license, "fg"]];
    if (p.installed) {
      meta.push(["INSTALLED", p.date, "fg"]);
      meta.push(["REASON", p.reason === "explicit" ? "Explicit" : p.orphan ? "Dependency (orphan)" : "Dependency", p.reason === "explicit" ? "g" : "d"]);
    } else meta.push(["STATUS", "not installed", "f"]);
    meta.push(["UPSTREAM", p.url.replace(/^https?:\/\//, ""), "a"]);
    meta.forEach(function (m) { g.put(x, y, fit(m[0], 11), "f"); g.put(x + 11, y, fit(m[1], w - 11), m[2]); y++; });
    var prot = window.PP.protection(p);
    if (prot) { g.put(x, y++, fit("PROTECTED   " + prot, w), "r"); }
    y++;
    if (p.deps.length && y < BODY + BH - 1) {
      g.put(x, y++, "DEPENDENCIES", "f");
      wrap(p.deps.join(" · "), w).forEach(function (l) { if (y < BODY + BH) g.put(x, y++, l, "d"); });
    }
  };

  D.listWindow = function (state, n, rows) {
    if (state.cursor < state.scroll) state.scroll = state.cursor;
    if (state.cursor >= state.scroll + rows) state.scroll = state.cursor - rows + 1;
    state.scroll = Math.max(0, Math.min(state.scroll, Math.max(0, n - rows)));
    return state.scroll;
  };

  D.drawInstalled = function (g) {
    var S = this.S, list = this.installedList(), x = 2;
    FILTERS.forEach(function (f, i) { x = g.put(x, 3, " " + f + " ", i === S.inst.filter ? "ink bg-acc B" : "d"); x += 1; });
    var srt = "Sort: " + SORTS[S.inst.sort] + " ▲";
    g.put(W - 2 - srt.length, 3, srt, "f");
    g.put(1, BODY, fit("       NAME", 25) + fit("VERSION", 15) + fit("REPO", 9) + rfit("SIZE", 8), "f");
    var rows = BH - 1, top = this.listWindow(S.inst, list.length, rows);
    this.hitList = { y0: BODY + 1, top: top, n: list.length, which: "inst" };
    for (var i = 0; i < rows && top + i < list.length; i++) {
      var p = list[top + i], y = BODY + 1 + i, cur = top + i === S.inst.cursor;
      if (cur) g.bg(1, y, LIST, "sel");
      var sel = !!S.sel[p.name];
      g.put(2, y, sel ? "[x]" : "[ ]", sel ? "a" : "f");
      g.put(6, y, p.reason === "explicit" ? "●" : "○", p.reason === "explicit" ? "g" : "f");
      g.put(8, y, fit(p.name, 18), cur ? "fg B" : "fg");
      g.put(26, y, fit(p.version, 14), "d");
      g.put(41, y, fit("[" + p.repo + "]", 9), p.repo === "aur" ? "m" : "d");
      g.put(50, y, rfit(size(p.size), 8), "d");
      if (cur) g.put(1, y, "▌", "a");
    }
    if (!list.length) g.put(4, BODY + 2, "no packages match this filter", "f");
    this.drawDetail(g, list[S.inst.cursor]);
  };

  D.drawSearch = function (g) {
    var S = this.S, res = this.searchList();
    var x = g.put(2, 3, "Search: ", "f"); x = g.put(x, 3, "› ", "a"); x = g.put(x, 3, S.search.q, "fg");
    if (S.tick % 2 === 0 || !this.focused) g.put(x, 3, " ", "bg-fg");
    var cnt = res.length + " match(es)"; g.put(W - 2 - cnt.length, 3, cnt, "f");
    g.put(1, BODY, fit("   NAME", 25) + fit("VERSION", 14) + fit("SOURCE", 9) + "STATUS", "f");
    var rows = BH - 1, top = this.listWindow(S.search, res.length, rows);
    this.hitList = { y0: BODY + 1, top: top, n: res.length, which: "search" };
    for (var i = 0; i < rows && top + i < res.length; i++) {
      var r = res[top + i], p = r.p, y = BODY + 1 + i, cur = top + i === S.search.cursor;
      if (cur) { g.bg(1, y, LIST, "sel"); g.put(1, y, "▌", "a"); }
      g.put(2, y, S.sel[p.name] ? "✓" : " ", "a");
      g.put(4, y, fit(p.name, 21), cur ? "fg B" : "fg");
      g.put(26, y, fit(p.version, 13), "d");
      g.put(40, y, fit("[" + p.repo + "]", 9), p.repo === "aur" ? "m" : "d");
      g.put(49, y, p.installed ? "✓ installed" : "not installed", p.installed ? "g" : "f");
    }
    if (!S.search.q) { g.put(4, BODY + 2, "type to search installed packages and every configured repo", "f"); g.put(4, BODY + 3, "try: zellij, ripgrep, editor, git", "f"); }
    else if (!res.length) g.put(4, BODY + 2, "no matches for \"" + S.search.q + "\"", "f");
    this.drawDetail(g, res[S.search.cursor] && res[S.search.cursor].p);
  };

  D.drawApps = function (g) {
    var S = this.S, list = this.appList(), wc = list.filter(function (a) { return a.kind === "webapp"; }).length, tc = list.length - wc;
    g.put(2, 3, "Managed by pacpad — " + wc + " web app" + (wc === 1 ? "" : "s") + " · " + tc + " tui app" + (tc === 1 ? "" : "s"), "d");
    var y = BODY, idx = 0;
    this.hitApps = [];
    var section = function (title, kind) {
      g.put(2, y++, title, "f");
      list.forEach(function (a, i) {
        if (a.kind !== kind) return;
        var cur = i === S.apps.cursor;
        if (cur) { g.bg(1, y, LIST, "sel"); g.put(1, y, "▌", "a"); }
        g.put(4, y, fit(a.name, 18), cur ? "fg B" : "fg");
        g.put(23, y, fit(a.url || a.command, LIST - 23), "d");
        this.hitApps.push([y, i]);
        y++; idx++;
      }, this);
      y++;
    }.bind(this);
    section("WEB APPS", "webapp");
    section("TUI APPS", "tui");
    var a = list[S.apps.cursor], x = 2 + LIST, w = DET - 1, dy = BODY;
    if (!a) { g.put(x, dy, "(nothing selected)", "f"); return; }
    var nx = g.put(x, dy, a.name, "fg B");
    g.put(nx + 1, dy++, a.kind === "webapp" ? "[webapp]" : "[tui]", a.kind === "webapp" ? "a" : "g");
    g.put(x, dy++, fit(a.url || a.command, w), "d");
    dy++;
    var cls = a.kind === "webapp" ? "pacpad-" + window.PP.slug(a.name) : "pacpad-tui-" + (a.style || "float");
    g.put(x, dy, "WINDOW CLASS", "f"); g.put(x + 13, dy++, fit(cls, w - 13), "fg");
    g.put(x, dy, "CREATED", "f"); g.put(x + 13, dy++, a.created, "fg");
    dy++;
    g.put(x, dy++, "LAUNCH COMMAND", "f");
    var cmd = a.kind === "webapp" ? "chromium --app=" + a.url + " --class=" + cls + " --name=" + a.name : "xdg-terminal-exec --app-id=" + cls + " -e sh -c '" + a.command + "'";
    wrap(cmd.replace(/([=/])/g, "$1 ").replace(/ $/, ""), w).forEach(function (l) { if (dy < BODY + BH) g.put(x, dy++, l.replace(/([=/]) /g, "$1"), "d"); });
  };

  D.drawUpdates = function (g) {
    var y = BODY + 4;
    g.put(Math.floor((W - 7) / 2), y, "Updates", "fg B");
    wrap("Version-diff listing is still pending -- press u on the Installed tab to run a full system update now.", 60)
      .forEach(function (l, i) { g.put(Math.floor((W - l.length) / 2), y + 2 + i, l, "d"); });
  };

  D.drawRun = function (g) {
    var r = this.S.run, mouth = this.S.tick % 2 ? "ᗤ" : "ᗧ";
    var lbl = r.status === "running" ? " " + mouth + " Running: " + r.title + " " : r.status === "done" ? " ✓ Done: " + r.title + " " : " ✗ Failed: " + r.title + " ";
    g.put(2, 3, lbl, r.status === "running" ? "ink bg-acc B" : r.status === "done" ? "ink bg-grn B" : "ink bg-red B");
    var lines = r.lines.slice(-BH);
    lines.forEach(function (l, i) { g.put(2, BODY + i, fit(l.t, W - 4), l.c || "fg"); });
    if (r.wait && lines.length) {
      var last = lines[lines.length - 1], cx = 2 + Array.from(last.t).length;
      if (this.S.tick % 2 === 0) g.put(Math.min(cx, W - 3), BODY + lines.length - 1, " ", "bg-fg");
    }
  };

  D.modalBox = function (g, w, h, title, st, titleSt) {
    var x = Math.floor((W - w) / 2), y = Math.floor((H - h) / 2);
    g.clear(x, y, w, h, "");
    g.box(x, y, w, h, st);
    g.put(x + Math.floor((w - title.length) / 2), y, title, titleSt);
    return { x: x + 2, y: y + 1, w: w - 4, h: h - 2, bx: x, by: y };
  };

  D.drawModal = function (g) {
    var m = this.S.modal;
    if (m.kind === "removeApp") {
      var b = this.modalBox(g, 64, 9, " Remove Launcher ", "r", "ink bg-red B"), y = b.y;
      g.put(b.x, y, m.app.name, "fg B"); g.put(b.x + m.app.name.length + 1, y++, m.app.kind === "webapp" ? "[webapp]" : "[tui]", "a");
      y++;
      g.put(b.x, y++, "removes", "d");
      g.put(b.x, y++, "~/.local/share/applications/pacpad-" + window.PP.slug(m.app.name) + ".desktop", "fg");
      y++;
      this.keyHints(g, b.x, y, true, "remove");
      return;
    }
    var guarded = m.guard.length > 0, lines = 6 + (m.action === "remove" ? 3 : 0) + (guarded ? 3 + m.guard.length : 0);
    var title = { install: " Install ", remove: " Confirm Removal ", reinstall: " Reinstall ", explicit: " Mark Explicit ", deps: " Mark as Dependency ", update: " Update System ", files: " List Files " }[m.action];
    var red = m.action === "remove";
    var b2 = this.modalBox(g, 66, lines + 2, title, red ? "r" : "a", red ? "ink bg-red B" : "ink bg-acc B"), y2 = b2.y;
    if (m.pkgs.length) { g.put(b2.x, y2++, fit(m.pkgs.length + " package" + (m.pkgs.length === 1 ? "" : "s") + ": " + m.pkgs.join(", "), b2.w), "d"); y2++; }
    g.put(b2.x, y2++, "$", "f");
    g.put(b2.x, y2++, fit(this.cmd(m), b2.w), "fg B");
    y2++;
    if (m.action === "remove") {
      g.put(b2.x, y2++, m.recursive ? "-Rns: also removes now-unneeded dependencies and configs" : "-R: removes only the listed package(s)", "f");
      g.put(b2.x, y2++, "r  toggle -Rns / -R", "f");
      y2++;
    }
    if (guarded) {
      g.put(b2.x, y2++, "⚠ this touches a protected package:", "r B");
      m.guard.forEach(function (gr) { g.put(b2.x + 2, y2++, gr[0] + " — " + gr[1], "r"); });
      y2++;
      var tx = g.put(b2.x, y2, "type REMOVE to confirm: ", "d");
      tx = g.put(tx, y2, m.typed, "r B");
      g.put(tx, y2++, "█", "r");
      y2++;
    }
    this.keyHints(g, b2.x, b2.y + lines - 1, !guarded || m.typed === "REMOVE", "confirm");
  };

  D.keyHints = function (g, x, y, can, label) {
    x = g.put(x, y, " esc ", "ink bg-fnt B");
    x = g.put(x, y, " cancel  ", "d");
    x = g.put(x, y, " enter ", can ? "ink bg-grn B" : "ink bg-fnt B");
    g.put(x, y, " " + label, "d");
  };

  D.drawForm = function (g) {
    var f = this.S.form, web = f.kind === "webapp";
    var fields = web ? [["NAME", "name"], ["URL", "url"], ["ICON (URL, PATH, OR BLANK FOR AUTO)", "icon"]] : [["NAME", "name"], ["COMMAND", "command"], ["ICON PATH (OPTIONAL)", "icon"], ["WINDOW STYLE", "style"]];
    var title = (f.editing ? (web ? " Edit Web App " : " Edit TUI App ") : (web ? " New Web App " : " New TUI App "));
    var h = fields.length * 3 + 5, b = this.modalBox(g, 64, h, title, "a", "ink bg-acc B"), y = b.y + 1, self = this;
    fields.forEach(function (fd, i) {
      var on = i === f.focus;
      g.put(b.x, y, fd[1] === "style" && on ? "WINDOW STYLE (←→ to change)" : fd[0], on ? "a B" : "f");
      if (fd[1] === "style") {
        var x = g.put(b.x, y + 1, " Float ", f.style === "float" ? "ink bg-acc B" : "d");
        g.put(x + 2, y + 1, " Tile ", f.style === "tile" ? "ink bg-acc B" : "d");
      } else {
        var v = f[fd[1]] || "", tx = g.put(b.x, y + 1, fit(v, b.w - 1).trimEnd(), "fg");
        if (on && self.S.tick % 2 === 0) g.put(tx, y + 1, " ", "bg-fg");
        g.put(b.x, y + 2, rep("─", b.w), on ? "a" : "b");
      }
      y += 3;
    });
    if (f.error) g.put(b.x, y, "⚠ " + f.error, "r");
    var hy = b.y + b.h - 1, x2 = g.put(b.x, hy, " esc ", "ink bg-fnt B");
    x2 = g.put(x2, hy, " cancel  ", "d"); x2 = g.put(x2, hy, " tab ", "ink bg-fnt B"); x2 = g.put(x2, hy, " next field  ", "d");
    x2 = g.put(x2, hy, " enter ", "ink bg-grn B"); g.put(x2, hy, f.editing ? " save" : " create", "d");
  };

  var HELP = [
    ["Global", [["1-4 / Tab", "switch tabs"], ["F1", "toggle this help"], ["q / Ctrl+C", "quit"]]],
    ["Installed", [["↑↓ / jk", "move"], ["←→ / hl", "cycle filter"], ["space", "select"], ["s", "cycle sort"], ["/", "jump to search"], ["i / d / r", "install / remove / reinstall"], ["e / D", "mark explicit / as dependency"], ["u", "update system"], ["f / o", "list files / open URL"]]],
    ["Search", [["type", "filter by name or description"], ["↑↓", "move"], ["space", "select"], ["enter", "install"], ["esc", "clear query"]]],
    ["Apps", [["↑↓ / jk", "move"], ["a / t", "add webapp / add TUI app"], ["enter", "launch"], ["e / d", "edit / remove"]]]
  ];
  D.drawHelp = function (g) {
    var b = this.modalBox(g, 84, 19, " Keybindings ", "a", "ink bg-acc B");
    var col = function (secs, x) {
      var y = b.y + 1;
      secs.forEach(function (s, i) {
        if (i) y++;
        g.put(x, y++, s[0], "a B");
        s[1].forEach(function (kv) { g.put(x + 1, y, fit(kv[0], 12), "fg"); g.put(x + 13, y++, kv[1], "d"); });
      });
    };
    col([HELP[0], HELP[1]], b.x);
    col([HELP[2], HELP[3]], b.x + 42);
    g.put(b.x, b.y + b.h - 1, "F1 · esc · q · enter  close", "f");
  };

  // ---------------------------------------------------------------- commands + scripts
  D.cmd = function (m) {
    var p = m.pkgs.join(" "), aur = m.pkgs.some(function (n) { var x = window.PP.find(n); return x && x.repo === "aur"; });
    switch (m.action) {
      case "install": case "reinstall": return aur ? "yay -S " + p : "sudo pacman -S " + p;
      case "remove": return "sudo pacman " + (m.recursive ? "-Rns " : "-R ") + p;
      case "explicit": return "sudo pacman -D --asexplicit " + p;
      case "deps": return "sudo pacman -D --asdeps " + p;
      case "update": return "yay -Syu";
      case "files": return "pacman -Ql " + p;
    }
    return "";
  };

  D.openModal = function (action) {
    var S = this.S, pkgs = action === "update" ? [] : this.targets();
    if (action === "files") pkgs = pkgs.slice(0, 1);
    if (!pkgs.length && action !== "update") return;
    var guard = [];
    if (action === "remove") pkgs.forEach(function (n) { var r = window.PP.protection(window.PP.find(n)); if (r) guard.push([n, r]); });
    S.modal = { kind: "confirm", action: action, pkgs: pkgs, recursive: true, guard: guard, typed: "" };
  };

  D.confirm = function () {
    var S = this.S, m = S.modal;
    if (m.kind === "removeApp") {
      window.PP.removeApp(m.app.kind, m.app.name);
      S.modal = null; this.flash("removed " + JSON.stringify(m.app.name));
      return;
    }
    if (m.guard.length && m.typed !== "REMOVE") return;
    S.modal = null;
    this.run(m);
  };

  D.flash = function (text, err) { this.S.banner = { text: text, err: !!err, until: Date.now() + 3800 }; this.draw(); };

  D.run = function (m) {
    var S = this.S, self = this, g = this.gen, cmd = this.cmd(m);
    var r = S.run = { title: cmd, lines: [], status: "running", wait: null };
    var PP = window.PP, pkgs = m.pkgs.map(function (n) { return PP.find(n); }).filter(Boolean);
    var out = function (t, c) { r.lines.push({ t: t, c: c }); self.draw(); };
    var sleep = function (ms) { return new Promise(function (res) { setTimeout(function () { res(g === self.gen); }, ms); }); };
    var wait = function (type, prompt) { out(prompt, "fg"); return new Promise(function (res) { r.wait = { type: type, buf: "", resolve: res }; self.draw(); self.emit("onWait", type); }); };
    var bar = function (label, ms, tail) {
      var line = { t: "", c: "fg" }; r.lines.push(line);
      var n = 18, start = Date.now();
      return new Promise(function (res) {
        var tickf = function () {
          if (g !== self.gen) return res(false);
          var k = Math.min(1, (Date.now() - start) / ms), fill = Math.round(n * k);
          line.t = fit(label, tail ? 40 : 50) + (tail ? tail(k) : "") + "[" + rep("#", fill) + rep("-", n - fill) + "] " + rfit(Math.round(k * 100) + "%", 4);
          self.draw();
          if (k < 1) setTimeout(tickf, 60); else res(true);
        };
        tickf();
      });
    };
    var sudo = function () { return wait("password", "[sudo] password for " + PP.user + ": "); };
    var names = function () { return pkgs.map(function (p) { return p.name + "-" + p.version; }).join("  "); };
    var total = pkgs.reduce(function (a, p) { return a + p.size; }, 0);

    var script = (async function () {
      var a = m.action, n = pkgs.length;
      if (a === "remove") {
        await sudo(); out("checking dependencies...", "fg"); await sleep(500); out("");
        out("Packages (" + n + ") " + names()); out("");
        out("Total Removed Size:  " + mib(total)); out("");
        var ok = await wait("yn", ":: Do you want to remove these packages? [Y/n] ");
        if (!ok) return "aborted";
        out(":: Processing package changes...", "fg");
        for (var i = 0; i < n; i++) if (!(await bar("(" + (i + 1) + "/" + n + ") removing " + pkgs[i].name, 700))) return;
        out(":: Running post-transaction hooks...", "fg"); await sleep(300);
        out("(1/1) Arming ConditionNeedsUpdate..."); await sleep(300);
        PP.remove(m.pkgs);
        return "removed " + m.pkgs.join(", ");
      }
      if (a === "install" || a === "reinstall") {
        var aur = pkgs.filter(function (p) { return p.repo === "aur"; });
        if (aur.length) {
          out("AUR Explicit (" + aur.length + "): " + aur.map(function (p) { return p.name + "-" + p.version; }).join(", "), "a");
          aur.forEach(function (p, i) { out("  " + (i + 1) + " " + fit(p.name, 36) + "(Build Files Exist)", "fg"); });
          out("==> Packages to cleanBuild?", "g");
          if (!(await wait("enter", "==> [N]one [A]ll [Ab]ort [I]nstalled [No]tInstalled or (1 2 3, 1-3, ^4) "))) return;
          out("==> Diffs to show?", "g");
          if (!(await wait("enter", "==> [N]one [A]ll [Ab]ort [I]nstalled [No]tInstalled or (1 2 3, 1-3, ^4) "))) return;
          for (var j = 0; j < aur.length; j++) {
            var p = aur[j];
            out("==> Making package: " + p.name + " " + p.version + " (Sat 07 Nov 2026)", "g"); await sleep(300);
            out("==> Checking runtime dependencies...", "g"); await sleep(250);
            out("==> Retrieving sources...", "g");
            if (!(await bar("  -> Downloading " + p.name + "-" + p.version.split("-")[0] + ".tar.gz...", 1400))) return;
            out("==> Validating source files with sha256sums...", "g"); await sleep(300);
            out("    " + p.name + ".tar.gz ... Passed"); await sleep(200);
            out("==> Entering fakeroot environment...", "g"); await sleep(250);
            out("==> Starting package()...", "g"); await sleep(450);
            out("==> Finished making: " + p.name + " " + p.version + " (Sat 07 Nov 2026)", "g"); await sleep(300);
          }
        }
        if (!(await sudo())) return;
        if (a === "reinstall" || pkgs.some(function (p) { return p.installed; })) pkgs.forEach(function (p) { if (p.installed) out("warning: " + p.name + "-" + p.version + " is up to date -- reinstalling", "m"); });
        out(aur.length ? "loading packages..." : "resolving dependencies..."); await sleep(400);
        out("looking for conflicting packages..."); await sleep(350); out("");
        out("Packages (" + n + ") " + names()); out("");
        out("Total Download Size:    " + mib(total * 0.27)); out("Total Installed Size:   " + mib(total)); out("");
        if (!(await wait("yn", ":: Proceed with installation? [Y/n] "))) return "aborted";
        out(":: Retrieving packages...", "fg");
        for (var k = 0; k < n; k++) if (!(await bar(" " + pkgs[k].name + "-" + pkgs[k].version + "-x86_64", 1100, function (q) { return rfit((pkgs[k].size * 0.27 * 0.953674 * q).toFixed(1) + " MiB", 10) + "  12.4 MiB/s "; }))) return;
        var steps = ["checking keys in keyring", "checking package integrity", "loading package files", "checking for file conflicts"];
        for (var s = 0; s < steps.length; s++) if (!(await bar("(" + n + "/" + n + ") " + steps[s], 260))) return;
        out(":: Processing package changes...", "fg");
        for (var q2 = 0; q2 < n; q2++) if (!(await bar("(" + (q2 + 1) + "/" + n + ") " + (pkgs[q2].installed ? "reinstalling " : "installing ") + pkgs[q2].name, 700))) return;
        out(":: Running post-transaction hooks...", "fg"); await sleep(250);
        out("(1/2) Arming ConditionNeedsUpdate..."); await sleep(200);
        out("(2/2) Refreshing PackageKit..."); await sleep(250);
        PP.install(m.pkgs);
        return (a === "reinstall" ? "reinstalled " : "installed ") + m.pkgs.join(", ");
      }
      if (a === "explicit" || a === "deps") {
        if (!(await sudo())) return;
        m.pkgs.forEach(function (nm) { out(nm + ": install reason has been set to '" + (a === "explicit" ? "explicitly installed" : "installed as dependency") + "'"); });
        PP.mark(m.pkgs, a === "explicit" ? "explicit" : "dependency");
        return "marked " + m.pkgs.join(", ");
      }
      if (a === "update") {
        out(":: Synchronizing package databases...", "fg");
        if (!(await bar(" core", 500))) return;
        if (!(await bar(" extra", 900))) return;
        out(":: Searching AUR for updates...", "fg"); await sleep(700);
        out(":: Searching databases for updates...", "fg"); await sleep(400);
        out(" there is nothing to do");
        return "system is up to date";
      }
      if (a === "files") {
        var nm2 = m.pkgs[0], fl = ["/usr/", "/usr/bin/", "/usr/bin/" + nm2, "/usr/share/", "/usr/share/doc/", "/usr/share/doc/" + nm2 + "/", "/usr/share/doc/" + nm2 + "/README.md", "/usr/share/licenses/", "/usr/share/licenses/" + nm2 + "/", "/usr/share/licenses/" + nm2 + "/LICENSE", "/usr/share/man/", "/usr/share/man/man1/", "/usr/share/man/man1/" + nm2 + ".1.gz"];
        for (var f = 0; f < fl.length; f++) { out(nm2 + " " + fl[f]); await sleep(40); }
        return "listed files of " + nm2;
      }
    })();

    script.then(function (msg) {
      if (g !== self.gen || !S.run) return;
      r.wait = null;
      r.status = msg === "aborted" ? "failed" : "done";
      r.result = msg === "aborted" ? "transaction aborted" : msg;
      S.sel = {};
      self.draw();
      self.emit("onRunEnd", r.status);
    });
  };

  // ---------------------------------------------------------------- input
  D.key = function (k, auto) {
    var S = this.S;
    this.emit("onKey", { key: k, auto: !!auto });
    if (S.screen === "splash") return true;
    if (S.screen === "shell") { if (k === "enter") this.startSplash(); return true; }

    if (S.run) {
      var r = S.run;
      if (r.wait) {
        if (r.wait.type === "password") {
          if (k === "enter") { var w = r.wait; r.wait = null; w.resolve(true); }
          else if (k === "backspace") r.wait.buf = r.wait.buf.slice(0, -1);
          else if (k.length === 1) r.wait.buf += k;
        } else if (r.wait.type === "yn") {
          if (k === "enter" || k === "y" || k === "Y") { r.lines[r.lines.length - 1].t += k === "enter" ? "" : k; var w2 = r.wait; r.wait = null; w2.resolve(true); }
          else if (k === "n" || k === "N") { r.lines[r.lines.length - 1].t += k; var w3 = r.wait; r.wait = null; w3.resolve(false); }
        } else if (k === "enter") { var w4 = r.wait; r.wait = null; w4.resolve(true); }
        this.draw(); return true;
      }
      if (r.status !== "running") {
        S.run = null;
        if (r.result) this.flash(r.result, r.status === "failed");
        this.draw();
      }
      return true;
    }

    if (S.help) { if (k === "F1" || k === "esc" || k === "q" || k === "enter") S.help = false; this.draw(); return true; }
    if (S.modal) return this.keyModal(k);
    if (S.form) return this.keyForm(k);

    if (k === "F1") { S.help = true; this.draw(); return true; }
    if (k === "tab") { this.setTab((S.tab + 1) % 4); return true; }
    if (k === "backtab") { this.setTab((S.tab + 3) % 4); return true; }

    if (S.tab === 1) return this.keySearch(k);
    if (/^[1-4]$/.test(k)) { this.setTab(+k - 1); return true; }
    if (k === "q") { S.screen = "shell"; this.draw(); return true; }
    if (S.tab === 0) return this.keyInstalled(k);
    if (S.tab === 2) return this.keyApps(k);
    return k !== "esc";
  };

  D.setTab = function (t) { this.S.tab = t; this.S.sel = {}; this.clampCursors(); this.draw(); };

  D.keyInstalled = function (k) {
    var S = this.S, n = this.installedList().length;
    switch (k) {
      case "down": case "j": S.inst.cursor = Math.min(n - 1, S.inst.cursor + 1); break;
      case "up": case "k": S.inst.cursor = Math.max(0, S.inst.cursor - 1); break;
      case "right": case "l": S.inst.filter = (S.inst.filter + 1) % FILTERS.length; S.inst.cursor = 0; S.inst.scroll = 0; S.sel = {}; break;
      case "left": case "h": S.inst.filter = (S.inst.filter + FILTERS.length - 1) % FILTERS.length; S.inst.cursor = 0; S.inst.scroll = 0; S.sel = {}; break;
      case "space": var c = this.current(); if (c) S.sel[c.name] = !S.sel[c.name]; break;
      case "s": S.inst.sort = (S.inst.sort + 1) % SORTS.length; break;
      case "/": this.setTab(1); return true;
      case "i": this.openModal("install"); break;
      case "d": this.openModal("remove"); break;
      case "r": this.openModal("reinstall"); break;
      case "e": this.openModal("explicit"); break;
      case "D": this.openModal("deps"); break;
      case "u": this.openModal("update"); break;
      case "f": this.openModal("files"); break;
      case "o": var p = this.current(); if (p) this.flash("opened " + p.url); return true;
      default: return k !== "esc";
    }
    this.draw(); return true;
  };

  D.keySearch = function (k) {
    var S = this.S, n = this.searchList().length;
    if (k === "down") S.search.cursor = Math.min(n - 1, S.search.cursor + 1);
    else if (k === "up") S.search.cursor = Math.max(0, S.search.cursor - 1);
    else if (k === "space") { var c = this.current(); if (c) S.sel[c.name] = !S.sel[c.name]; }
    else if (k === "enter") this.openModal("install");
    else if (k === "esc") { if (!S.search.q) return false; S.search.q = ""; S.search.cursor = 0; }
    else if (k === "backspace") { S.search.q = S.search.q.slice(0, -1); S.search.cursor = 0; }
    else if (k.length === 1) { S.search.q += k; S.search.cursor = 0; S.search.scroll = 0; }
    else return true;
    this.draw(); return true;
  };

  D.keyApps = function (k) {
    var S = this.S, list = this.appList(), a = list[S.apps.cursor];
    switch (k) {
      case "down": case "j": S.apps.cursor = Math.min(list.length - 1, S.apps.cursor + 1); break;
      case "up": case "k": S.apps.cursor = Math.max(0, S.apps.cursor - 1); break;
      case "a": S.form = { kind: "webapp", name: "", url: "", icon: "", focus: 0 }; break;
      case "t": S.form = { kind: "tui", name: "", command: "", icon: "", style: "float", focus: 0 }; break;
      case "e": if (a) S.form = a.kind === "webapp" ? { kind: "webapp", name: a.name, url: a.url, icon: "", focus: 0, editing: a } : { kind: "tui", name: a.name, command: a.command, icon: "", style: a.style || "float", focus: 0, editing: a }; break;
      case "d": if (a) S.modal = { kind: "removeApp", app: a }; break;
      case "enter": if (a) { this.flash("launched " + a.name); return true; } break;
      default: return k !== "esc";
    }
    this.draw(); return true;
  };

  D.keyModal = function (k) {
    var m = this.S.modal;
    if (k === "esc") { this.S.modal = null; }
    else if (k === "enter") this.confirm();
    else if (m.kind === "confirm" && m.action === "remove" && k === "r") m.recursive = !m.recursive;
    else if (m.kind === "confirm" && m.guard.length) {
      if (k === "backspace") m.typed = m.typed.slice(0, -1);
      else if (k.length === 1 && m.typed.length < 12) m.typed += k;
    }
    this.draw(); return true;
  };

  D.keyForm = function (k) {
    var f = this.S.form, keys = f.kind === "webapp" ? ["name", "url", "icon"] : ["name", "command", "icon", "style"], field = keys[f.focus];
    if (k === "esc") this.S.form = null;
    else if (k === "tab" || k === "down") f.focus = (f.focus + 1) % keys.length;
    else if (k === "backtab" || k === "up") f.focus = (f.focus + keys.length - 1) % keys.length;
    else if (field === "style" && (k === "left" || k === "right" || k === "h" || k === "l")) f.style = f.style === "float" ? "tile" : "float";
    else if (k === "enter") {
      if (!f.name.trim()) { f.error = "name is required"; }
      else if (f.kind === "webapp" && !/^https?:\/\/\S+\.\S+/.test(f.url)) { f.error = "url must start with http:// or https://"; }
      else if (f.kind === "tui" && !f.command.trim()) { f.error = "command is required"; }
      else {
        if (f.editing) window.PP.removeApp(f.editing.kind, f.editing.name);
        var app = f.kind === "webapp" ? { kind: "webapp", name: f.name.trim(), url: f.url.trim() } : { kind: "tui", name: f.name.trim(), command: f.command.trim(), style: f.style };
        var path = window.PP.addApp(app);
        this.S.form = null;
        var list = this.appList(); this.S.apps.cursor = Math.max(0, list.findIndex(function (a) { return a.name === app.name && a.kind === app.kind; }));
        this.flash((f.editing ? "saved " : "created ") + path);
        return true;
      }
    }
    else if (field !== "style" && k === "backspace") f[field] = f[field].slice(0, -1);
    else if (field !== "style" && k === "space") f[field] += " ";
    else if (field !== "style" && k.length === 1) f[field] += k;
    this.draw(); return true;
  };

  // is there anything an Escape press would cancel inside the demo?
  D.canEscape = function () {
    var S = this.S;
    return !!(S.modal || S.form || S.help || (S.run && S.run.status !== "running") || (S.tab === 1 && S.search.q));
  };

  D.click = function (e) {
    var S = this.S;
    if (S.screen !== "main" || S.modal || S.form || S.help || S.run) return;
    var rect = this.pre.getBoundingClientRect(), cw = rect.width / W, lh = rect.height / H;
    var col = Math.floor((e.clientX - rect.left) / cw), row = Math.floor((e.clientY - rect.top) / lh);
    if (row === 1 && this.hitTabs) {
      for (var i = 0; i < this.hitTabs.length; i++) if (col >= this.hitTabs[i][0] && col < this.hitTabs[i][1]) { this.setTab(this.hitTabs[i][2]); this.emit("onKey", { key: String(i + 1), auto: false }); return; }
    }
    if (S.tab === 0 && row === 3) { S.inst.filter = (S.inst.filter + 1) % FILTERS.length; S.inst.cursor = 0; this.draw(); return; }
    if ((S.tab === 0 || S.tab === 1) && this.hitList && row >= this.hitList.y0 && col <= LIST) {
      var idx = this.hitList.top + row - this.hitList.y0;
      if (idx < this.hitList.n) { (S.tab === 0 ? S.inst : S.search).cursor = idx; this.draw(); }
    }
    if (S.tab === 2 && this.hitApps) this.hitApps.forEach(function (h) { if (h[0] === row && col <= LIST) { S.apps.cursor = h[1]; } });
    this.draw();
  };

  D.reset = function () {
    this.gen++;
    window.PP.reset();
    var keep = this.S.tick;
    this.S = freshState(); this.S.tick = keep;
    this.startSplash();
  };

  // ---------------------------------------------------------------- autoplay
  var SCRIPT = [
    ["wait", 1300],
    ["l", 650], ["l", 500], ["l", 500],
    ["d", 1100],
    ["enter", 2300],
    ["until", "password"], ["type", "********", 90], ["enter", 300],
    ["until", "yn"], ["enter", 800],
    ["until", "end"], ["any", 1700],
    ["h", 1400], ["h", 350], ["h", 350],
    ["j", 450], ["j", 320], ["j", 320],
    ["d", 900],
    ["type", "REMO", 300, 1500], ["wait", 1700], ["esc", 0],
    ["/", 1300], ["type", "zellij", 130, 700],
    ["enter", 1100], ["enter", 2000],
    ["until", "password"], ["type", "********", 90], ["enter", 300],
    ["until", "yn"], ["enter", 800],
    ["until", "end"], ["any", 1700],
    ["tab", 1200],
    ["a", 1300], ["type", "GitHub", 120, 500], ["tab", 500], ["type", "https://github.com", 70, 300], ["enter", 900],
    ["wait", 2600],
    ["d", 1500], ["wait", 1300], ["esc", 0],
    ["wait", 1200], ["F1", 0], ["wait", 2800], ["F1", 0],
    ["wait", 1500]
  ];

  D.setAutoplay = function (on) {
    if (on === this.auto) return;
    this.auto = on;
    this.emit("onAutoplay", on);
    if (on) this.playLoop(++this.autoGen);
    else this.autoGen++;
  };
  D.autoGen = 0;

  D.playLoop = async function (ag) {
    var self = this;
    var alive = function () { return self.auto && ag === self.autoGen; };
    var sleep = function (ms) {
      return new Promise(function (res) {
        var left = ms;
        var t = function () {
          if (!alive()) return res(false);
          if (self.paused) return setTimeout(t, 200);
          left -= 50; if (left <= 0) res(true); else setTimeout(t, 50);
        };
        t();
      });
    };
    var until = function (what) {
      return new Promise(function (res) {
        var t = function () {
          if (!alive()) return res(false);
          var r = self.S.run;
          if (what === "end" ? (r && r.status !== "running") : (r && r.wait && r.wait.type === what)) return res(true);
          setTimeout(t, 80);
        };
        t();
      });
    };
    while (alive()) {
      this.reset();
      if (!(await sleep(1600))) return;
      for (var i = 0; i < SCRIPT.length; i++) {
        var s = SCRIPT[i];
        if (s[0] === "wait") { if (!(await sleep(s[1]))) return; continue; }
        if (s[0] === "until") { if (!(await until(s[1]))) return; if (!(await sleep(500))) return; continue; }
        if (s[0] === "type") {
          if (!(await sleep(s[3] || 300))) return;
          var chars = Array.from(s[1]);
          for (var c = 0; c < chars.length; c++) { if (!(await sleep(s[2]))) return; this.key(chars[c], true); }
          continue;
        }
        if (!(await sleep(s[1]))) return;
        this.key(s[0] === "any" ? "enter" : s[0], true);
      }
    }
  };

  window.PPDemo = { mount: function (el, opts) { return new Demo(el, opts); }, W: W, H: H };
})();
