// A small fake shell for the Install section. It first types out the real
// build steps by itself, then hands the prompt to the visitor, who can run
// pacpad's actual CLI (search / webapp / tui, --dry-run, --help) against the
// same in-memory system the live demo uses.
(function () {
  "use strict";

  var CRATES = ["libc v0.2.189", "proc-macro2 v1.0.107", "unicode-ident v1.0.24", "quote v1.0.47", "cfg-if v1.0.4",
    "memchr v2.8.3", "log v0.4.34", "itoa v1.0.18", "serde_core v1.0.229", "bitflags v2.9.4", "syn v2.0.106",
    "serde_derive v1.0.229", "serde v1.0.229", "unicode-width v0.2.2", "regex-syntax v0.8.6", "regex-automata v0.4.11",
    "rustix v1.1.2", "mio v1.0.4", "signal-hook v0.3.18", "crossterm v0.29.0", "ring v0.17.14", "rustls v0.23.44",
    "nucleo-matcher v0.3.1", "vte v0.15.0", "vt100 v0.16.2", "portable-pty v0.9.0", "toml v1.1.6+spec-1.1.0",
    "bincode v2.0.1", "ureq v3.4.1", "clap_builder v4.6.6", "clap v4.6.6", "ratatui-core v0.1.2",
    "ratatui-widgets v0.3.2", "ratatui-crossterm v0.1.2", "ratatui v0.30.2", "anyhow v1.0.104"];

  var HELP = [
    "A terminal app manager for Arch",
    "",
    "<b>Usage:</b> pacpad [OPTIONS] [COMMAND]",
    "",
    "<b>Commands:</b>",
    "  <a>search</a>  Search installed + repo packages by name or description",
    "  <a>webapp</a>  Manage pacpad-created webapp launchers",
    "  <a>tui</a>     Manage pacpad-created TUI-app launchers",
    "  <a>help</a>    Print this message or the help of the given subcommand(s)",
    "",
    "<b>Options:</b>",
    "      <a>--pacman-conf</a> &lt;PACMAN_CONF&gt;  Path to pacman.conf, for testing against a fixture without root [default: /etc/pacman.conf]",
    "      <a>--dry-run</a>                    Print the command any write action would run, and run nothing",
    "  <a>-h</a>, <a>--help</a>                       Print help",
    "  <a>-V</a>, <a>--version</a>                    Print version"
  ];

  var BUILTINS = [
    ["pacpad", "open the live demo above"],
    ["pacpad search &lt;query&gt; [--all]", "search installed + repo packages"],
    ["pacpad webapp add &lt;name&gt; &lt;url&gt;", "create a webapp launcher"],
    ["pacpad tui add &lt;name&gt; &lt;cmd&gt; [--tile]", "create a TUI-app launcher"],
    ["pacpad webapp|tui list|remove", "list or remove launchers"],
    ["pacpad --dry-run …", "print what would change, change nothing"],
    ["pacpad --help", "the real clap help text"],
    ["cat ~/.config/pacpad/config.toml", "the optional config file"],
    ["clear · history · ls · whoami · uname -a", "the usual"]
  ];

  function esc(s) { return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;"); }
  function pad(s, n) { s = String(s); return s.length >= n ? s : s + new Array(n - s.length + 1).join(" "); }

  // split a command line into words, honouring "double" and 'single' quotes
  function words(line) {
    var out = [], cur = "", q = null, has = false;
    for (var i = 0; i < line.length; i++) {
      var ch = line[i];
      if (q) { if (ch === q) q = null; else cur += ch; continue; }
      if (ch === '"' || ch === "'") { q = ch; has = true; continue; }
      if (/\s/.test(ch)) { if (cur || has) out.push(cur); cur = ""; has = false; continue; }
      cur += ch;
    }
    if (cur || has) out.push(cur);
    return out;
  }

  function Shell(el, opts) {
    this.el = el; this.opts = opts || {};
    this.out = el.querySelector(".sh-out");
    this.input = el.querySelector(".sh-in");
    this.line = el.querySelector(".sh-line");
    this.ghost = el.querySelector(".sh-typed");
    this.hist = []; this.hi = 0;
    this.cwd = "~";
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
  }
  var S = Shell.prototype;

  S.ps1 = function () {
    return '<span class="c-g">' + window.PP.user + "@" + window.PP.host + '</span> <span class="c-a">' + this.cwd + '</span> <span class="c-d">$</span> ';
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
  S.focus = function () { if (this.state === "ready") this.input.focus({ preventScroll: true }); };
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
    var rx = this.print("");
    for (var p = 0; p <= 100; p += 10) { rx.textContent = "Receiving objects: " + pad(p + "%", 4) + " (" + Math.round(p * 13.42) + "/1342)" + (p === 100 ? ", 412.18 KiB | 3.12 MiB/s, done." : ""); await this.sleep(45); }
    this.print("Resolving deltas: 100% (702/702), done.");
    this.cwd = "~/pacpad";
    await this.sleep(300);
    await this.typeCmd("cargo build --release");
    var self = this;
    var prog = null;
    for (var i = 0; i < CRATES.length; i++) {
      if (prog) prog.remove();
      this.print('<span class="c-g B">   Compiling</span> ' + esc(CRATES[i]));
      var k = Math.round(((i + 1) / (CRATES.length + 1)) * 26);
      prog = this.print('<span class="c-a B">    Building</span> [' + new Array(k + 1).join("=") + ">" + new Array(27 - k).join(" ") + "] " + (i + 1) * 5 + "/183: " + esc(CRATES[i].split(" ")[0]));
      await this.sleep(60 + Math.random() * 90);
    }
    prog.remove();
    this.print('<span class="c-g B">   Compiling</span> pacpad v0.1.0 (/home/' + window.PP.user + '/pacpad)');
    await this.sleep(900);
    this.print('<span class="c-g B">    Finished</span> `release` profile [optimized] target(s) in 41.20s');
    await this.sleep(400);
    await this.typeCmd("install -Dm755 target/release/pacpad ~/.local/bin/pacpad");
    await this.sleep(200);
    await this.typeCmd("pacpad --version");
    this.print("pacpad 0.1.0");
    this.print("");
    self.print('<span class="c-y">your turn.</span> <span class="c-d">try</span> <a class="sh-try" data-cmd="pacpad search editor">pacpad search editor</a><span class="c-d">,</span> <a class="sh-try" data-cmd="pacpad webapp add Notion https://notion.so">pacpad webapp add Notion https://notion.so</a> <span class="c-d">or</span> <a class="sh-try" data-cmd="help">help</a>');
  };

  S.ready = function () {
    this.state = "ready";
    this.skipping = false;
    this.line.hidden = false;
    this.line.querySelector(".sh-ps1").innerHTML = this.ps1();
    var self = this;
    this.out.querySelectorAll(".sh-try").forEach(function (a) { self.bindTry(a); });
    this.scroll();
    if (this.opts.onReady) this.opts.onReady();
  };
  S.bindTry = function (a) {
    var self = this;
    a.href = "#install";
    a.addEventListener("click", function (e) { e.preventDefault(); e.stopPropagation(); self.runTyped(a.dataset.cmd); });
  };
  // types a suggested command into the prompt, then runs it
  S.runTyped = async function (cmd) {
    if (this.state !== "ready" || this.busy) return;
    this.busy = true;
    this.input.value = "";
    for (var i = 0; i < cmd.length; i++) { this.input.value += cmd[i]; await this.sleep(18); }
    await this.sleep(160);
    this.busy = false;
    this.submit();
    this.focus();
  };

  // ------------------------------------------------------------- input
  S.onKey = function (e) {
    var k = e.key;
    e.stopPropagation();
    if (k === "Enter") { e.preventDefault(); this.submit(); }
    else if (k === "ArrowUp") { e.preventDefault(); if (this.hi > 0) { this.hi--; this.input.value = this.hist[this.hi]; } }
    else if (k === "ArrowDown") { e.preventDefault(); if (this.hi < this.hist.length) { this.hi++; this.input.value = this.hist[this.hi] || ""; } }
    else if (k === "Tab") { e.preventDefault(); this.complete(); }
    else if (k === "Escape") { this.input.blur(); if (this.opts.onBlur) this.opts.onBlur(); }
    else if ((k === "l" || k === "L") && e.ctrlKey) { e.preventDefault(); this.out.innerHTML = ""; }
    else if ((k === "c" || k === "C") && e.ctrlKey && !String(window.getSelection())) { e.preventDefault(); this.print(this.ps1() + esc(this.input.value) + "^C"); this.input.value = ""; }
  };

  S.complete = function () {
    var v = this.input.value, opts = ["pacpad search ", "pacpad webapp add ", "pacpad webapp list", "pacpad webapp remove ", "pacpad tui add ", "pacpad tui list", "pacpad tui remove ", "pacpad --help", "pacpad --version", "pacpad --dry-run ", "help", "clear", "cat ~/.config/pacpad/config.toml"];
    var m = opts.filter(function (o) { return o.indexOf(v) === 0 && o !== v; });
    if (m.length === 1) this.input.value = m[0];
    else if (m.length > 1) {
      var pre = m.reduce(function (a, b) { var i = 0; while (i < a.length && a[i] === b[i]) i++; return a.slice(0, i); });
      if (pre.length > v.length) this.input.value = pre;
      else { this.print(this.ps1() + esc(v)); this.print(m.map(function (x) { return esc(x.trim()); }).join("    "), "c-d"); }
    }
  };

  S.submit = function () {
    var line = this.input.value;
    this.input.value = "";
    this.print(this.ps1() + esc(line));
    if (line.trim()) { this.hist.push(line); }
    this.hi = this.hist.length;
    try { this.exec(line.trim()); } catch (err) { this.print("bash: " + esc(err.message), "c-r"); }
    this.line.querySelector(".sh-ps1").innerHTML = this.ps1();
    this.scroll();
    if (this.opts.onCommand) this.opts.onCommand(line);
  };

  // ------------------------------------------------------------- commands
  S.exec = function (line) {
    if (!line) return;
    var self = this, P = function (h, c) { self.print(h, c); };
    if (line.indexOf("sudo ") === 0) {
      P("[sudo] password for " + window.PP.user + ": ");
      P("Sorry, this is a website. Nothing here can run as root.", "c-m");
      return;
    }
    var w = words(line), cmd = w[0];
    switch (cmd) {
      case "clear": this.out.innerHTML = ""; return;
      case "help":
        P('<span class="c-y B">pacpad web shell</span> <span class="c-d">· runs against the same fake system as the demo above</span>');
        P("");
        BUILTINS.forEach(function (b) { P("  " + '<span class="c-a">' + pad(b[0], 46).replace(/&lt;|&gt;/g, function (m) { return m; }) + "</span>" + '<span class="c-d">' + b[1] + "</span>"); });
        P("");
        P('<span class="c-f">↑↓ history · tab completes · ctrl+l clears · esc leaves the prompt</span>');
        return;
      case "history": this.hist.forEach(function (h, i) { P(pad(i + 1, 5) + esc(h)); }); return;
      case "whoami": P(window.PP.user); return;
      case "hostname": P(window.PP.host); return;
      case "pwd": P(this.cwd.replace("~", "/home/" + window.PP.user)); return;
      case "uname": P(w[1] === "-a" ? "Linux " + window.PP.host + " 6.11.6-arch1-1 #1 SMP PREEMPT_DYNAMIC x86_64 GNU/Linux" : "Linux"); return;
      case "echo": P(esc(w.slice(1).join(" "))); return;
      case "date": P("Sat Nov  7 12:00:00 UTC 2026"); return;
      case "ls":
        P(this.cwd === "~/pacpad" ? '<span class="c-a B">src</span>  <span class="c-a B">site</span>  <span class="c-a B">target</span>  Cargo.lock  Cargo.toml  LICENSE  README.md' : '<span class="c-a B">pacpad</span>  <span class="c-a B">Downloads</span>  <span class="c-a B">notes</span>');
        return;
      case "cd":
        var to = w[1] || "~";
        if (to === "~" || to === "..") this.cwd = "~";
        else if ((to === "pacpad" || to === "~/pacpad") && this.cwd === "~" || to === "~/pacpad") this.cwd = "~/pacpad";
        else P("bash: cd: " + esc(to) + ": No such file or directory");
        return;
      case "cat": return this.cat(w[1]);
      case "exit": case "logout": P("there's no leaving. try <span class=\"c-a\">pacpad</span> instead.", "c-d"); return;
      case "pacman": case "yay": case "paru":
        P(esc(cmd) + ": this page can't touch a real system. pacpad would show you this exact command and ask first.", "c-d"); return;
      case "pacpad": return this.pacpad(w.slice(1));
      case "vim": case "nvim": case "nano": case "hx": P(esc(cmd) + ": nice choice, but not in a browser.", "c-d"); return;
    }
    P("bash: " + esc(cmd) + ": command not found");
  };

  S.cat = function (f) {
    var P = this.print.bind(this);
    if (!f) { P("cat: reading stdin is not supported here", "c-d"); return; }
    if (/config\.toml$/.test(f)) {
      ["<span class=\"c-f\"># ~/.config/pacpad/config.toml (every key is optional)</span>",
        "<span class=\"c-f\"># unset keys fall back to detection: xdg default browser, xdg-terminal-exec / $TERMINAL, then yay or paru</span>",
        '<span class="c-a">browser</span> = <span class="c-g">"chromium"</span>',
        '<span class="c-a">terminal</span> = <span class="c-g">"ghostty"</span>',
        '<span class="c-a">aur_helper</span> = <span class="c-g">"yay"</span>'].forEach(function (l) { P(l); });
      return;
    }
    if (/README/.test(f) && this.cwd === "~/pacpad") { P("# pacpad"); P(""); P("An omarchy-style terminal app manager for Arch Linux — browse, search, and manage"); P("packages (repo + AUR), plus create and remove Chrome webapp and TUI-app launchers,"); P("all from one ratatui TUI."); return; }
    if (/Cargo\.toml/.test(f) && this.cwd === "~/pacpad") { P("[package]"); P('name = "pacpad"'); P('version = "0.1.0"'); P('edition = "2021"'); P('rust-version = "1.82"'); P('license = "Apache-2.0"'); return; }
    P("cat: " + esc(f) + ": No such file or directory");
  };

  S.pacpad = function (a) {
    var P = this.print.bind(this), PP = window.PP;
    var dry = false;
    a = a.filter(function (x) { if (x === "--dry-run") { dry = true; return false; } return true; });
    var pc = a.indexOf("--pacman-conf"); if (pc >= 0) a.splice(pc, 2);
    var sub = a[0];
    if (!sub) { P('<span class="c-d">opening pacpad…</span>'); if (this.opts.onLaunch) this.opts.onLaunch(); return; }
    if (sub === "-h" || sub === "--help" || sub === "help") { HELP.forEach(function (l) { P(l.replace(/<a>/g, '<span class="c-a">').replace(/<\/a>/g, "</span>").replace(/<b>/g, '<span class="B">').replace(/<\/b>/g, "</span>")); }); return; }
    if (sub === "-V" || sub === "--version") { P("pacpad 0.1.0"); return; }
    if (sub === "search") {
      var all = a.indexOf("--all") >= 0, q = a.slice(1).filter(function (x) { return x !== "--all"; }).join(" ");
      if (!q) return this.usage("search", "&lt;QUERY&gt;");
      var hits = PP.search(q), limit = all ? hits.length : Math.min(20, hits.length);
      if (!hits.length) { P("no matches for " + esc(JSON.stringify(q))); return; }
      P(hits.length + " match(es)" + (!all && hits.length > limit ? " (showing " + limit + ")" : "") + ":");
      hits.slice(0, limit).forEach(function (h) {
        var p = h.p, st = p.installed ? (p.repo === "aur" ? "installed, foreign" : "installed") : "not installed";
        var repo = p.repo === "aur" ? "-" : p.repo;
        P("  " + esc(pad(p.name, 24)) + " " + esc(pad(p.version, 14)) + " [" + pad(repo, 8) + "] " + '<span class="' + (p.installed ? "c-g" : "c-f") + '">' + st + "</span>");
        if (h.desc) P('<span class="c-d">      ↳ ' + esc(p.desc) + "</span>");
      });
      var c = PP.counts();
      P("");
      P('<span class="c-d">' + c.total + " installed (" + c.explicit + " explicit, " + c.dependency + " dependency, " + c.foreign + " foreign, " + c.orphan + " orphan)</span>");
      return;
    }
    if (sub === "webapp" || sub === "tui") return this.launchers(sub, a.slice(1), dry);
    P('<span class="c-r B">error:</span> unrecognized subcommand \'' + esc(sub) + "'");
    P("");
    P("<span class=\"B\">Usage:</span> pacpad [OPTIONS] [COMMAND]");
    P("");
    P("For more information, try '<span class=\"B\">--help</span>'.");
  };

  S.usage = function (sub, args) {
    var P = this.print.bind(this);
    P('<span class="c-r B">error:</span> the following required arguments were not provided:');
    P('  <span class="c-g">' + args + "</span>");
    P("");
    P("<span class=\"B\">Usage:</span> pacpad " + sub + " " + args);
    P("");
    P("For more information, try '<span class=\"B\">--help</span>'.");
  };

  S.launchers = function (kind, a, dry) {
    var P = this.print.bind(this), PP = window.PP, act = a[0];
    var mine = PP.apps.filter(function (x) { return x.kind === kind; });
    if (act === "list") {
      if (!mine.length) { P("(none)"); return; }
      mine.forEach(function (x) { P("  " + esc(pad(x.name, 24)) + " " + esc(x.url || x.command)); });
      return;
    }
    if (act === "add") {
      var tile = a.indexOf("--tile") >= 0, rest = a.slice(1).filter(function (x) { return x !== "--tile" && x !== "--float"; });
      var ii = rest.indexOf("--icon"); if (ii >= 0) rest.splice(ii, 2);
      var name = rest[0], target = rest[1];
      if (!name || !target) return this.usage(kind + " add", kind === "webapp" ? "&lt;NAME&gt; &lt;URL&gt;" : "&lt;NAME&gt; &lt;COMMAND&gt;");
      if (kind === "webapp" && !/^https?:\/\/\S+/.test(target)) { P('<span class="c-r B">Error:</span> url must start with http:// or https://'); return; }
      if (dry) { P("[dry-run] would create " + (kind === "webapp" ? "webapp" : "tui app") + " " + esc(JSON.stringify(name)) + " (" + esc(target) + ")"); return; }
      var path = PP.addApp(kind === "webapp" ? { kind: "webapp", name: name, url: target } : { kind: "tui", name: name, command: target, style: tile ? "tile" : "float" });
      P("created " + esc(path.replace("~", "/home/" + PP.user)));
      P('<span class="c-f">→ it\'s in the demo\'s Apps tab now. press 3 there to see it.</span>');
      return;
    }
    if (act === "remove") {
      var n = a.slice(1).join(" ");
      if (!n) return this.usage(kind + " remove", "&lt;NAME&gt;");
      var found = mine.filter(function (x) { return x.name === n; })[0];
      if (!found) { P('<span class="c-r B">Error:</span> no pacpad-managed ' + (kind === "webapp" ? "webapp" : "tui app") + " named " + esc(JSON.stringify(n))); return; }
      var p2 = "/home/" + PP.user + "/.local/share/applications/pacpad-" + PP.slug(n) + ".desktop";
      if (dry) { P("[dry-run] would remove " + esc(p2)); return; }
      PP.removeApp(kind, n);
      P("removed " + esc(JSON.stringify(n)));
      return;
    }
    P("<span class=\"B\">Usage:</span> pacpad " + kind + " &lt;COMMAND&gt;");
    P("");
    P("<span class=\"B\">Commands:</span>");
    P('  <span class="c-a">add</span>     ' + (kind === "webapp" ? "Create a Chrome app-mode launcher" : "Create a terminal launcher running `command`"));
    P('  <span class="c-a">remove</span>');
    P('  <span class="c-a">list</span>');
  };

  window.PPShell = { mount: function (el, opts) { return new Shell(el, opts); } };
})();
