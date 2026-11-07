// The web shell in the Install section. It first types out the real build
// steps by itself, then hands the prompt over. ~/pacpad is the real
// ric-v/pacpad repo (see vfs.js): cd around it, read files with cat, less,
// nano or vim, grep it, run cargo test against its real test names, query
// the fake pacman database, and run pacpad's own CLI against the same
// in-memory system the live demo uses.
(function () {
  "use strict";

  var FS = window.PPFS, PP = window.PP;

  var CRATES = ["libc v0.2.189", "proc-macro2 v1.0.107", "unicode-ident v1.0.24", "quote v1.0.47", "cfg-if v1.0.4",
    "memchr v2.8.3", "log v0.4.34", "itoa v1.0.18", "serde_core v1.0.229", "bitflags v2.9.4", "syn v2.0.106",
    "serde_derive v1.0.229", "serde v1.0.229", "unicode-width v0.2.2", "regex-syntax v0.8.6", "regex-automata v0.4.11",
    "rustix v1.1.2", "mio v1.0.4", "signal-hook v0.3.18", "crossterm v0.29.0", "ring v0.17.14", "rustls v0.23.44",
    "nucleo-matcher v0.3.1", "vte v0.15.0", "vt100 v0.16.2", "portable-pty v0.9.0", "toml v1.1.6+spec-1.1.0",
    "bincode v2.0.1", "ureq v3.4.1", "clap_builder v4.6.6", "clap v4.6.6", "ratatui-core v0.1.2",
    "ratatui-widgets v0.3.2", "ratatui-crossterm v0.1.2", "ratatui v0.30.2", "anyhow v1.0.104"];

  var HELP = [
    "A terminal app manager for Arch", "",
    "<b>Usage:</b> pacpad [OPTIONS] [COMMAND]", "",
    "<b>Commands:</b>",
    "  <a>search</a>  Search installed + repo packages by name or description",
    "  <a>webapp</a>  Manage pacpad-created webapp launchers",
    "  <a>tui</a>     Manage pacpad-created TUI-app launchers",
    "  <a>help</a>    Print this message or the help of the given subcommand(s)", "",
    "<b>Options:</b>",
    "      <a>--pacman-conf</a> &lt;PACMAN_CONF&gt;  Path to pacman.conf, for testing against a fixture without root [default: /etc/pacman.conf]",
    "      <a>--dry-run</a>                    Print the command any write action would run, and run nothing",
    "  <a>-h</a>, <a>--help</a>                       Print help",
    "  <a>-V</a>, <a>--version</a>                    Print version"
  ].map(function (l) { return l.replace(/<a>/g, '<span class="c-a">').replace(/<\/a>/g, "</span>").replace(/<b>/g, '<span class="B">').replace(/<\/b>/g, "</span>"); });

  var GUIDE = [
    ["the repo", ""],
    ["ls -la · cd src · tree -L 2", "walk the real ric-v/pacpad tree"],
    ["cat Cargo.toml · less src/cli.rs", "read files, fetched live from GitHub"],
    ["nano README.md · vim src/main.rs", "edit them (saves stay in this tab)"],
    ["grep -rn REMOVE src · find . -name '*.rs'", "search it"],
    ["git log · git status · git diff", "after you edit something"],
    ["cargo test · cargo run", "real test names, then the demo"],
    ["pacpad", ""],
    ["pacpad search editor", "the headless search"],
    ["pacpad webapp add Notion https://notion.so", "lands in the demo's Apps tab"],
    ["pacpad --help · pacpad --dry-run …", ""],
    ["the box", ""],
    ["pacman -Qe · pacman -Qi glibc · pacman -Qdt", "queries against the demo's db"],
    ["cat ~/.config/pacpad/config.toml", ""],
    ["ls ~/.local/share/applications", "the launchers pacpad owns"],
    ["fastfetch · history · clear", ""]
  ];

  var COMMANDS = ["alias", "cargo", "cat", "cd", "clear", "cp", "date", "du", "echo", "env", "exit", "fastfetch", "file", "find", "gh", "git", "grep", "head", "help", "history", "hostname", "id", "less", "ls", "man", "mkdir", "more", "mv", "nano", "neofetch", "nvim", "open", "pacman", "pacpad", "paru", "printenv", "pwd", "rg", "rm", "rustc", "sort", "stat", "sudo", "tail", "touch", "tree", "true", "type", "uname", "uniq", "uptime", "vi", "vim", "wc", "which", "whoami", "xdg-open", "yay"];

  function esc(s) { return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;"); }
  function pad(s, n) { s = String(s); return s.length >= n ? s : s + new Array(n - s.length + 1).join(" "); }
  function lpad(s, n) { s = String(s); return s.length >= n ? s : new Array(n - s.length + 1).join(" ") + s; }
  function strip(h) { var d = document.createElement("div"); d.innerHTML = h; return d.textContent; }
  function human(n) { var u = ["", "K", "M", "G"], i = 0; while (n >= 1024 && i < 3) { n /= 1024; i++; } return i ? (n < 10 ? n.toFixed(1) : Math.round(n)) + u[i] : String(n); }
  var MON = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  function lsDate(iso) { var d = new Date(iso); return MON[d.getUTCMonth()] + " " + lpad(d.getUTCDate(), 2) + " " + lpad(d.getUTCHours(), 2).replace(" ", "0") + ":" + lpad(d.getUTCMinutes(), 2).replace(" ", "0"); }
  function glob(p) { return new RegExp("^" + p.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*").replace(/\?/g, ".") + "$"); }

  // split a command line into words, honouring "double" and 'single' quotes
  // unquoted * and ? are remembered so the word can be glob-expanded later
  function words(line) {
    var out = [], cur = "", q = null, has = false, wild = false;
    out.wild = [];
    function push() { if (cur || has) { if (wild) out.wild.push(out.length); out.push(cur); } cur = ""; has = false; wild = false; }
    for (var i = 0; i < line.length; i++) {
      var ch = line[i];
      if (q) { if (ch === q) q = null; else cur += ch; continue; }
      if (ch === "\\" && i + 1 < line.length) { cur += line[++i]; continue; }
      if (ch === '"' || ch === "'") { q = ch; has = true; continue; }
      if (/\s/.test(ch)) { push(); continue; }
      if (ch === "*" || ch === "?") wild = true;
      cur += ch;
    }
    push();
    return out;
  }
  // bash-style pathname expansion: no match leaves the word as typed
  function expandGlobs(w, cwd) {
    if (!w.wild || !w.wild.length) return w;
    var out = [];
    w.forEach(function (word, i) {
      if (i === 0 || w.wild.indexOf(i) < 0) { out.push(word); return; }
      var abs = word[0] === "/", tilde = word === "~" || word.indexOf("~/") === 0;
      var segs = (tilde ? word.slice(2) : word).split("/").filter(function (x, j, arr) { return x || j === arr.length - 1; });
      var base = abs ? "/" : tilde ? FS.HOME : cwd, found = [{ p: base, shown: abs ? "/" : tilde ? "~/" : "" }];
      segs.forEach(function (sg, j) {
        var next = [], last = j === segs.length - 1;
        found.forEach(function (f) {
          if (!/[*?]/.test(sg)) {
            var p = FS.resolve(f.p, sg);
            if (FS.stat(p)) next.push({ p: p, shown: f.shown + sg + (last ? "" : "/") });
            return;
          }
          var re = glob(sg);
          FS.list(f.p, sg[0] === ".").forEach(function (n) {
            if (re.test(n.name) && (last || n.type === "dir")) next.push({ p: n.path, shown: f.shown + n.name + (last ? "" : "/") });
          });
        });
        found = next;
      });
      if (!found.length) out.push(word);
      else found.map(function (f) { return f.shown; }).sort().forEach(function (x) { out.push(x); });
    });
    return out;
  }
  // split on an operator outside quotes
  function splitOp(line, ops) {
    var parts = [], cur = "", q = null;
    for (var i = 0; i < line.length; i++) {
      var ch = line[i];
      if (q) { if (ch === q) q = null; cur += ch; continue; }
      if (ch === '"' || ch === "'") { q = ch; cur += ch; continue; }
      var hit = null;
      ops.forEach(function (o) { if (!hit && line.substr(i, o.length) === o) hit = o; });
      if (hit) { parts.push({ text: cur, op: hit }); cur = ""; i += hit.length - 1; continue; }
      cur += ch;
    }
    parts.push({ text: cur, op: null });
    return parts;
  }
  function flags(args) {
    var f = {}, rest = [];
    args.forEach(function (a) {
      if (/^--[a-z]/.test(a)) f[a.slice(2)] = true;
      else if (/^-[a-zA-Z0-9]+$/.test(a)) a.slice(1).split("").forEach(function (c) { f[c] = true; });
      else rest.push(a);
    });
    return { f: f, rest: rest };
  }

  function Shell(el, opts) {
    this.el = el; this.opts = opts || {};
    this.out = el.querySelector(".sh-out");
    this.input = el.querySelector(".sh-in");
    this.line = el.querySelector(".sh-line");
    this.wrap = el.parentNode;
    this.hist = []; this.hi = 0;
    this.cwd = FS.HOME; this.oldpwd = FS.HOME;
    this.state = "idle";
    var self = this;
    this.input.addEventListener("keydown", function (e) { self.onKey(e); });
    el.addEventListener("click", function (e) {
      if (window.getSelection && String(window.getSelection())) return;
      if (e.target.closest("a")) return;
      if (self.state === "intro") self.skip();
      self.focus();
    });
    this.line.hidden = true;
    FS.init();
  }
  var S = Shell.prototype;

  S.ps1 = function () {
    return '<span class="c-g">' + PP.user + "@" + PP.host + '</span> <span class="c-a">' + esc(FS.pretty(this.cwd)) + '</span> <span class="c-d">$</span> ';
  };
  S.print = function (html, cls) {
    var d = document.createElement("div");
    d.className = "sh-row" + (cls ? " " + cls : "");
    d.innerHTML = html === "" ? "&nbsp;" : html;
    this.out.appendChild(d);
    this.scroll();
    return d;
  };
  S.scroll = function () { this.el.scrollTop = this.el.scrollHeight; };
  S.focus = function () { if (this.state === "ready" && !this.prog) this.input.focus({ preventScroll: true }); else if (this.prog) this.prog.focus(); };
  S.fast = function () { return this.skipping || matchMedia("(prefers-reduced-motion: reduce)").matches; };
  S.sleep = function (ms) { var self = this; return new Promise(function (r) { setTimeout(r, self.fast() ? 0 : ms); }); };

  // ------------------------------------------------------------- the intro
  S.start = function () {
    if (this.state !== "idle") return;
    this.state = "intro";
    var self = this;
    this.intro().then(function () { self.ready(); });
  };
  S.skip = function () { this.skipping = true; };

  S.typeCmd = async function (cmd) {
    var row = this.print(this.ps1() + '<span class="sh-cmd"></span><span class="sh-caret"></span>');
    var c = row.querySelector(".sh-cmd");
    if (this.fast()) c.textContent = cmd;
    else for (var i = 0; i < cmd.length; i++) {
      c.textContent += cmd[i];
      if (this.fast()) { c.textContent = cmd; break; }
      await this.sleep(cmd[i] === " " ? 70 : 28 + Math.random() * 40);
    }
    await this.sleep(260);
    row.querySelector(".sh-caret").remove();
  };

  S.intro = async function () {
    await this.sleep(300);
    this.print('<span class="c-f"># build pacpad from source. click here to skip ahead.</span>');
    await this.typeCmd("git clone https://github.com/ric-v/pacpad && cd pacpad");
    this.print("Cloning into 'pacpad'...");
    await this.sleep(380);
    var n = 0; FS.walk(FS.PROJ, function (x) { if (x.type === "file") n++; });
    var rx = this.print("");
    for (var p = 0; p <= 100; p += 10) { rx.textContent = "Receiving objects: " + pad(p + "%", 4) + " (" + Math.round((p / 100) * n) + "/" + n + ")" + (p === 100 ? ", done." : ""); await this.sleep(45); }
    this.print("Resolving deltas: 100%, done.");
    this.cwd = FS.PROJ;
    await this.sleep(300);
    await this.typeCmd("cargo build --release");
    var prog = null;
    for (var i = 0; i < CRATES.length; i++) {
      if (prog) prog.remove();
      this.print('<span class="c-g B">   Compiling</span> ' + esc(CRATES[i]));
      var k = Math.round(((i + 1) / (CRATES.length + 1)) * 26);
      prog = this.print('<span class="c-a B">    Building</span> [' + new Array(k + 1).join("=") + ">" + new Array(27 - k).join(" ") + "] " + (i + 1) * 5 + "/183: " + esc(CRATES[i].split(" ")[0]));
      await this.sleep(60 + Math.random() * 90);
    }
    prog.remove();
    this.print('<span class="c-g B">   Compiling</span> pacpad v0.1.0 (' + FS.PROJ + ")");
    await this.sleep(900);
    this.print('<span class="c-g B">    Finished</span> `release` profile [optimized] target(s) in 41.20s');
    await this.sleep(400);
    await this.typeCmd("install -Dm755 target/release/pacpad ~/.local/bin/pacpad");
    await this.sleep(200);
    await this.typeCmd("pacpad --version");
    this.print("pacpad 0.1.0");
    this.print("");
    this.print('<span class="c-y">your turn.</span> <span class="c-d">this is the real repo. try</span> ' + this.tryLink("ls -la") + '<span class="c-d">,</span> ' + this.tryLink("nano README.md") + '<span class="c-d">,</span> ' + this.tryLink("cargo test") + ' <span class="c-d">or</span> ' + this.tryLink("help"));
  };
  S.tryLink = function (cmd) { return '<a class="sh-try" data-cmd="' + esc(cmd) + '">' + esc(cmd) + "</a>"; };

  S.ready = function () {
    this.state = "ready";
    this.skipping = false;
    this.line.hidden = false;
    this.line.querySelector(".sh-ps1").innerHTML = this.ps1();
    this.bindTries();
    this.scroll();
    if (this.opts.onReady) this.opts.onReady();
  };
  S.bindTries = function () {
    var self = this;
    this.out.querySelectorAll(".sh-try:not([href])").forEach(function (a) {
      a.href = "#install";
      a.addEventListener("click", function (e) { e.preventDefault(); e.stopPropagation(); self.runTyped(a.dataset.cmd); });
    });
  };
  // types a suggested command into the prompt, then runs it
  S.runTyped = async function (cmd) {
    if (this.state !== "ready" || this.busy || this.prog) return;
    this.busy = true;
    this.input.value = "";
    for (var i = 0; i < cmd.length; i++) { this.input.value += cmd[i]; await this.sleep(18); }
    await this.sleep(160);
    this.busy = false;
    await this.submit();
    this.focus();
  };

  // ------------------------------------------------------------- input
  S.onKey = function (e) {
    var k = e.key;
    e.stopPropagation();
    if (this.running) { if ((k === "c" || k === "C") && e.ctrlKey) { e.preventDefault(); this.cancel = true; } else e.preventDefault(); return; }
    if (k === "Enter") { e.preventDefault(); this.submit(); }
    else if (k === "ArrowUp") { e.preventDefault(); if (this.hi > 0) { this.hi--; this.input.value = this.hist[this.hi]; } }
    else if (k === "ArrowDown") { e.preventDefault(); if (this.hi < this.hist.length) { this.hi++; this.input.value = this.hist[this.hi] || ""; } }
    else if (k === "Tab") { e.preventDefault(); this.complete(); }
    else if (k === "Escape") { this.input.blur(); if (this.opts.onBlur) this.opts.onBlur(); }
    else if ((k === "l" || k === "L") && e.ctrlKey) { e.preventDefault(); this.out.innerHTML = ""; }
    else if ((k === "c" || k === "C") && e.ctrlKey && !String(window.getSelection())) { e.preventDefault(); this.print(this.ps1() + esc(this.input.value) + "^C"); this.input.value = ""; }
    else if ((k === "u" || k === "U") && e.ctrlKey) { e.preventDefault(); this.input.value = ""; }
  };

  S.complete = function () {
    var v = this.input.value, ws = v.split(" "), last = ws[ws.length - 1], self = this, opts;
    if (ws.length === 1) opts = COMMANDS.filter(function (c) { return c.indexOf(last) === 0; }).map(function (c) { return c + " "; });
    else if (ws[0] === "pacpad" && ws.length === 2) opts = ["search ", "webapp ", "tui ", "--help", "--version", "--dry-run "].filter(function (c) { return c.indexOf(last) === 0; });
    else if (ws[0] === "git" && ws.length === 2) opts = ["status", "log", "diff", "branch", "remote", "show"].filter(function (c) { return c.indexOf(last) === 0; }).map(function (c) { return c + " "; });
    else if (ws[0] === "cargo" && ws.length === 2) opts = ["build", "run", "test", "check", "clippy", "fmt"].filter(function (c) { return c.indexOf(last) === 0; }).map(function (c) { return c + " "; });
    else {
      var slash = last.lastIndexOf("/"), dirPart = slash >= 0 ? last.slice(0, slash + 1) : "", base = slash >= 0 ? last.slice(slash + 1) : last;
      var dir = FS.resolve(this.cwd, dirPart || ".");
      opts = FS.list(dir, base[0] === ".").filter(function (n) { return n.name.indexOf(base) === 0; }).map(function (n) { return dirPart + n.name + (n.type === "dir" ? "/" : " "); });
    }
    if (!opts.length) return;
    var pre = opts.reduce(function (a, b) { var i = 0; while (i < a.length && a[i] === b[i]) i++; return a.slice(0, i); });
    if (opts.length === 1 || pre.length > last.length) { ws[ws.length - 1] = opts.length === 1 ? opts[0] : pre; this.input.value = ws.join(" "); return; }
    this.print(this.ps1() + esc(v));
    this.print(opts.map(function (o) { o = o.trim(); var n = o.split("/"); var b = /\/$/.test(o) ? n[n.length - 2] + "/" : n[n.length - 1]; return /\/$/.test(o) ? '<span class="c-a B">' + esc(b) + "</span>" : esc(b); }).join("  "));
    void self;
  };

  S.submit = async function () {
    if (this.running) return;
    var line = this.input.value;
    this.input.value = "";
    this.print(this.ps1() + esc(line));
    if (line.trim()) this.hist.push(line);
    this.hi = this.hist.length;
    this.running = true; this.cancel = false;
    this.line.classList.add("busy");
    try { await this.run(line.trim()); } catch (err) { this.print("bash: " + esc(err.message || err), "c-r"); }
    this.running = false;
    this.line.classList.remove("busy");
    this.line.querySelector(".sh-ps1").innerHTML = this.ps1();
    this.bindTries();
    this.scroll();
    if (!this.prog) this.input.focus({ preventScroll: true });
    if (this.opts.onCommand) this.opts.onCommand(line);
  };

  // ------------------------------------------------------------- pipelines
  // a sink collects lines; it either prints them or buffers them for a pipe
  function Sink(shell, buffer) {
    this.shell = shell; this.buf = buffer ? [] : null;
  }
  Sink.prototype.html = function (h, cls) { if (this.buf) this.buf.push(strip(h)); else this.shell.print(h, cls); };
  Sink.prototype.text = function (t, cls) { if (this.buf) this.buf.push(String(t)); else this.shell.print(esc(t), cls); };
  Sink.prototype.err = function (t) { this.shell.print(esc(t), "c-r"); };

  S.run = async function (line) {
    if (!line) return;
    await FS.init();
    var seq = splitOp(line, ["&&", ";"]), ok = true;
    for (var i = 0; i < seq.length; i++) {
      var prev = i ? seq[i - 1].op : null;
      if (prev === "&&" && !ok) break;
      if (!seq[i].text.trim()) continue;
      ok = await this.pipeline(seq[i].text.trim());
      if (this.cancel) { this.print("^C"); break; }
    }
  };

  S.pipeline = async function (text) {
    var stages = splitOp(text, ["|"]).map(function (p) { return p.text.trim(); }), stdin = null, ok = true;
    for (var i = 0; i < stages.length; i++) {
      var st = stages[i], redirect = null, app = false;
      var m = /\s(>>?)\s*(\S+)\s*$/.exec(" " + st);
      if (m) { redirect = m[2]; app = m[1] === ">>"; st = (" " + st).slice(0, m.index).trim(); }
      var last = i === stages.length - 1, sink = new Sink(this, !last || !!redirect);
      ok = await this.exec(expandGlobs(words(st), this.cwd), sink, stdin);
      stdin = sink.buf;
      if (redirect && last) {
        var p = FS.resolve(this.cwd, redirect);
        try {
          var prevTxt = app && FS.stat(p) ? await FS.read(p) : "";
          FS.write(p, prevTxt + (sink.buf.length ? sink.buf.join("\n") + "\n" : ""));
        } catch (e) { this.print("bash: " + esc(redirect) + ": " + esc(e.message), "c-r"); ok = false; }
      }
    }
    return ok !== false;
  };

  // ------------------------------------------------------------- commands
  S.exec = async function (w, o, stdin) {
    var cmd = w[0], a = w.slice(1), self = this;
    if (!cmd) return true;
    if (cmd === "sudo") {
      if (!a.length) { o.text("usage: sudo -h | -K | -k | -V"); return false; }
      o.text("[sudo] password for " + PP.user + ": ");
      o.html('<span class="c-m">Sorry, this is a website. Nothing here can run as root.</span>');
      return false;
    }
    if (cmd === "./target/release/pacpad" || cmd === "target/release/pacpad" || cmd === "~/.local/bin/pacpad" || cmd === FS.HOME + "/.local/bin/pacpad") cmd = "pacpad";
    var fn = this.cmds[cmd];
    if (fn) { var r = await fn.call(this, a, o, stdin); return r !== false; }
    if (cmd === "vi" || cmd === "nvim") return this.cmds.vim.call(this, a, o, stdin);
    var pkg = PP.find(cmd === "hx" ? "helix" : cmd === "rg" ? "ripgrep" : cmd);
    if (pkg && pkg.installed) { o.html('<span class="c-d">' + esc(cmd) + " is installed on this box, but it needs a real terminal. try it in pacpad: press d.</span>"); return true; }
    if (pkg && !pkg.installed) { o.text("bash: " + cmd + ": command not found"); o.html('<span class="c-f">hint: ' + esc(pkg.name) + " isn't installed. press d and search for it in the demo above.</span>"); return false; }
    if (/^(code|emacs|micro|subl)$/.test(cmd)) { o.text("bash: " + cmd + ": command not found"); o.html('<span class="c-f">this box has nano and vim.</span>'); return false; }
    if (cmd.indexOf("/") >= 0) {
      var p = FS.resolve(this.cwd, cmd), n = FS.stat(p);
      o.text("bash: " + cmd + ": " + (!n ? "No such file or directory" : n.type === "dir" ? "Is a directory" : "Permission denied"));
      return false;
    }
    o.text("bash: " + cmd + ": command not found");
    void self;
    return false;
  };

  S.path = function (p) { return FS.resolve(this.cwd, p); };
  S.readOr = async function (path, label, o) {
    var n = FS.stat(path);
    if (!n) { o.text(label + ": No such file or directory"); return null; }
    if (n.type === "dir") { o.text(label + ": Is a directory"); return null; }
    if (FS.isBinary(path)) { o.text(label + ": binary file (" + human(n.size) + "B), not printing it"); return null; }
    try { return await FS.read(path); } catch (e) { o.text(label + ": " + e.message); return null; }
  };
  S.lines = async function (args, o, stdin, cmdName) {
    if (!args.length) return stdin ? [{ name: "", lines: stdin }] : [];
    var out = [];
    for (var i = 0; i < args.length; i++) {
      var t = await this.readOr(this.path(args[i]), cmdName + ": " + args[i], o);
      if (t !== null) out.push({ name: args[i], lines: t.replace(/\n$/, "").split("\n") });
    }
    return out;
  };

  S.openProgram = function (mode, path, text, isNew) {
    var self = this;
    return new Promise(function (res) {
      self.input.blur();
      self.wrap.classList.add("ed-open");
      self.wrap.setAttribute("data-prog", mode);
      var display = path ? FS.pretty(path).replace(FS.pretty(self.cwd) + "/", "") : "";
      if (path && path.indexOf(self.cwd + "/") === 0) display = path.slice(self.cwd.length + 1);
      self.prog = window.PPEditor.open(self.wrap, {
        mode: mode, path: path || "", display: display, text: text, isNew: isNew,
        writable: !path || FS.writable(path),
        save: function (t, to) {
          var target = to ? self.path(to) : path;
          if (!target) return "[ No file name ]";
          try { FS.write(target, t); path = target; return null; }
          catch (e) { return "[ Error writing " + (to || display) + ": " + e.message + " ]"; }
        },
        onExit: function () {
          self.prog = null;
          self.wrap.classList.remove("ed-open");
          self.wrap.removeAttribute("data-prog");
          document.dispatchEvent(new CustomEvent("pp-mode", { detail: "shell" }));
          setTimeout(function () { self.input.focus({ preventScroll: true }); }, 0);
          res();
        }
      });
      document.dispatchEvent(new CustomEvent("pp-mode", { detail: mode }));
    });
  };

  S.edit = async function (mode, a, o, stdin) {
    var f = a.filter(function (x) { return x[0] !== "-" && x[0] !== "+"; })[0];
    if (mode === "less" && !f && stdin) { await this.openProgram("less", "", stdin.join("\n") + "\n"); return true; }
    if (!f) {
      if (mode === "less") { o.text("Missing filename (\"less --help\" for help)"); return false; }
      await this.openProgram(mode, "", "", true); return true;
    }
    var p = this.path(f), n = FS.stat(p);
    if (n && n.type === "dir") { o.text(mode === "nano" ? "nano: " + f + " is a directory" : mode === "vim" ? '"' + f + '" is a directory' : f + " is a directory"); return false; }
    if (n && FS.isBinary(p)) { o.text(mode === "less" ? '"' + f + '" may be a binary file.  See it anyway? n' : f + ": binary file, not opening it here"); return false; }
    if (!n) {
      if (mode === "less") { o.text(f + ": No such file or directory"); return false; }
      await this.openProgram(mode, p, "", true); return true;
    }
    var loading = this.print('<span class="c-f">reading ' + esc(f) + (n.repo ? " from github.com/" + FS.OWNER + "/" + FS.REPO : "") + "…</span>");
    var t;
    try { t = await FS.read(p); } catch (e) { loading.remove(); o.text(f + ": " + e.message); return false; }
    loading.remove();
    await this.openProgram(mode, p, t, false);
    return true;
  };

  S.cmds = {
    help: function (a, o) {
      o.html('<span class="c-y B">pacpad web shell</span> <span class="c-d">· ~/pacpad is the real repo, read live from GitHub · writes stay in this tab</span>');
      GUIDE.forEach(function (g) {
        if (!g[1] && g[0].indexOf(" ") < 0 && g[0].indexOf("·") < 0) { o.html(""); o.html('<span class="c-f">' + esc(g[0]) + "</span>"); return; }
        o.html("  " + '<span class="c-a">' + esc(pad(g[0], 46)) + "</span>" + '<span class="c-d">' + esc(g[1]) + "</span>");
      });
      o.html("");
      o.html('<span class="c-f">↑↓ history · tab completes paths · | pipes · ctrl+l clears · esc leaves the prompt</span>');
    },
    clear: function () { this.out.innerHTML = ""; },
    history: function (a, o) { this.hist.forEach(function (h, i) { o.text(lpad(i + 1, 5) + "  " + h); }); },
    whoami: function (a, o) { o.text(PP.user); },
    hostname: function (a, o) { o.text(PP.host); },
    id: function (a, o) { o.text("uid=1000(" + PP.user + ") gid=1000(" + PP.user + ") groups=1000(" + PP.user + "),998(wheel)"); },
    pwd: function (a, o) { o.text(this.cwd); },
    true: function () {},
    date: function (a, o) { o.text(new Date().toUTCString().replace(/^(\w+), (\d+) (\w+) (\d+) (\S+) GMT$/, "$1 $3 $2 $5 UTC $4")); },
    uptime: function (a, o) { o.text(" 12:00:00 up 3 days,  2:14,  1 user,  load average: 0.08, 0.12, 0.09"); },
    uname: function (a, o) { var f = flags(a).f; o.text(f.a ? "Linux " + PP.host + " 6.11.6-arch1-1 #1 SMP PREEMPT_DYNAMIC x86_64 GNU/Linux" : f.r ? "6.11.6-arch1-1" : "Linux"); },
    echo: function (a, o) { o.text(a.join(" ").replace(/\$HOME|~(?=\/|$)/g, FS.HOME).replace(/\$USER/g, PP.user).replace(/\$SHELL/g, "/usr/bin/bash").replace(/\$EDITOR/g, "nano")); },
    env: function (a, o) { ["SHELL=/usr/bin/bash", "EDITOR=nano", "HOME=" + FS.HOME, "LANG=en_US.UTF-8", "USER=" + PP.user, "PATH=" + FS.HOME + "/.local/bin:/usr/local/bin:/usr/bin", "TERM=xterm-ghostty", "PWD=" + this.cwd].forEach(function (l) { o.text(l); }); },
    alias: function (a, o) { o.text("alias grep='grep --color=auto'"); o.text("alias ls='ls --color=auto'"); },
    exit: function (a, o) { o.html('<span class="c-d">there\'s no leaving. try <span class="c-a">pacpad</span> instead.</span>'); },
    which: function (a, o) {
      var ok = true;
      a.forEach(function (c) {
        if (c === "pacpad") o.text(FS.HOME + "/.local/bin/pacpad");
        else if (COMMANDS.indexOf(c) >= 0 && FS.stat("/usr/bin/" + c)) o.text("/usr/bin/" + c);
        else if (COMMANDS.indexOf(c) >= 0) o.text("/usr/bin/" + c);
        else { o.text("which: no " + c + " in (" + FS.HOME + "/.local/bin:/usr/local/bin:/usr/bin)"); ok = false; }
      });
      return ok;
    },
    type: function (a, o) { a.forEach(function (c) { o.text(/^(cd|echo|pwd|history|alias|exit|type|true)$/.test(c) ? c + " is a shell builtin" : c === "ls" || c === "grep" ? c + " is aliased to `" + c + " --color=auto'" : c + " is /usr/bin/" + c); }); },

    cd: function (a, o) {
      var t = a[0];
      if (t === "-") { t = this.oldpwd; o.text(t); }
      var p = this.path(t || "~"), n = FS.stat(p);
      if (!n) { o.text("bash: cd: " + a[0] + ": No such file or directory"); return false; }
      if (n.type !== "dir") { o.text("bash: cd: " + a[0] + ": Not a directory"); return false; }
      if (p === "/root" || p.indexOf("/root/") === 0) { o.text("bash: cd: " + a[0] + ": Permission denied"); return false; }
      this.oldpwd = this.cwd; this.cwd = p;
    },

    ls: function (a, o) {
      var F = flags(a), f = F.f, targets = F.rest.length ? F.rest : ["."], self = this, ok = true;
      var all = f.a || f.A || f.all, long = f.l, h = f.h;
      var name = function (n) {
        var cls = n.type === "dir" ? "c-a B" : /x/.test(n.mode) ? "c-g B" : /\.desktop$/.test(n.name) ? "" : n.binary || FS.isBinary(n.path) ? "c-m" : "";
        return cls ? '<span class="' + cls + '">' + esc(n.name) + "</span>" : esc(n.name);
      };
      var row = function (n, label) {
        var size = h ? human(n.size) : String(n.size);
        return esc(n.mode + " 1 " + pad(n.owner || PP.user, 6) + " " + pad(n.owner || PP.user, 6) + " " + lpad(size, h ? 5 : 8) + " " + lsDate(n.mtime) + " ") + (label || name(n));
      };
      var files = [], dirs = [];
      targets.forEach(function (t) {
        var p = self.path(t), n = FS.stat(p);
        if (!n) { o.text("ls: cannot access '" + t + "': No such file or directory"); ok = false; return; }
        if (p === "/root" && n.type === "dir") { o.text("ls: cannot open directory '" + t + "': Permission denied"); ok = false; return; }
        (n.type === "dir" ? dirs : files).push({ t: t, n: n, p: p });
      });
      files.forEach(function (x) { o.html(long ? row(x.n, esc(x.t)) : esc(x.t)); });
      dirs.forEach(function (x, i) {
        if (targets.length > 1) { if (i || files.length) o.html(""); o.text(x.t + ":"); }
        var kids = FS.list(x.p, all);
        if (f.t) kids.sort(function (p, q) { return p.mtime < q.mtime ? 1 : -1; });
        if (f.S) kids.sort(function (p, q) { return q.size - p.size; });
        if (f.r) kids.reverse();
        if (long) {
          var blocks = kids.reduce(function (s, k) { return s + Math.ceil(k.size / 4096) * 4; }, 0);
          o.text("total " + (h ? human(blocks * 1024) : blocks));
          if (f.a) { o.html(row(x.n, '<span class="c-a B">.</span>')); o.html(row(FS.stat(x.p === "/" ? "/" : x.p.replace(/\/[^/]+$/, "") || "/") || x.n, '<span class="c-a B">..</span>')); }
          kids.forEach(function (k) { o.html(row(k)); });
        } else if (f["1"] || o.buf) kids.forEach(function (k) { o.html(name(k)); });
        else if (kids.length) o.html((f.a ? '<span class="c-a B">.</span>  <span class="c-a B">..</span>  ' : "") + kids.map(name).join("  "));
      });
      return ok;
    },

    tree: function (a, o) {
      var F = flags(a.filter(function (x, i) { return a[i - 1] !== "-L"; })), depth = a.indexOf("-L") >= 0 ? parseInt(a[a.indexOf("-L") + 1], 10) || 99 : 99;
      var root = F.rest[0] || ".", p = this.path(root), nd = 0, nf = 0;
      if (!FS.stat(p)) { o.text(root + " [error opening dir]"); return false; }
      o.html('<span class="c-a B">' + esc(root) + "</span>");
      var walk = function (dir, prefix, d) {
        var kids = FS.list(dir, F.f.a).filter(function (k) { return k.name !== ".git"; });
        kids.forEach(function (k, i) {
          var lastK = i === kids.length - 1;
          o.html('<span class="c-b">' + prefix + (lastK ? "└── " : "├── ") + "</span>" + (k.type === "dir" ? '<span class="c-a B">' + esc(k.name) + "</span>" : esc(k.name)));
          if (k.type === "dir") { nd++; if (d < depth) walk(k.path, prefix + (lastK ? "    " : "│   "), d + 1); } else nf++;
        });
      };
      walk(p, "", 1);
      o.html("");
      o.text(nd + " director" + (nd === 1 ? "y" : "ies") + ", " + nf + " file" + (nf === 1 ? "" : "s"));
    },

    cat: async function (a, o, stdin) {
      var F = flags(a), ok = true, files = await this.lines(F.rest, o, stdin, "cat");
      if (F.rest.length && files.length < F.rest.length) ok = false;
      var n = 0;
      files.forEach(function (fl) { fl.lines.forEach(function (l) { o.text(F.f.n ? lpad(++n, 6) + "  " + l : l); }); });
      return ok;
    },
    head: async function (a, o, stdin) { return this.headTail(a, o, stdin, true); },
    tail: async function (a, o, stdin) { return this.headTail(a, o, stdin, false); },
    wc: async function (a, o, stdin) {
      var F = flags(a), files = await this.lines(F.rest, o, stdin, "wc"), tot = [0, 0, 0];
      var only = F.f.l ? 0 : F.f.w ? 1 : F.f.c ? 2 : -1;
      files.forEach(function (fl) {
        var txt = fl.lines.join("\n") + "\n", c = [fl.lines.length, txt.split(/\s+/).filter(Boolean).length, new Blob([txt]).size];
        tot = tot.map(function (v, i) { return v + c[i]; });
        o.text((only >= 0 ? String(c[only]) : c.map(function (v) { return lpad(v, 7); }).join(" ")) + (fl.name ? " " + fl.name : ""));
      });
      if (files.length > 1) o.text((only >= 0 ? String(tot[only]) : tot.map(function (v) { return lpad(v, 7); }).join(" ")) + " total");
    },
    sort: async function (a, o, stdin) { var F = flags(a), files = await this.lines(F.rest, o, stdin, "sort"), all = []; files.forEach(function (f) { all = all.concat(f.lines); }); all.sort(); if (F.f.r) all.reverse(); all.forEach(function (l) { o.text(l); }); },
    uniq: async function (a, o, stdin) { var files = await this.lines(a, o, stdin, "uniq"), prev = null; files.forEach(function (f) { f.lines.forEach(function (l) { if (l !== prev) o.text(l); prev = l; }); }); },

    less: async function (a, o, stdin) { return this.edit("less", a, o, stdin); },
    more: async function (a, o, stdin) { return this.edit("less", a, o, stdin); },
    nano: async function (a, o, stdin) { return this.edit("nano", a, o, stdin); },
    vim: async function (a, o, stdin) { return this.edit("vim", a, o, stdin); },
    man: async function (a, o) {
      if (!a.length) { o.text("What manual page do you want?"); o.text("For example, try 'man man'."); return false; }
      if (a[0] !== "pacpad") { o.text("No manual entry for " + a[0]); return false; }
      var t = ["PACPAD(1)                        User Commands                        PACPAD(1)", "", "NAME", "       pacpad - a terminal app manager for Arch", "", "SYNOPSIS", "       pacpad [OPTIONS] [COMMAND]", "", "DESCRIPTION", "       One TUI for repo and AUR packages, plus the webapp and TUI-app launchers", "       you create. Every change is a plain pacman or yay command, shown before", "       it runs.", "", "COMMANDS"].concat(HELP.slice(5, 9).map(function (l) { return "       " + strip(l).trim(); })).concat(["", "OPTIONS"]).concat(HELP.slice(11).map(function (l) { return "     " + strip(l).replace(/ \[default: .*\]$/, "").replace(/, and run nothing$/, ""); })).concat(["", "FILES", "       ~/.config/pacpad/config.toml", "              browser, terminal and aur_helper. Every key is optional.", "       ~/.cache/pacpad/index.bin", "              the package index cache.", "       ~/.local/share/applications/pacpad-*.desktop", "              launchers pacpad created (X-PacPad-Managed=true).", "", "SEE ALSO", "       pacman(8), yay(8), pacman.conf(5)", "", "pacpad 0.1.0                       2026-11-07                         PACPAD(1)"]);
      await this.openProgram("less", "", t.join("\n") + "\n");
    },

    grep: async function (a, o, stdin) { return this.grep(a, o, stdin); },
    rg: async function (a, o, stdin) { return this.grep(["-rn"].concat(a), o, stdin); },

    find: function (a, o) {
      var root = a[0] && a[0][0] !== "-" ? a[0] : ".", rest = a[0] && a[0][0] !== "-" ? a.slice(1) : a, nm = null, type = null, ok = true;
      for (var i = 0; i < rest.length; i++) {
        if (rest[i] === "-name" || rest[i] === "-iname") { nm = glob(rest[++i] || ""); if (rest[i - 1] === "-iname") nm = new RegExp(nm.source, "i"); }
        else if (rest[i] === "-type") type = rest[++i];
      }
      var p = this.path(root);
      if (!FS.stat(p)) { o.text("find: '" + root + "': No such file or directory"); return false; }
      FS.walk(p, function (n) {
        if (nm && !nm.test(n.name)) return;
        if (type === "f" && n.type !== "file") return;
        if (type === "d" && n.type !== "dir") return;
        o.text(n.path === p ? root : (root === "/" ? "" : root.replace(/\/$/, "")) + n.path.slice(p === "/" ? 0 : p.length));
      });
      return ok;
    },
    du: function (a, o) {
      var F = flags(a), t = F.rest[0] || ".", p = this.path(t), sum = 0;
      if (!FS.stat(p)) { o.text("du: cannot access '" + t + "': No such file or directory"); return false; }
      FS.walk(p, function (n) { sum += n.type === "file" ? n.size : 4096; });
      o.text((F.f.h ? human(sum) : String(Math.ceil(sum / 1024))) + "\t" + t);
    },
    file: function (a, o) {
      a.forEach(function (t) {
        var p = this.path(t), n = FS.stat(p), d;
        if (!n) d = "cannot open `" + t + "' (No such file or directory)";
        else if (n.type === "dir") d = "directory";
        else if (/\/pacpad$/.test(p)) d = "ELF 64-bit LSB pie executable, x86-64, version 1 (SYSV), dynamically linked, interpreter /lib64/ld-linux-x86-64.so.2, for GNU/Linux 4.4.0, stripped";
        else if (/\.woff2$/.test(p)) d = "Web Open Font Format (Version 2)";
        else if (/\.png$/.test(p)) d = "PNG image data, 128 x 128, 8-bit/color RGBA, non-interlaced";
        else if (/\.iso$/.test(p)) d = "ISO 9660 CD-ROM filesystem data (DOS/MBR boot sector) 'ARCH_202611' (bootable)";
        else if (/\.db$/.test(p)) d = "gzip compressed data";
        else if (/\/usr\/bin\//.test(p)) d = "ELF 64-bit LSB pie executable, x86-64, dynamically linked, stripped";
        else if (FS.isBinary(p)) d = "data";
        else if (/\.rs$/.test(p)) d = "C source, ASCII text";
        else if (/\.svg$/.test(p)) d = "SVG Scalable Vector Graphics image";
        else if (/\.html$/.test(p)) d = "HTML document, Unicode text, UTF-8 text";
        else if (/\.desktop$/.test(p)) d = "freedesktop.org application entry, ASCII text";
        else d = "ASCII text";
        o.text(t + ": " + d);
      }, this);
    },
    stat: function (a, o) {
      var t = a[0], p = this.path(t || ""), n = t && FS.stat(p);
      if (!n) { o.text("stat: cannot statx '" + (t || "") + "': No such file or directory"); return false; }
      o.text("  File: " + t); o.text("  Size: " + pad(n.size, 15) + "Blocks: " + pad(Math.ceil(n.size / 512), 10) + " IO Block: 4096   " + (n.type === "dir" ? "directory" : "regular file"));
      o.text("Access: (" + (n.mode === "-rwxr-xr-x" || n.type === "dir" ? "0755" : "0644") + "/" + n.mode + ")  Uid: ( 1000/  " + PP.user + ")   Gid: ( 1000/  " + PP.user + ")");
      o.text("Modify: " + n.mtime.replace("T", " ").replace(/(\+.*|Z)$/, ""));
    },
    touch: function (a, o) {
      var ok = true;
      a.forEach(function (t) { var p = this.path(t); if (FS.stat(p)) return; try { FS.write(p, ""); } catch (e) { o.text("touch: cannot touch '" + t + "': " + e.message); ok = false; } }, this);
      return ok;
    },
    mkdir: function (a, o) {
      var F = flags(a), ok = true;
      F.rest.forEach(function (t) { try { FS.mkdir(this.path(t), F.f.p); } catch (e) { if (!(F.f.p && e.message === "File exists")) { o.text("mkdir: cannot create directory '" + t + "': " + e.message); ok = false; } } }, this);
      return ok;
    },
    rm: function (a, o) {
      var F = flags(a), ok = true;
      if (F.f.r && F.f.f && F.rest.some(function (t) { return t === "/" || t === "/*" || t === "~" || t === "~/"; })) { o.html('<span class="c-r B">rm: it is dangerous to operate recursively on \'/\'</span>'); o.html('<span class="c-d">and pacpad would have asked you to type REMOVE first anyway.</span>'); return false; }
      F.rest.forEach(function (t) {
        var p = this.path(t), n = FS.stat(p);
        if (!n) { if (!F.f.f) { o.text("rm: cannot remove '" + t + "': No such file or directory"); ok = false; } return; }
        if (n.type === "dir" && !F.f.r) { o.text("rm: cannot remove '" + t + "': Is a directory"); ok = false; return; }
        try { FS.rm(p); } catch (e) { o.text("rm: cannot remove '" + t + "': " + e.message); ok = false; }
      }, this);
      return ok;
    },
    cp: async function (a, o) { return this.copyMove(a, o, false); },
    mv: async function (a, o) { return this.copyMove(a, o, true); },

    open: function (a, o) { return this.cmds["xdg-open"].call(this, a, o); },
    "xdg-open": function (a, o) {
      var t = a[0];
      if (!t) { o.text("xdg-open: file or URL argument missing"); return false; }
      var url = /^https?:\/\//.test(t) ? t : FS.url(this.path(t));
      if (!url) { o.text("xdg-open: no method available for opening '" + t + "'"); return false; }
      window.open(url, "_blank", "noopener");
      o.html('<span class="c-d">opened ' + esc(url) + "</span>");
    },
    gh: function (a, o) {
      if (a[0] !== "browse") { o.text("gh: this box only knows `gh browse [path]`"); return false; }
      var t = a[1], url = t ? FS.url(this.path(t)) : "https://github.com/" + FS.OWNER + "/" + FS.REPO;
      if (!url) { o.text("gh: " + t + " isn't in the repository"); return false; }
      window.open(url, "_blank", "noopener");
      o.text("Opening " + url.replace("https://", "") + " in your browser.");
    },

    fastfetch: function (a, o) {
      var c = PP.counts(), L = function (k, v) { o.html('<span class="c-a B">' + esc(k) + "</span>: " + esc(v)); };
      o.html('<span class="c-a B">' + PP.user + '</span>@<span class="c-a B">' + PP.host + "</span>");
      o.text("--------------");
      L("OS", "Arch Linux x86_64"); L("Kernel", "Linux 6.11.6-arch1-1"); L("Uptime", "3 days, 2 hours, 14 mins");
      L("Packages", c.total + " (pacman), " + c.foreign + " foreign"); L("Shell", "bash 5.2.37"); L("WM", "Hyprland (Wayland)");
      L("Terminal", "ghostty"); L("Browser", "chromium (for webapps)"); L("Launchers", PP.apps.length + " made by pacpad");
      o.html('<span class="c-r">███</span><span class="c-m">███</span><span class="c-y">███</span><span class="c-g">███</span><span class="c-a">███</span><span class="c-d">███</span>');
    },
    neofetch: function (a, o) { return this.cmds.fastfetch.call(this, a, o); },
    rustc: function (a, o) { if (a[0] === "--version" || a[0] === "-V") o.text("rustc 1.91.0 (f8297e351 2025-10-28)"); else { o.text("error: no input filename given"); return false; } },

    git: async function (a, o) { return this.git(a, o); },
    cargo: async function (a, o) { return this.cargo(a, o); },
    pacman: function (a, o) { return this.pacman(a, o, "pacman"); },
    yay: function (a, o) { return this.pacman(a, o, "yay"); },
    paru: function (a, o) { return this.pacman(a, o, "paru"); },
    pacpad: function (a, o) { return this.pacpad(a, o); },
    printenv: function (a, o) { return this.cmds.env.call(this, a, o); }
  };

  S.headTail = async function (a, o, stdin, head) {
    var n = 10, rest = [];
    for (var i = 0; i < a.length; i++) {
      if (a[i] === "-n") n = parseInt(a[++i], 10) || 10;
      else if (/^-n?\d+$/.test(a[i])) n = parseInt(a[i].replace(/^-n?/, ""), 10);
      else rest.push(a[i]);
    }
    var files = await this.lines(rest, o, stdin, head ? "head" : "tail");
    files.forEach(function (fl, i) {
      if (files.length > 1) { if (i) o.text(""); o.text("==> " + fl.name + " <=="); }
      (head ? fl.lines.slice(0, n) : fl.lines.slice(-n)).forEach(function (l) { o.text(l); });
    });
    return !rest.length || files.length === rest.length;
  };

  S.copyMove = async function (a, o, move) {
    var F = flags(a), name = move ? "mv" : "cp";
    if (F.rest.length < 2) { o.text(name + ": missing file operand"); return false; }
    var src = this.path(F.rest[0]), dst = this.path(F.rest[1]), n = FS.stat(src);
    if (!n) { o.text(name + ": cannot stat '" + F.rest[0] + "': No such file or directory"); return false; }
    if (n.type === "dir") { o.text(name + ": -r not specified; omitting directory '" + F.rest[0] + "'"); return false; }
    if (FS.stat(dst) && FS.stat(dst).type === "dir") dst = dst + "/" + n.name;
    try {
      var t = FS.isBinary(src) ? "" : await FS.read(src);
      FS.write(dst, t);
      if (move) FS.rm(src);
    } catch (e) { o.text(name + ": cannot " + (move ? "move" : "create regular file") + " '" + F.rest[1] + "': " + e.message); return false; }
  };

  // ------------------------------------------------------------- grep
  S.grep = async function (a, o, stdin) {
    var F = { f: {} }, rest = [];
    a.forEach(function (x) { if (/^-[a-zA-Z]+$/.test(x)) x.slice(1).split("").forEach(function (c) { F.f[c] = true; }); else if (!/^--/.test(x)) rest.push(x); });
    var f = F.f, pat = rest.shift();
    if (pat === undefined) { o.text("Usage: grep [OPTION]... PATTERNS [FILE]..."); o.text("Try 'grep --help' for more information."); return false; }
    var re; try { re = new RegExp(pat, f.i ? "gi" : "g"); } catch (e) { re = new RegExp(pat.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), f.i ? "gi" : "g"); }
    var rec = f.r || f.R, targets = [], self = this;
    if (!rest.length && !stdin && rec) rest = ["."];
    if (!rest.length && stdin) targets.push({ label: "", lines: stdin });
    else {
      var paths = [];
      rest.forEach(function (t) {
        var p = self.path(t), n = FS.stat(p);
        if (!n) { o.text("grep: " + t + ": No such file or directory"); return; }
        if (n.type === "dir") {
          if (!rec) { o.text("grep: " + t + ": Is a directory"); return; }
          FS.walk(p, function (k) { if (k.type === "file" && !FS.isBinary(k.path) && k.path.indexOf("/.git/") < 0) paths.push({ p: k.path, label: (t === "." ? "" : t.replace(/\/$/, "") + "/") + k.path.slice(p.length + 1) }); });
        } else paths.push({ p: p, label: t });
      });
      var multi = rec || paths.length > 1;
      var note = paths.length > 6 ? this.print('<span class="c-f">searching ' + paths.length + " files…</span>") : null;
      for (var i = 0; i < paths.length; i += 8) {
        if (this.cancel) break;
        var chunk = await Promise.all(paths.slice(i, i + 8).map(function (x) { return FS.read(x.p).then(function (t) { return { label: x.label, lines: t.split("\n"), multi: multi }; }, function () { return null; }); }));
        chunk.forEach(function (c) { if (c) targets.push(c); });
      }
      if (note) note.remove();
    }
    var hits = 0;
    targets.forEach(function (t) {
      var count = 0;
      t.lines.forEach(function (l, i) {
        re.lastIndex = 0;
        var m = re.test(l);
        if (f.v ? m : !m) return;
        count++; hits++;
        if (f.l || f.c) return;
        re.lastIndex = 0;
        var body = f.v ? esc(l) : esc(l).replace(new RegExp(re.source.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;"), re.flags), function (s) { return '<span class="c-r B">' + s + "</span>"; });
        o.html((t.multi ? '<span class="c-m">' + esc(t.label) + '</span><span class="c-a">:</span>' : "") + (f.n ? '<span class="c-g">' + (i + 1) + '</span><span class="c-a">:</span>' : "") + body);
      });
      if (f.l && count) o.html('<span class="c-m">' + esc(t.label) + "</span>");
      if (f.c) o.text((t.multi ? t.label + ":" : "") + count);
    });
    return hits > 0;
  };

  // ------------------------------------------------------------- git
  S.inRepo = function () { return this.cwd === FS.PROJ || this.cwd.indexOf(FS.PROJ + "/") === 0; };
  S.git = async function (a, o) {
    var sub = a[0];
    if (sub === "clone") {
      var target = (a[1] || "").replace(/\.git$/, "").split("/").pop();
      if (!target) { o.text("fatal: You must specify a repository to clone."); return false; }
      if (FS.stat(this.path(a[2] || target))) { o.text("fatal: destination path '" + (a[2] || target) + "' already exists and is not an empty directory."); return false; }
      o.text("Cloning into '" + target + "'..."); o.text("fatal: unable to access '" + a[1] + "': this box only has one clone, and it's ~/pacpad"); return false;
    }
    if (sub === "--version" || sub === "version") { o.text("git version 2.47.0"); return; }
    if (!this.inRepo()) { o.text("fatal: not a git repository (or any of the parent directories): .git"); return false; }
    var mods = FS.modified();
    switch (sub) {
      case "status": case "st":
        if (a.indexOf("-s") >= 0 || a.indexOf("--short") >= 0) {
          mods.forEach(function (m) { o.html(m.kind === "modified" ? ' <span class="c-r">M</span> ' + esc(m.path) : '<span class="c-r">??</span> ' + esc(m.path)); });
          return;
        }
        o.text("On branch " + FS.REF);
        o.text("Your branch is up to date with 'origin/" + FS.REF + "'.");
        o.text("");
        var ch = mods.filter(function (m) { return m.kind === "modified"; }), un = mods.filter(function (m) { return m.kind === "untracked"; });
        if (!mods.length) { o.text("nothing to commit, working tree clean"); return; }
        if (ch.length) { o.text("Changes not staged for commit:"); o.text('  (use "git add <file>..." to update what will be committed)'); o.text('  (use "git restore <file>..." to discard changes in working directory)'); ch.forEach(function (m) { o.html('\t<span class="c-r">modified:   ' + esc(m.path) + "</span>"); }); o.text(""); }
        if (un.length) { o.text("Untracked files:"); o.text('  (use "git add <file>..." to include in what will be committed)'); un.forEach(function (m) { o.html('\t<span class="c-r">' + esc(m.path) + "</span>"); }); o.text(""); }
        o.text(ch.length ? 'no changes added to commit (use "git add" and/or "git commit -a")' : 'nothing added to commit but untracked files present (use "git add" to track)');
        return;
      case "log":
        var cs = await FS.commits(), one = a.indexOf("--oneline") >= 0, nIdx = a.findIndex(function (x) { return /^-n$|^-\d+$/.test(x); });
        var lim = nIdx >= 0 ? (a[nIdx] === "-n" ? parseInt(a[nIdx + 1], 10) : parseInt(a[nIdx].slice(1), 10)) : cs.length;
        cs.slice(0, lim).forEach(function (c, i) {
          var dec = i === 0 ? ' <span class="c-y">(</span><span class="c-a B">HEAD -&gt; </span><span class="c-g B">' + FS.REF + '</span><span class="c-y">, </span><span class="c-r B">origin/' + FS.REF + '</span><span class="c-y">)</span>' : "";
          if (one) { o.html('<span class="c-y">' + c[0].slice(0, 7) + "</span>" + dec + " " + esc(c[3])); return; }
          o.html('<span class="c-y">commit ' + c[0] + "</span>" + dec);
          o.text("Author: " + c[1]);
          o.text("Date:   " + new Date(c[2]).toString().replace(/ GMT.*$/, ""));
          o.text(""); o.text("    " + c[3]); o.text("");
        });
        return;
      case "diff":
        var list = mods.filter(function (m) { return m.kind === "modified" && (!a[1] || this.path(a[1]) === FS.PROJ + "/" + m.path); }, this);
        for (var i = 0; i < list.length; i++) await this.diff(list[i].path, o);
        return;
      case "branch": o.html('* <span class="c-g">' + FS.REF + "</span>"); return;
      case "remote": o.text(a[1] === "-v" ? "origin\thttps://github.com/" + FS.OWNER + "/" + FS.REPO + " (fetch)" : "origin"); if (a[1] === "-v") o.text("origin\thttps://github.com/" + FS.OWNER + "/" + FS.REPO + " (push)"); return;
      case "restore": case "checkout":
        var t = a.filter(function (x) { return x !== "--" && x !== sub; })[0];
        if (!t) { o.text("fatal: you must specify path(s) to restore"); return false; }
        var p = this.path(t), n = FS.stat(p);
        if (!n || !n.repo) { o.text("error: pathspec '" + t + "' did not match any file(s) known to git"); return false; }
        delete n.mem; n.size = n.origSize || n.size; n.mtime = n.origMtime || n.mtime; return;
      case "add": case "commit": case "push": case "pull": case "fetch": case "rebase": case "merge": case "stash": case "reset":
        o.html('<span class="c-d">this clone lives in your browser, so it can\'t ' + sub + ". send a pull request at github.com/" + FS.OWNER + "/" + FS.REPO + " instead.</span>");
        return false;
      case "show":
        var cs2 = await FS.commits(); if (!cs2.length) return;
        o.html('<span class="c-y">commit ' + cs2[0][0] + "</span>"); o.text("Author: " + cs2[0][1]); o.text("Date:   " + new Date(cs2[0][2]).toString().replace(/ GMT.*$/, "")); o.text(""); o.text("    " + cs2[0][3]);
        return;
      default:
        if (!sub) { o.text("usage: git [-v | --version] [-h | --help] <command> [<args>]"); return false; }
        o.text("git: '" + sub + "' is not a git command. See 'git --help'.");
        return false;
    }
  };
  S.diff = async function (path, o) {
    var full = FS.PROJ + "/" + path, now = (await FS.read(full)).split("\n"), was;
    try { was = ((await FS.original(full)) || "").split("\n"); } catch (e) { was = []; }
    // classic LCS, fine for files this size
    var n = was.length, m = now.length, dp = [];
    for (var i = 0; i <= n; i++) { dp.push(new Uint16Array(m + 1)); }
    for (i = n - 1; i >= 0; i--) for (var j = m - 1; j >= 0; j--) dp[i][j] = was[i] === now[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    var ops = []; i = 0; j = 0;
    while (i < n || j < m) {
      if (i < n && j < m && was[i] === now[j]) { ops.push([" ", was[i], i, j]); i++; j++; }
      else if (i < n && (j >= m || dp[i + 1][j] >= dp[i][j + 1])) { ops.push(["-", was[i], i, j]); i++; }
      else { ops.push(["+", now[j], i, j]); j++; }
    }
    o.html('<span class="B">diff --git a/' + esc(path) + " b/" + esc(path) + "</span>");
    o.html('<span class="B">--- a/' + esc(path) + "</span>"); o.html('<span class="B">+++ b/' + esc(path) + "</span>");
    var k = 0;
    while (k < ops.length) {
      if (ops[k][0] === " ") { k++; continue; }
      var s = Math.max(0, k - 3), e = k;
      while (e < ops.length) { var nxt = e; while (nxt < ops.length && ops[nxt][0] === " ") nxt++; if (nxt >= ops.length || nxt - e > 6) break; e = nxt; while (e < ops.length && ops[e][0] !== " ") e++; }
      e = Math.min(ops.length, e + 3);
      var hunk = ops.slice(s, e), a0 = hunk[0][2] + 1, b0 = hunk[0][3] + 1;
      var ac = hunk.filter(function (x) { return x[0] !== "+"; }).length, bc = hunk.filter(function (x) { return x[0] !== "-"; }).length;
      o.html('<span class="c-a">@@ -' + a0 + "," + ac + " +" + b0 + "," + bc + " @@</span>");
      hunk.forEach(function (x) { o.html(x[0] === "+" ? '<span class="c-g">+' + esc(x[1]) + "</span>" : x[0] === "-" ? '<span class="c-r">-' + esc(x[1]) + "</span>" : " " + esc(x[1])); });
      k = e;
    }
  };

  // ------------------------------------------------------------- cargo
  S.cargo = async function (a, o) {
    var sub = a[0], self = this;
    if (sub === "--version" || sub === "-V") { o.text("cargo 1.91.0 (ea2d97820 2025-10-10)"); return; }
    if (!sub) { o.text("Rust's package manager"); o.text(""); o.text("Usage: cargo [OPTIONS] [COMMAND]"); return; }
    if (!this.inRepo()) { o.html('<span class="c-r B">error</span>: could not find `Cargo.toml` in `' + esc(this.cwd) + "` or any parent directory"); return false; }
    var release = a.indexOf("--release") >= 0 || a.indexOf("-r") >= 0, profile = release ? "`release` profile [optimized]" : "`dev` profile [unoptimized + debuginfo]";
    var edited = FS.modified().some(function (m) { return /\.rs$|Cargo\.toml$/.test(m.path); });
    var build = async function () {
      if (edited) { o.html('<span class="c-g B">   Compiling</span> pacpad v0.1.0 (' + FS.PROJ + ")"); await self.sleep(1400); }
      o.html('<span class="c-g B">    Finished</span> ' + profile + " target(s) in " + (edited ? (release ? "9.84s" : "3.12s") : "0.07s"));
    };
    switch (sub) {
      case "build": case "b": case "check": case "c": case "clippy": await build(); return;
      case "fmt": return;
      case "run": case "r":
        await build();
        o.html('<span class="c-g B">     Running</span> `target/' + (release ? "release" : "debug") + "/pacpad" + (a.indexOf("--") >= 0 ? " " + a.slice(a.indexOf("--") + 1).join(" ") : "") + "`");
        return this.pacpad(a.indexOf("--") >= 0 ? a.slice(a.indexOf("--") + 1) : [], o);
      case "test": case "t": return this.cargoTest(a, o, build);
    }
    o.html('<span class="c-r B">error</span>: no such command: `' + esc(sub) + "`");
    return false;
  };
  S.cargoTest = async function (a, o, build) {
    var filter = a.slice(1).filter(function (x) { return x[0] !== "-"; })[0] || "";
    var files = [];
    FS.walk(FS.PROJ + "/src", function (n) { if (n.type === "file" && /\.rs$/.test(n.name)) files.push(n.path); });
    var note = this.print('<span class="c-f">reading ' + files.length + " source files from github.com/" + FS.OWNER + "/" + FS.REPO + "…</span>");
    var tests = [];
    var texts = await Promise.all(files.map(function (p) { return FS.read(p).then(function (t) { return [p, t]; }, function () { return [p, ""]; }); }));
    note.remove();
    texts.forEach(function (ft) {
      var mod = ft[0].slice((FS.PROJ + "/src/").length).replace(/\.rs$/, "").replace(/\/mod$/, "").replace(/^main$/, "").split("/").join("::");
      var re = /#\[test\]\s*(?:#\[[^\]]*\]\s*)*fn\s+(\w+)/g, m;
      while ((m = re.exec(ft[1]))) tests.push((mod ? mod + "::" : "") + "tests::" + m[1]);
    });
    if (!tests.length) { o.text("couldn't read the sources from GitHub right now."); return false; }
    await build();
    o.html('<span class="c-g B">     Running</span> unittests src/main.rs (target/debug/deps/pacpad-3f2b9c41e8d07a65)');
    o.text("");
    var run = tests.filter(function (t) { return t.indexOf(filter) >= 0; }).sort();
    o.text("running " + run.length + " test" + (run.length === 1 ? "" : "s"));
    for (var i = 0; i < run.length; i++) {
      if (this.cancel) return false;
      o.html("test " + esc(run[i]) + ' ... <span class="c-g">ok</span>');
      if (i % 6 === 5) await this.sleep(16);
    }
    o.text("");
    o.html('test result: <span class="c-g">ok</span>. ' + run.length + " passed; 0 failed; 0 ignored; 0 measured; " + (tests.length - run.length) + " filtered out; finished in 0.41s");
    o.html('<span class="c-f">(the names are the repo\'s real #[test] functions. the results are pretend: nothing compiles in a browser.)</span>');
  };

  // ------------------------------------------------------------- pacman / yay
  S.pacman = function (a, o, who) {
    var op = a.filter(function (x) { return x[0] === "-"; }).join("").replace(/-/g, ""), args = a.filter(function (x) { return x[0] !== "-"; });
    var all = PP.packages, line = function (p) { o.html('<span class="B">' + esc(p.name) + '</span> <span class="c-g B">' + esc(p.version) + "</span>"); };
    if (!a.length) {
      if (who === "pacman") { o.text("error: no operation specified (use -h for help)"); return false; }
      o.html('<span class="c-d">' + who + " would run a full system upgrade here. this page can't touch a real system, but the demo's u key shows what it looks like.</span>"); return false;
    }
    if (op[0] === "Q") {
      var f = op.slice(1);
      if (f.indexOf("i") >= 0) {
        if (!args.length) { all.forEach(line); return; }
        var ok = true;
        args.forEach(function (n) {
          var p = PP.find(n);
          if (!p || !p.installed) { o.text("error: package '" + n + "' was not found"); ok = false; return; }
          var kv = function (k, v) { o.html('<span class="B">' + esc(pad(k, 16)) + "</span>: " + esc(v)); };
          kv("Name", p.name); kv("Version", p.version); kv("Description", p.desc); kv("Architecture", "x86_64"); kv("URL", p.url);
          kv("Licenses", p.license); kv("Groups", p.groups ? p.groups.join("  ") : "None"); kv("Depends On", p.deps.join("  ") || "None");
          kv("Installed Size", (p.size >= 1 ? p.size.toFixed(2) + " MiB" : Math.round(p.size * 1024) + ".00 KiB"));
          kv("Install Date", new Date(p.date + "T12:00:00Z").toUTCString().replace(" GMT", ""));
          kv("Install Reason", p.reason === "explicit" ? "Explicitly installed" : "Installed as a dependency for another package");
          o.text("");
        });
        return ok;
      }
      if (f.indexOf("l") >= 0) {
        var p2 = PP.find(args[0]);
        if (!p2 || !p2.installed) { o.text("error: package '" + (args[0] || "") + "' was not found"); return false; }
        ["/usr/", "/usr/bin/", "/usr/bin/" + p2.name, "/usr/share/", "/usr/share/doc/" + p2.name + "/README.md", "/usr/share/licenses/" + p2.name + "/LICENSE", "/usr/share/man/man1/" + p2.name + ".1.gz"].forEach(function (x) { o.html('<span class="B">' + esc(p2.name) + "</span> " + esc(x)); });
        return;
      }
      if (f.indexOf("s") >= 0) return this.pacSearch(args.join(" "), o, true);
      var list = all.filter(function (p) {
        if (f.indexOf("e") >= 0 && p.reason !== "explicit") return false;
        if (f.indexOf("d") >= 0 && p.reason !== "dependency") return false;
        if (f.indexOf("t") >= 0 && !p.orphan) return false;
        if (f.indexOf("m") >= 0 && p.repo !== "aur") return false;
        if (f.indexOf("n") >= 0 && p.repo === "aur") return false;
        return !args.length || args.indexOf(p.name) >= 0;
      });
      if (args.length && !list.length) { o.text("error: package '" + args[0] + "' was not found"); return false; }
      list.forEach(f.indexOf("q") >= 0 ? function (p) { o.text(p.name); } : line);
      return list.length > 0;
    }
    if (op === "Ss" || op === "Ssq") return this.pacSearch(args.join(" "), o, false);
    if (op === "Si") {
      var p3 = PP.find(args[0]);
      if (!p3 || p3.repo === "aur") { o.text("error: package '" + (args[0] || "") + "' was not found"); return false; }
      o.html('<span class="B">Repository      </span>: ' + p3.repo); o.html('<span class="B">Name            </span>: ' + esc(p3.name)); o.html('<span class="B">Version         </span>: ' + esc(p3.version)); o.html('<span class="B">Description     </span>: ' + esc(p3.desc));
      return;
    }
    if (who === "pacman" && /^[SRUD]/.test(op)) { o.text("error: you cannot perform this operation unless you are root."); return false; }
    if (/^[SRU]/.test(op)) { o.html('<span class="c-d">' + who + " would ask for sudo here. this page can't touch a real system. pacpad would show you this exact command and ask first: try it in the demo.</span>"); return false; }
    o.text("error: invalid option '" + a[0] + "'");
    return false;
  };
  S.pacSearch = function (q, o, localOnly) {
    var hits = PP.search(q).filter(function (h) { return localOnly ? h.p.installed : h.p.repo !== "aur" || h.p.installed; });
    hits.forEach(function (h) {
      var p = h.p;
      o.html('<span class="c-m B">' + (localOnly ? "local" : p.repo === "aur" ? "aur" : p.repo) + '/</span><span class="B">' + esc(p.name) + '</span> <span class="c-g B">' + esc(p.version) + "</span>" + (p.installed && !localOnly ? ' <span class="c-a B">[installed]</span>' : "") + (p.groups ? ' <span class="c-a">(' + p.groups.join(" ") + ")</span>" : ""));
      o.text("    " + p.desc);
    });
    return hits.length > 0;
  };

  // ------------------------------------------------------------- pacpad's own CLI
  S.pacpad = function (a, o) {
    var dry = false;
    a = a.filter(function (x) { if (x === "--dry-run") { dry = true; return false; } return true; });
    var pc = a.indexOf("--pacman-conf"); if (pc >= 0) a.splice(pc, 2);
    var sub = a[0];
    if (!sub) { o.html('<span class="c-d">opening pacpad…</span>'); if (this.opts.onLaunch) this.opts.onLaunch(); return; }
    if (sub === "-h" || sub === "--help" || sub === "help") { HELP.forEach(function (l) { o.html(l); }); return; }
    if (sub === "-V" || sub === "--version") { o.text("pacpad 0.1.0"); return; }
    if (sub === "search") {
      var all = a.indexOf("--all") >= 0, q = a.slice(1).filter(function (x) { return x !== "--all"; }).join(" ");
      if (!q) return this.usage("search", "&lt;QUERY&gt;", o);
      var hits = PP.search(q).filter(function (h) { return h.p.repo !== "aur" || h.p.installed; }), limit = all ? hits.length : Math.min(20, hits.length);
      if (!hits.length) { o.text("no matches for " + JSON.stringify(q)); return; }
      o.text(hits.length + " match(es)" + (!all && hits.length > limit ? " (showing " + limit + ")" : "") + ":");
      hits.slice(0, limit).forEach(function (h) {
        var p = h.p, st = p.installed ? (p.repo === "aur" ? "installed, foreign" : "installed") : "not installed";
        var repo = p.repo === "aur" ? "-" : p.repo;
        o.html("  " + esc(pad(p.name, 24)) + " " + esc(pad(p.version, 14)) + " [" + pad(repo, 8) + "] " + '<span class="' + (p.installed ? "c-g" : "c-f") + '">' + st + "</span>");
        if (h.desc) o.html('<span class="c-d">      ↳ ' + esc(p.desc) + "</span>");
      });
      var c = PP.counts();
      o.html("");
      o.html('<span class="c-d">' + c.total + " installed (" + c.explicit + " explicit, " + c.dependency + " dependency, " + c.foreign + " foreign, " + c.orphan + " orphan)</span>");
      return;
    }
    if (sub === "webapp" || sub === "tui") return this.launchers(sub, a.slice(1), dry, o);
    o.html('<span class="c-r B">error:</span> unrecognized subcommand \'' + esc(sub) + "'");
    o.html(""); o.html('<span class="B">Usage:</span> pacpad [OPTIONS] [COMMAND]'); o.html("");
    o.html("For more information, try '<span class=\"B\">--help</span>'.");
    return false;
  };
  S.usage = function (sub, args, o) {
    o.html('<span class="c-r B">error:</span> the following required arguments were not provided:');
    o.html('  <span class="c-g">' + args + "</span>"); o.html("");
    o.html('<span class="B">Usage:</span> pacpad ' + sub + " " + args); o.html("");
    o.html("For more information, try '<span class=\"B\">--help</span>'.");
    return false;
  };
  S.launchers = function (kind, a, dry, o) {
    var act = a[0], mine = PP.apps.filter(function (x) { return x.kind === kind; });
    if (act === "list") {
      if (!mine.length) { o.text("(none)"); return; }
      mine.forEach(function (x) { o.text("  " + pad(x.name, 24) + " " + (x.url || x.command)); });
      return;
    }
    if (act === "add") {
      var tile = a.indexOf("--tile") >= 0, rest = a.slice(1).filter(function (x) { return x !== "--tile" && x !== "--float"; });
      var ii = rest.indexOf("--icon"); if (ii >= 0) rest.splice(ii, 2);
      var name = rest[0], target = rest[1];
      if (!name || !target) return this.usage(kind + " add", kind === "webapp" ? "&lt;NAME&gt; &lt;URL&gt;" : "&lt;NAME&gt; &lt;COMMAND&gt;", o);
      if (kind === "webapp" && !/^https?:\/\/\S+/.test(target)) { o.html('<span class="c-r B">Error:</span> url must start with http:// or https://'); return false; }
      if (dry) { o.text("[dry-run] would create " + (kind === "webapp" ? "webapp" : "tui app") + " " + JSON.stringify(name) + " (" + target + ")"); return; }
      var path = PP.addApp(kind === "webapp" ? { kind: "webapp", name: name, url: target } : { kind: "tui", name: name, command: target, style: tile ? "tile" : "float" });
      o.text("created " + path.replace("~", FS.HOME));
      o.html('<span class="c-f">→ it\'s in the demo\'s Apps tab now (press 3 there), and you can cat it: ' + this.tryLink("cat " + path) + "</span>");
      return;
    }
    if (act === "remove") {
      var n = a.slice(1).join(" ");
      if (!n) return this.usage(kind + " remove", "&lt;NAME&gt;", o);
      var found = mine.filter(function (x) { return x.name === n; })[0];
      if (!found) { o.html('<span class="c-r B">Error:</span> no pacpad-managed ' + (kind === "webapp" ? "webapp" : "tui app") + " named " + esc(JSON.stringify(n))); return false; }
      var p2 = FS.HOME + "/.local/share/applications/pacpad-" + PP.slug(n) + ".desktop";
      if (dry) { o.text("[dry-run] would remove " + p2); return; }
      PP.removeApp(kind, n);
      o.text("removed " + JSON.stringify(n));
      return;
    }
    o.html('<span class="B">Usage:</span> pacpad ' + kind + " &lt;COMMAND&gt;"); o.html("");
    o.html('<span class="B">Commands:</span>');
    o.html('  <span class="c-a">add</span>     ' + (kind === "webapp" ? "Create a Chrome app-mode launcher" : "Create a terminal launcher running `command`"));
    o.html('  <span class="c-a">remove</span>'); o.html('  <span class="c-a">list</span>');
    return false;
  };

  window.PPShell = { mount: function (el, opts) { return new Shell(el, opts); } };
})();
