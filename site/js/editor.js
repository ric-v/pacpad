// Full-screen programs for the web shell: nano, vim and less. They share
// one buffer model and one renderer, drawn on a character grid inside the
// shell panel, and each keeps its own real keys, status line and quirks.
(function () {
  "use strict";

  function esc(s) { return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;"); }
  function rep(s, n) { return n > 0 ? new Array(n + 1).join(s) : ""; }
  function padR(s, n) { return s.length >= n ? s.slice(0, n) : s + rep(" ", n - s.length); }

  // ------------------------------------------------------------- tiny highlighter
  var KW = /^(as|async|await|break|const|continue|crate|dyn|else|enum|extern|false|fn|for|if|impl|in|let|loop|match|mod|move|mut|pub|ref|return|self|Self|static|struct|super|trait|true|type|unsafe|use|where|while|Some|None|Ok|Err)$/;
  function lang(path) {
    if (/\.rs$/.test(path)) return "rust";
    if (/\.toml$|\.conf$|config$|\.desktop$|mirrorlist$|os-release$/.test(path)) return "conf";
    if (/\.md$/.test(path)) return "md";
    if (/\.(js|mjs)$/.test(path)) return "js";
    if (/\.css$/.test(path)) return "css";
    if (/\.(html|svg|xml)$/.test(path)) return "html";
    if (/\.ya?ml$/.test(path)) return "conf";
    if (/(bashrc|\.sh)$/.test(path)) return "sh";
    return "";
  }
  // returns one colour class per character of `s`
  function paint(s, lg) {
    var c = new Array(s.length).fill(""), i, m;
    if (!lg) return c;
    var fill = function (a, b, k) { for (var x = a; x < b && x < s.length; x++) c[x] = k; };
    if (lg === "md") {
      if (/^#{1,6} /.test(s)) fill(0, s.length, "h-k");
      else if ((m = /^(\s*[-*] )(\[[ x]\])?/.exec(s))) { fill(0, m[1].length, "h-p"); if (m[2]) fill(m[1].length, m[0].length, "h-n"); }
      var re = /`[^`]*`/g; while ((m = re.exec(s))) fill(m.index, m.index + m[0].length, "h-s");
      return c;
    }
    if (lg === "conf" || lg === "sh") {
      if (/^\s*#/.test(s)) { fill(0, s.length, "h-c"); return c; }
      if (/^\s*\[.*\]\s*$/.test(s)) { fill(0, s.length, "h-k"); return c; }
      if ((m = /^(\s*[\w.\-%$]+)\s*=/.exec(s))) fill(0, m[1].length, "h-n");
      var rs = /"[^"]*"|'[^']*'/g; while ((m = rs.exec(s))) fill(m.index, m.index + m[0].length, "h-s");
      return c;
    }
    if (lg === "html") {
      var rt = /<\/?[\w-]+|\/?>/g; while ((m = rt.exec(s))) fill(m.index, m.index + m[0].length, "h-k");
      var ra = /"[^"]*"/g; while ((m = ra.exec(s))) fill(m.index, m.index + m[0].length, "h-s");
      var rc = /<!--.*?(-->|$)/g; while ((m = rc.exec(s))) fill(m.index, m.index + m[0].length, "h-c");
      return c;
    }
    // rust / js / css: strings, comments, numbers, keywords, attributes
    i = 0;
    while (i < s.length) {
      var ch = s[i];
      if (ch === "/" && s[i + 1] === "/") { fill(i, s.length, "h-c"); break; }
      if (ch === "/" && s[i + 1] === "*") { var e = s.indexOf("*/", i + 2); e = e < 0 ? s.length : e + 2; fill(i, e, "h-c"); i = e; continue; }
      if (ch === '"' || (ch === "'" && lg !== "rust") || ch === "`") {
        var j = i + 1; while (j < s.length && s[j] !== ch) { if (s[j] === "\\") j++; j++; }
        fill(i, j + 1, "h-s"); i = j + 1; continue;
      }
      if (lg === "rust" && ch === "#" && s[i + 1] === "[") { var k = s.indexOf("]", i); k = k < 0 ? s.length : k + 1; fill(i, k, "h-p"); i = k; continue; }
      if (/[0-9]/.test(ch) && !/\w/.test(s[i - 1] || "")) { var n = i; while (n < s.length && /[\w.]/.test(s[n])) n++; fill(i, n, "h-n"); i = n; continue; }
      if (/[A-Za-z_]/.test(ch)) {
        var w = i; while (w < s.length && /\w/.test(s[w])) w++;
        var word = s.slice(i, w);
        if (lg === "rust" && s[w] === "!") fill(i, w + 1, "h-p");
        else if (KW.test(word) || (lg === "js" && /^(var|function|return|if|else|for|new|this|true|false|null|typeof)$/.test(word))) fill(i, w, "h-k");
        else if (/^[A-Z]/.test(word) && lg === "rust") fill(i, w, "h-t");
        i = w; continue;
      }
      i++;
    }
    return c;
  }

  // ------------------------------------------------------------- the program
  function Program(host, o) {
    this.o = o;
    this.mode = o.mode;                       // nano | vim | less
    this.lines = (o.text || "").replace(/\r\n/g, "\n").replace(/\t/g, "    ").split("\n");
    if (this.lines.length > 1 && this.lines[this.lines.length - 1] === "" && /\n$/.test(o.text || "")) this.lines.pop();
    this.trailingNL = /\n$/.test(o.text || "") || !o.text;
    this.r = 0; this.c = 0; this.want = 0; this.top = 0;
    this.dirty = false; this.lang = lang(o.path);
    this.vmode = "normal"; this.pending = ""; this.count = ""; this.undo = []; this.cut = null; this.cutChain = false;
    this.prompt = null; this.msg = ""; this.msgCls = ""; this.escs = 0; this.nu = false; this.search = "";

    var el = this.el = document.createElement("div");
    el.className = "ed ed-" + this.mode;
    el.innerHTML = '<pre class="ed-scr" aria-hidden="true"></pre><textarea class="ed-in" autocapitalize="off" autocomplete="off" autocorrect="off" spellcheck="false" aria-label="' + this.mode + ' editor"></textarea><button type="button" class="ed-x">✕ ' + (this.mode === "vim" ? ":q" : this.mode === "nano" ? "^X exit" : "q") + "</button>";
    host.appendChild(el);
    this.scr = el.querySelector(".ed-scr");
    this.inp = el.querySelector(".ed-in");
    var self = this;
    this.inp.addEventListener("keydown", function (e) { self.onKey(e); });
    this.inp.addEventListener("input", function () { var v = self.inp.value; self.inp.value = ""; if (v) self.typeText(v); });
    this.inp.addEventListener("paste", function (e) { e.preventDefault(); self.typeText((e.clipboardData || window.clipboardData).getData("text")); });
    this.scr.addEventListener("mousedown", function (e) { e.preventDefault(); self.onClick(e); self.focus(); });
    this.scr.addEventListener("wheel", function (e) { e.preventDefault(); self.scrollBy(e.deltaY > 0 ? 3 : -3); }, { passive: false });
    el.querySelector(".ed-x").addEventListener("click", function () { self.quitButton(); });
    this.ro = new ResizeObserver(function () { self.measure(); self.draw(); });
    this.ro.observe(el);
    this.measure();
    if (this.mode === "nano") this.say(o.isNew ? "[ New File ]" : !o.writable ? "[ File '" + o.display + "' is unwritable ]" : "[ Read " + this.lines.length + " line" + (this.lines.length === 1 ? "" : "s") + " ]", !o.writable && !o.isNew ? "warn" : "");
    if (this.mode === "vim") this.say(o.isNew ? '"' + o.display + '" [New]' : '"' + o.display + '"' + (o.writable ? "" : " [readonly]") + " " + this.lines.length + "L, " + (o.text || "").length + "B");
    if (this.mode === "less") this.say(o.display, "inv");
    this.draw();
    this.focus();
  }
  var P = Program.prototype;

  P.focus = function () { this.inp.focus({ preventScroll: true }); };
  P.measure = function () {
    var probe = document.createElement("span");
    probe.textContent = "MMMMMMMMMM";
    this.scr.appendChild(probe);
    var r = probe.getBoundingClientRect();
    var cs = getComputedStyle(this.scr);
    this.cw = r.width / 10 || 8; this.lh = parseFloat(cs.lineHeight) || r.height || 18;
    probe.remove();
    var w = this.scr.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
    var h = this.scr.clientHeight - parseFloat(cs.paddingTop) - parseFloat(cs.paddingBottom);
    this.cols = Math.max(20, Math.floor((w - 1) / this.cw));
    this.rows = Math.max(8, Math.floor((h - 1) / this.lh));
  };
  P.textRows = function () { return this.mode === "nano" ? this.rows - 4 : this.rows - 1; };
  P.gutter = function () { return this.nu ? Math.max(3, String(this.lines.length).length) + 1 : 0; };
  P.say = function (m, cls) { this.msg = m; this.msgCls = cls || ""; };

  // ------------------------------------------------------------- drawing
  P.draw = function () {
    var R = this.rows, C = this.cols, out = [], tr = this.textRows(), g = this.gutter(), w = C - g;
    this.clamp();
    if (this.r < this.top) this.top = this.r;
    if (this.r >= this.top + tr) this.top = this.r - tr + 1;
    if (this.mode === "nano") {
      var title = "  GNU nano 8.2", name = this.o.display || "New Buffer", right = this.dirty ? "Modified  " : "";
      var mid = Math.max(title.length + 2, Math.floor((C - name.length) / 2));
      var bar = padR(padR(title, mid) + name, C - right.length) + right;
      out.push('<span class="inv">' + esc(padR(bar, C)) + "</span>");
    }
    for (var i = 0; i < tr; i++) {
      var ln = this.top + i, s = this.lines[ln];
      if (s === undefined) { out.push(this.mode === "vim" ? '<span class="h-v">~</span>' : this.mode === "less" && ln === this.lines.length ? "" : ""); continue; }
      var off = 0;
      if (ln === this.r && this.c >= w - 1) off = this.c - (w - 1) + Math.min(8, Math.floor(w / 4));
      var trunc = this.mode === "nano" && s.length > off + w, vw = trunc ? w - 1 : w;
      var cls = paint(s, this.lang), view = s.slice(off, off + vw), vcls = cls.slice(off, off + vw), html = "";
      if (g) html += '<span class="h-v">' + padR(String(ln + 1).padStart(g - 1) + " ", g) + "</span>";
      if (this.mode === "less" && this.search) {
        var q = this.search.toLowerCase(), low = view.toLowerCase(), pos = low.indexOf(q);
        while (q && pos >= 0) { for (var z = pos; z < pos + q.length; z++) vcls[z] = "hl"; pos = low.indexOf(q, pos + q.length); }
      }
      var cur = this.mode !== "less" && ln === this.r && !this.prompt ? this.c - off : -1;
      var run = "", rk = null;
      var flush = function () { if (run) { html += rk ? '<span class="' + rk + '">' + esc(run) + "</span>" : esc(run); run = ""; } };
      for (var x = 0; x < Math.max(view.length, cur + 1); x++) {
        var ch = view[x] === undefined ? " " : view[x], k = x === cur ? "cur" : (vcls[x] || "");
        if (k !== rk) { flush(); rk = k; }
        run += ch;
      }
      flush();
      if (trunc) html += '<span class="h-v">&gt;</span>';
      out.push(html);
    }
    out.push(this.statusLine());
    if (this.mode === "nano") this.nanoBars().forEach(function (b) { out.push(b); });
    this.scr.innerHTML = out.join("\n");
  };

  P.statusLine = function () {
    var C = this.cols;
    if (this.prompt) {
      var p = this.prompt, txt = p.label + p.val;
      if (this.mode === "nano") return '<span class="inv">' + esc(padR(txt, C)).replace(/ $/, "") + '<span class="cur"> </span></span>';
      return esc(txt) + '<span class="cur"> </span>';
    }
    if (this.mode === "nano") {
      if (!this.msg) return "";
      var m = this.msg, pad = Math.max(0, Math.floor((C - m.length) / 2));
      return rep(" ", pad) + '<span class="' + (this.msgCls === "warn" ? "inv-r" : "inv") + '">' + esc(m) + "</span>";
    }
    if (this.mode === "vim") {
      var tr = this.textRows(), pct = this.lines.length <= tr ? "All" : this.top === 0 ? "Top" : this.top + tr >= this.lines.length ? "Bot" : Math.round((this.top / (this.lines.length - tr)) * 100) + "%";
      var ruler = padR((this.r + 1) + "," + (this.c + 1), 14) + pct;
      var plain = this.vmode === "insert" ? "-- INSERT --" : this.msg;
      // like vim, a message too long for the space left of the ruler hides the ruler
      if (plain.length > C - ruler.length - 2) { ruler = ""; if (plain.length > C - 1) plain = plain.slice(0, C - 2) + ">"; }
      var left = this.vmode === "insert" ? '<span class="B">-- INSERT --</span>' : this.msgCls === "err" ? '<span class="inv-r">' + esc(plain) + "</span>" : esc(plain);
      var gap = Math.max(1, C - plain.length - ruler.length - 1);
      return left + rep(" ", gap) + esc(ruler);
    }
    // less
    var end = this.top + this.textRows() >= this.lines.length;
    if (this.msgCls === "inv") return '<span class="inv">' + esc(this.msg) + "</span>" + (end ? ' <span class="inv">(END)</span>' : "") + '<span class="cur"> </span>';
    if (this.msg) return '<span class="inv">' + esc(this.msg) + '</span><span class="cur"> </span>';
    return (end ? '<span class="inv">(END)</span>' : ":") + '<span class="cur"> </span>';
  };

  P.nanoBars = function () {
    var C = this.cols, items;
    if (this.prompt && this.prompt.kind === "exit") items = [[["Y", "Yes"], ["N", "No"]], [["^C", "Cancel"]]];
    else if (this.prompt) items = [[["^G", "Help"], ["^C", "Cancel"]], [["", ""], ["", ""]]];
    else items = [[["^G", "Help"], ["^O", "Write Out"], ["^W", "Where Is"], ["^K", "Cut"], ["^T", "Execute"], ["^C", "Location"]], [["^X", "Exit"], ["^R", "Read File"], ["^\\", "Replace"], ["^U", "Paste"], ["^J", "Justify"], ["^/", "Go To Line"]]];
    return items.map(function (row) {
      var n = Math.max(2, Math.min(row.length, Math.floor(C / 16))), w = Math.floor(C / n), s = "";
      row.slice(0, n).forEach(function (it) {
        if (!it[0]) { s += rep(" ", w); return; }
        s += '<span class="inv nk" data-k="' + esc(it[0]) + '">' + esc(it[0]) + "</span> " + esc(padR(it[1], w - it[0].length - 1));
      });
      return s;
    });
  };

  // ------------------------------------------------------------- buffer ops
  P.clamp = function () {
    if (!this.lines.length) this.lines = [""];
    this.r = Math.max(0, Math.min(this.r, this.lines.length - 1));
    var max = this.lines[this.r].length - (this.mode === "vim" && this.vmode === "normal" && this.lines[this.r].length ? 1 : 0);
    this.c = Math.max(0, Math.min(this.c, max));
    var maxTop = Math.max(0, this.lines.length - (this.mode === "less" ? this.textRows() : 1));
    this.top = Math.max(0, Math.min(this.top, maxTop));
  };
  P.snap = function () { this.undo.push({ l: this.lines.slice(), r: this.r, c: this.c }); if (this.undo.length > 200) this.undo.shift(); };
  P.touch = function () { this.dirty = true; };
  P.insert = function (s) {
    var l = this.lines[this.r];
    this.lines[this.r] = l.slice(0, this.c) + s + l.slice(this.c);
    this.c += s.length; this.want = this.c; this.touch();
  };
  P.newline = function () {
    var l = this.lines[this.r], ind = this.mode === "vim" ? (/^\s*/.exec(l)[0]) : "";
    this.lines.splice(this.r + 1, 0, ind + l.slice(this.c));
    this.lines[this.r] = l.slice(0, this.c);
    this.r++; this.c = ind.length; this.want = this.c; this.touch();
  };
  P.backspace = function () {
    if (this.c > 0) { var l = this.lines[this.r]; this.lines[this.r] = l.slice(0, this.c - 1) + l.slice(this.c); this.c--; }
    else if (this.r > 0) { var prev = this.lines[this.r - 1]; this.lines[this.r - 1] = prev + this.lines[this.r]; this.lines.splice(this.r, 1); this.r--; this.c = prev.length; }
    else return;
    this.want = this.c; this.touch();
  };
  P.del = function () {
    var l = this.lines[this.r];
    if (this.c < l.length) this.lines[this.r] = l.slice(0, this.c) + l.slice(this.c + 1);
    else if (this.r < this.lines.length - 1) { this.lines[this.r] = l + this.lines[this.r + 1]; this.lines.splice(this.r + 1, 1); }
    else return;
    this.touch();
  };
  P.move = function (dr, dc) {
    if (dr) { this.r += dr; this.clamp(); this.c = Math.min(this.want, this.lines[this.r].length); }
    if (dc) {
      this.c += dc;
      if (this.mode === "nano") {
        if (this.c < 0 && this.r > 0) { this.r--; this.c = this.lines[this.r].length; }
        else if (this.c > this.lines[this.r].length && this.r < this.lines.length - 1) { this.r++; this.c = 0; }
      }
      this.clamp(); this.want = this.c;
    }
  };
  P.scrollBy = function (n) {
    this.top += n;
    var tr = this.textRows();
    this.top = Math.max(0, Math.min(this.top, Math.max(0, this.lines.length - (this.mode === "less" ? tr : 1))));
    if (this.mode !== "less") { if (this.r < this.top) this.r = this.top; if (this.r >= this.top + tr) this.r = this.top + tr - 1; this.clamp(); }
    this.draw();
  };
  P.page = function (n) {
    var tr = this.textRows();
    if (this.mode === "less") { this.scrollBy(n * tr); return; }
    this.r += n * tr; this.top += n * tr; this.clamp(); this.c = Math.min(this.want, this.lines[this.r].length);
  };
  P.find = function (q, from, dir) {
    if (!q) return false;
    var lq = q.toLowerCase(), n = this.lines.length;
    for (var k = 1; k <= n; k++) {
      var i = ((from + dir * k) % n + n) % n, p = this.lines[i].toLowerCase().indexOf(lq);
      if (p >= 0) {
        if (this.mode === "less") { this.top = i; this.clamp(); } else { this.r = i; this.c = p; this.want = p; }
        if (dir > 0 && i <= from && k > 0) this.say(this.mode === "vim" ? "search hit BOTTOM, continuing at TOP" : this.mode === "nano" ? "[ Search Wrapped ]" : "", this.mode === "vim" ? "err" : "");
        return true;
      }
    }
    this.say(this.mode === "vim" ? "E486: Pattern not found: " + q : this.mode === "nano" ? '[ "' + q + '" not found ]' : "Pattern not found  (press RETURN)", this.mode === "vim" ? "err" : "");
    return false;
  };
  P.text = function () { return this.lines.join("\n") + (this.trailingNL ? "\n" : ""); };
  P.save = function (path) {
    var err = this.o.save(this.text(), path);
    if (err) return err;
    this.dirty = false;
    if (path) { this.o.display = path; this.o.path = path; }
    return null;
  };
  P.close = function () {
    this.ro.disconnect();
    this.el.remove();
    if (this.o.onExit) this.o.onExit();
  };
  P.onClick = function (e) {
    if (this.mode === "less") return;
    var b = this.scr.getBoundingClientRect(), row = Math.floor((e.clientY - b.top) / this.lh), col = Math.floor((e.clientX - b.left) / this.cw);
    var k = e.target.closest && e.target.closest(".nk");
    if (k) { this.nanoShortcut(k.dataset.k); return; }
    var tRow = row - (this.mode === "nano" ? 1 : 0);
    if (tRow < 0 || tRow >= this.textRows()) return;
    this.r = this.top + tRow; this.c = Math.max(0, col - this.gutter()); this.clamp(); this.want = this.c; this.draw();
  };
  P.quitButton = function () {
    if (this.mode === "nano") this.nanoShortcut("^X");
    else if (this.mode === "vim") { this.vmode = "normal"; this.prompt = { kind: "ex", label: ":", val: "q" }; this.draw(); this.focus(); }
    else this.close();
  };

  // ------------------------------------------------------------- keys
  P.onKey = function (e) {
    e.stopPropagation();
    var k = e.key;
    if (k === "Process" || k === "Unidentified") return;      // let the input event carry it
    var ctrl = e.ctrlKey || e.metaKey;
    if (ctrl && (k === "v" || k === "V")) return;               // native paste
    if (ctrl && (k === "c" || k === "C") && String(window.getSelection())) return;
    var handled = this.prompt ? this.promptKey(k, e) : this.mode === "nano" ? this.nanoKey(k, e) : this.mode === "vim" ? this.vimKey(k, e) : this.lessKey(k, e);
    if (handled !== false) { e.preventDefault(); if (this.el.isConnected) this.draw(); }
  };
  P.typeText = function (t) {
    if (this.prompt) { this.prompt.val += t.replace(/\n/g, ""); this.draw(); return; }
    if (this.mode === "less" || (this.mode === "vim" && this.vmode !== "insert")) { var self = this; Array.from(t).forEach(function (ch) { self.onKey({ key: ch, preventDefault: function () {}, stopPropagation: function () {} }); }); return; }
    var parts = t.replace(/\t/g, "    ").split("\n"), first = true, s2 = this;
    if (this.mode === "nano") this.snap();
    parts.forEach(function (p) { if (!first) s2.newline(); s2.insert(p); first = false; });
    this.draw();
  };

  P.promptKey = function (k, e) {
    var p = this.prompt, ctrl = e.ctrlKey || e.metaKey;
    if (p.kind === "exit") {
      if (k === "y" || k === "Y") { this.prompt = { kind: "write", label: "File Name to Write: ", val: this.o.display || "", then: "exit" }; return; }
      if (k === "n" || k === "N") { this.prompt = null; this.close(); return; }
      if ((ctrl && (k === "c" || k === "C")) || k === "Escape") { this.prompt = null; this.say("[ Cancelled ]"); return; }
      return;
    }
    if (k === "Escape" || (ctrl && (k === "c" || k === "C"))) { this.prompt = null; this.say(this.mode === "nano" ? "[ Cancelled ]" : ""); this.vmode = "normal"; return; }
    if (k === "Backspace") { if (!p.val && this.mode !== "nano") { this.prompt = null; return; } p.val = p.val.slice(0, -1); return; }
    if (k === "Enter") { this.prompt = null; this.runPrompt(p); return; }
    if (k.length === 1 && !ctrl) { p.val += k; return; }
    if (k === "ArrowUp" && p.kind === "search" && this.search) { p.val = this.search; return; }
  };
  P.runPrompt = function (p) {
    if (p.kind === "write") {
      var err = this.save(p.val.trim());
      if (err) { this.say(err, "warn"); return; }
      this.say("[ Wrote " + this.lines.length + " line" + (this.lines.length === 1 ? "" : "s") + " ]");
      if (p.then === "exit") this.close();
      return;
    }
    if (p.kind === "search") {
      var q = p.val || this.search; if (!q) return;
      this.search = q; this.msg = "";
      this.find(q, this.mode === "less" ? this.top : this.r, p.dir || 1);
      return;
    }
    if (p.kind === "goto") { var n = parseInt(p.val, 10); if (n > 0) { this.r = n - 1; this.c = 0; this.want = 0; } return; }
    if (p.kind === "ex") this.ex(p.val.trim());
  };

  // nano
  P.nanoShortcut = function (s) {
    var map = { "^X": "x", "^O": "o", "^W": "w", "^K": "k", "^U": "u", "^C": "c", "^G": "g", "^/": "/", "^T": "t", "^R": "r", "^\\": "\\", "^J": "j" };
    if (this.prompt && this.prompt.kind === "exit") { this.promptKey(s === "^C" ? "c" : s.toLowerCase(), { ctrlKey: s === "^C" }); this.draw(); return; }
    if (map[s]) this.nanoKey(map[s], { ctrlKey: true, shiftKey: false });
    if (this.el.isConnected) { this.draw(); this.focus(); }
  };
  P.nanoKey = function (k, e) {
    var ctrl = e.ctrlKey || e.metaKey, l = k.toLowerCase();
    if (ctrl || /^F\d+$/.test(k)) {
      var cut = false;
      if (l === "x" || k === "F2") {
        if (this.dirty) { this.prompt = { kind: "exit", label: "Save modified buffer?  ", val: "" }; return; }
        this.close(); return;
      }
      if (l === "o" || k === "F3") { this.prompt = { kind: "write", label: "File Name to Write: ", val: this.o.display || "" }; return; }
      if (l === "w" || k === "F6") { this.prompt = { kind: "search", label: "Search" + (this.search ? " [" + this.search + "]" : "") + ": ", val: "" }; return; }
      if (l === "k" || k === "F9") {
        this.snap(); cut = true;
        var line = this.lines.splice(this.r, 1)[0];
        this.cut = this.cutChain && this.cut ? this.cut.concat([line]) : [line];
        if (!this.lines.length) this.lines = [""];
        this.c = 0; this.touch();
      }
      else if (l === "u" || k === "F10") { if (this.cut) { this.snap(); var self = this; this.cut.forEach(function (x) { self.lines.splice(self.r, 0, x); self.r++; }); this.c = 0; this.touch(); } }
      else if (l === "c" || k === "F11") {
        var tot = this.lines.join("\n").length, before = this.lines.slice(0, this.r).join("\n").length + (this.r ? 1 : 0) + this.c;
        var pc = function (a, b) { return Math.round((a / Math.max(1, b)) * 100) + "%"; };
        this.say("[ line " + (this.r + 1) + "/" + this.lines.length + " (" + pc(this.r + 1, this.lines.length) + "), col " + (this.c + 1) + "/" + (this.lines[this.r].length + 1) + " (" + pc(this.c + 1, this.lines[this.r].length + 1) + "), char " + (before + 1) + "/" + (tot + 1) + " (" + pc(before + 1, tot + 1) + ") ]");
      }
      else if (l === "g" || k === "F1") this.say("^O save · ^X exit · ^W or F6 search (your browser keeps ^W) · ^K cut · ^U paste");
      else if (l === "/" || l === "_") this.prompt = { kind: "goto", label: "Enter line number, column number: ", val: "" };
      else if (l === "a" || l === "e") { this.c = l === "a" ? 0 : this.lines[this.r].length; }
      else if (l === "t" || l === "r" || l === "\\" || l === "j") this.say("[ That's not available in this nano: it lives in a web page ]");
      else if (l === "z") { var u = this.undo.pop(); if (u) { this.lines = u.l; this.r = u.r; this.c = u.c; } }
      else return false;
      this.cutChain = cut;
      return;
    }
    if (e.altKey) {
      if (l === "u") { var u2 = this.undo.pop(); if (u2) { this.lines = u2.l; this.r = u2.r; this.c = u2.c; this.touch(); } return; }
      if (l === "w") { if (this.search) this.find(this.search, this.r, 1); return; }
      return false;
    }
    this.cutChain = false; this.msg = "";
    switch (k) {
      case "ArrowUp": this.move(-1, 0); return;
      case "ArrowDown": this.move(1, 0); return;
      case "ArrowLeft": this.move(0, -1); return;
      case "ArrowRight": this.move(0, 1); return;
      case "Home": this.c = 0; this.want = 0; return;
      case "End": this.c = this.lines[this.r].length; this.want = this.c; return;
      case "PageUp": this.page(-1); return;
      case "PageDown": this.page(1); return;
      case "Enter": this.snap(); this.newline(); return;
      case "Backspace": this.snap(); this.backspace(); return;
      case "Delete": this.snap(); this.del(); return;
      case "Tab": this.snap(); this.insert("    "); return;
      case "Escape": return;
    }
    if (k.length === 1) { this.snap(); this.insert(k); return; }
    return false;
  };

  // vim
  P.vimKey = function (k, e) {
    var ctrl = e.ctrlKey || e.metaKey;
    if (this.vmode === "insert") {
      if (k === "Escape" || (ctrl && k === "[")) { this.vmode = "normal"; this.c = Math.max(0, this.c - 1); this.msg = ""; return; }
      switch (k) {
        case "ArrowUp": this.move(-1, 0); return; case "ArrowDown": this.move(1, 0); return;
        case "ArrowLeft": this.move(0, -1); return; case "ArrowRight": this.c++; this.clamp(); return;
        case "Enter": this.newline(); return; case "Backspace": this.backspace(); return; case "Delete": this.del(); return;
        case "Tab": this.insert("    "); return;
      }
      if (k.length === 1 && !ctrl) { this.insert(k); return; }
      return false;
    }
    // normal mode
    if (k === "Escape") {
      this.pending = ""; this.count = "";
      if (++this.escs >= 3) { this.say("type :q and press Enter to exit vim (:q! to throw away changes)"); this.escs = 0; }
      return;
    }
    this.escs = 0;
    if (ctrl) {
      var half = Math.floor(this.textRows() / 2);
      if (k === "d") { this.r += half; this.top += half; }
      else if (k === "u") { this.r -= half; this.top -= half; }
      else if (k === "f") this.page(1);
      else if (k === "b") this.page(-1);
      else if (k === "r") this.say("Already at newest change");
      else if (k === "g") this.say('"' + this.o.display + '"' + (this.dirty ? " [Modified]" : "") + " " + this.lines.length + " lines --" + Math.round(((this.r + 1) / this.lines.length) * 100) + "%--");
      else return false;
      this.clamp(); return;
    }
    if (/^[1-9]$/.test(k) || (k === "0" && this.count)) { this.count += k; return; }
    var n = parseInt(this.count || "1", 10), had = !!this.count; this.count = "";
    var pend = this.pending; this.pending = "";
    if (pend === "g") { if (k === "g") { this.r = had ? n - 1 : 0; this.c = 0; } return; }
    if (pend === "d") { if (k === "d") { this.snap(); this.cut = this.lines.splice(this.r, n); if (!this.lines.length) this.lines = [""]; this.touch(); this.c = 0; } return; }
    if (pend === "y") { if (k === "y") { this.cut = this.lines.slice(this.r, this.r + n); this.say(n > 1 ? n + " lines yanked" : ""); } return; }
    if (pend === "Z") { if (k === "Z") this.ex("x"); else if (k === "Q") this.ex("q!"); return; }
    var l = this.lines[this.r];
    switch (k) {
      case "h": case "ArrowLeft": case "Backspace": this.move(0, -n); return;
      case "l": case "ArrowRight": case " ": this.move(0, n); return;
      case "j": case "ArrowDown": case "Enter": this.move(n, 0); return;
      case "k": case "ArrowUp": this.move(-n, 0); return;
      case "0": case "Home": this.c = 0; this.want = 0; return;
      case "^": this.c = /^\s*/.exec(l)[0].length; this.want = this.c; return;
      case "$": case "End": this.c = Math.max(0, l.length - 1); this.want = 1e9; return;
      case "w": {
        var i = this.c, isW = function (ch) { return /\w/.test(ch); };
        if (i < l.length && /\S/.test(l[i])) { var t = isW(l[i]); while (i < l.length && /\S/.test(l[i]) && isW(l[i]) === t) i++; }
        while (i < l.length && /\s/.test(l[i])) i++;
        if (i >= l.length && this.r < this.lines.length - 1) { this.r++; i = /^\s*/.exec(this.lines[this.r])[0].length; }
        this.c = i; this.want = i; return;
      }
      case "b": { var s = l.slice(0, this.c), y = /(\w+|[^\w\s]+)\s*$/.exec(s); if (y) this.c = y.index; else if (this.r > 0) { this.r--; this.c = Math.max(0, this.lines[this.r].length - 1); } this.want = this.c; return; }
      case "e": { var r2 = /\s*(\w+|[^\w\s]+)/g; r2.lastIndex = this.c + 1; var z = r2.exec(l); if (z) this.c = r2.lastIndex - 1; this.want = this.c; return; }
      case "G": this.r = had ? n - 1 : this.lines.length - 1; this.c = 0; return;
      case "g": case "d": case "y": case "Z": this.pending = k; if (had) this.count = String(n); return;
      case "x": case "Delete": if (l.length) { this.snap(); this.lines[this.r] = l.slice(0, this.c) + l.slice(this.c + n); this.touch(); } return;
      case "p": if (this.cut) { this.snap(); var a = [this.r + 1, 0].concat(this.cut); Array.prototype.splice.apply(this.lines, a); this.r++; this.c = 0; this.touch(); } return;
      case "P": if (this.cut) { this.snap(); var b2 = [this.r, 0].concat(this.cut); Array.prototype.splice.apply(this.lines, b2); this.c = 0; this.touch(); } return;
      case "u": { var u = this.undo.pop(); if (u) { this.lines = u.l; this.r = u.r; this.c = u.c; this.say("1 change; before #" + (this.undo.length + 1)); } else this.say("Already at oldest change"); return; }
      case "i": this.enterInsert(); return;
      case "a": this.enterInsert(); if (l.length) this.c++; return;
      case "I": this.enterInsert(); this.c = /^\s*/.exec(l)[0].length; return;
      case "A": this.enterInsert(); this.c = l.length; return;
      case "o": this.enterInsert(); this.c = l.length; this.newline(); return;
      case "O": this.enterInsert(); this.lines.splice(this.r, 0, /^\s*/.exec(l)[0]); this.c = this.lines[this.r].length; this.touch(); return;
      case ":": this.prompt = { kind: "ex", label: ":", val: "" }; return;
      case "/": this.prompt = { kind: "search", label: "/", val: "", dir: 1 }; return;
      case "?": this.prompt = { kind: "search", label: "?", val: "", dir: -1 }; return;
      case "n": if (this.search) this.find(this.search, this.r, 1); return;
      case "N": if (this.search) this.find(this.search, this.r, -1); return;
      case "PageDown": this.page(1); return;
      case "PageUp": this.page(-1); return;
    }
    if (k.length === 1) return;
    return false;
  };
  P.enterInsert = function () { if (this.o.writable === false) this.say("W10: Warning: Changing a readonly file", "err"); else this.msg = ""; this.snap(); this.vmode = "insert"; };
  P.ex = function (cmd) {
    var bang = /!$/.test(cmd), c = cmd.replace(/!$/, "");
    if (/^\d+$/.test(c)) { this.r = parseInt(c, 10) - 1; this.c = 0; this.clamp(); return; }
    if (c === "set nu" || c === "set number") { this.nu = true; return; }
    if (c === "set nonu" || c === "set nonumber") { this.nu = false; return; }
    if (c === "w" || c === "wq" || c === "x" || /^w /.test(c) || /^wq? /.test(c)) {
      if (c === "x" && !this.dirty) { this.close(); return; }
      var target = c.split(" ")[1] || "";
      if (!this.o.writable && !target && !bang) { this.say("E45: 'readonly' option is set (add ! to override)", "err"); return; }
      var err = this.save(target || undefined);
      if (err) { this.say("E212: Can't open file for writing", "err"); return; }
      this.say('"' + this.o.display + '" ' + this.lines.length + "L, " + this.text().length + "B written");
      if (c !== "w" && !/^w /.test(c)) this.close();
      return;
    }
    if (c === "q" || c === "qa" || c === "quit") {
      if (this.dirty && !bang) { this.say("E37: No write since last change (add ! to override)", "err"); return; }
      this.close(); return;
    }
    if (c === "help" || c === "h") { this.say("this vim knows hjkl w b e 0 $ gg G dd yy p x u i a o / n :w :q :wq :q! :set nu"); return; }
    if (!c) return;
    this.say("E492: Not an editor command: " + cmd, "err");
  };

  // less
  P.lessKey = function (k, e) {
    var tr = this.textRows(), half = Math.floor(tr / 2);
    if (e.ctrlKey) { if (k === "d") this.scrollBy(half); else if (k === "u") this.scrollBy(-half); else if (k === "f") this.scrollBy(tr); else if (k === "b") this.scrollBy(-tr); else return false; this.msg = ""; return; }
    if (this.msg && this.msgCls !== "inv" && k === "Enter") { this.msg = ""; return; }
    this.msg = ""; this.msgCls = "";
    switch (k) {
      case "q": case "Q": case "Escape": this.close(); return;
      case "j": case "ArrowDown": case "Enter": case "e": this.scrollBy(1); return;
      case "k": case "ArrowUp": case "y": this.scrollBy(-1); return;
      case " ": case "f": case "PageDown": this.scrollBy(tr); return;
      case "b": case "PageUp": this.scrollBy(-tr); return;
      case "d": this.scrollBy(half); return;
      case "u": this.scrollBy(-half); return;
      case "g": case "<": case "Home": this.top = 0; return;
      case "G": case ">": case "End": this.top = this.lines.length; this.clamp(); return;
      case "/": this.prompt = { kind: "search", label: "/", val: "", dir: 1 }; return;
      case "?": this.prompt = { kind: "search", label: "?", val: "", dir: -1 }; return;
      case "n": if (this.search) this.find(this.search, this.top, 1); return;
      case "N": if (this.search) this.find(this.search, this.top, -1); return;
      case "h": case "H": this.say("j/k line · space/b page · d/u half · g/G ends · /pattern n N · q quit  (press RETURN)"); return;
    }
    if (k.length === 1) return;
    return false;
  };

  window.PPEditor = { open: function (host, opts) { return new Program(host, opts); } };
})();
