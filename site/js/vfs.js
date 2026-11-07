// An in-memory filesystem for the web shell. ~/pacpad is the real
// ric-v/pacpad repository: the tree comes from the GitHub API (with a
// bundled snapshot as fallback) and file contents are fetched from
// raw.githubusercontent.com on first read. Everything else under / is
// generated from the same fake system the demo uses, so installing a
// package or adding a launcher there shows up here too. Writes only ever
// land in this browser tab.
(function () {
  "use strict";

  var OWNER = "ric-v", REPO = "pacpad", REF = "main";
  var USER = window.PP.user, HOME = "/home/" + USER, PROJ = HOME + "/pacpad";
  var TODAY = "2026-11-07T12:00:00Z";

  var nodes = {};          // path -> node
  var repoCommits = null;  // [[sha, author, isoDate, subject]]
  var repoDate = "2026-09-13T22:57:36+05:30";
  var readyP = null;

  function node(path, type, extra) {
    var n = { path: path, type: type, name: path === "/" ? "/" : path.slice(path.lastIndexOf("/") + 1), size: type === "dir" ? 4096 : 0, mtime: TODAY, mode: type === "dir" ? "drwxr-xr-x" : "-rw-r--r--", kids: type === "dir" ? {} : null };
    if (extra) for (var k in extra) n[k] = extra[k];
    return n;
  }
  function parent(p) { var i = p.lastIndexOf("/"); return i <= 0 ? "/" : p.slice(0, i); }
  function mkdirp(p, extra) {
    if (nodes[p]) return nodes[p];
    if (p !== "/") mkdirp(parent(p));
    var n = nodes[p] = node(p, "dir", extra);
    if (p !== "/") nodes[parent(p)].kids[n.name] = true;
    return n;
  }
  function addFile(p, extra) {
    mkdirp(parent(p));
    var n = nodes[p] = node(p, "file", extra);
    nodes[parent(p)].kids[n.name] = true;
    return n;
  }
  function removeNode(p) {
    var n = nodes[p];
    if (!n) return;
    if (n.type === "dir") Object.keys(n.kids).forEach(function (k) { removeNode(p + "/" + k); });
    delete nodes[p];
    if (nodes[parent(p)]) delete nodes[parent(p)].kids[n.name];
  }

  var BINARY = /\.(woff2?|png|jpe?g|gif|ico|db|bin|iso|gz|zst|tar|so)$/i;
  function isBinary(p) { var n = nodes[p]; return !!(n && (n.binary || BINARY.test(p))); }

  // ------------------------------------------------------------- the static world
  function skeleton() {
    ["/bin", "/boot", "/dev", "/etc", "/home", "/mnt", "/opt", "/proc", "/root", "/run", "/srv", "/sys", "/tmp", "/usr/bin", "/usr/lib", "/usr/share", "/var/cache", "/var/log", "/var/lib/pacman/sync"].forEach(function (d) { mkdirp(d); });
    nodes["/root"].mode = "drwx------"; nodes["/root"].owner = "root";
    nodes["/tmp"].mode = "drwxrwxrwt";
    mkdirp(HOME, { owner: USER });
    gen("/etc/pacman.conf", function () {
      return ["#", "# /etc/pacman.conf", "#", "# See the pacman.conf(5) manpage for option directives", "", "[options]", "HoldPkg     = pacman glibc", "Architecture = auto", "Color", "CheckSpace", "VerbosePkgLists", "ParallelDownloads = 5", "SigLevel    = Required DatabaseOptional", "LocalFileSigLevel = Optional", "", "[core]", "Include = /etc/pacman.d/mirrorlist", "", "[extra]", "Include = /etc/pacman.d/mirrorlist", ""].join("\n");
    }, { owner: "root" });
    gen("/etc/pacman.d/mirrorlist", function () { return "## Arch Linux repository mirrorlist\n\nServer = https://geo.mirror.pkgbuild.com/$repo/os/$arch\n"; }, { owner: "root" });
    gen("/etc/os-release", function () {
      return 'NAME="Arch Linux"\nPRETTY_NAME="Arch Linux"\nID=arch\nBUILD_ID=rolling\nANSI_COLOR="38;2;23;147;209"\nHOME_URL="https://archlinux.org/"\nDOCUMENTATION_URL="https://wiki.archlinux.org/"\nSUPPORT_URL="https://bbs.archlinux.org/"\nBUG_REPORT_URL="https://gitlab.archlinux.org/groups/archlinux/-/issues"\nPRIVACY_POLICY_URL="https://terms.archlinux.org/docs/privacy-policy/"\nLOGO=archlinux-logo\n';
    }, { owner: "root" });
    gen("/etc/hostname", function () { return window.PP.host + "\n"; }, { owner: "root" });
    ["core", "extra"].forEach(function (r) { addFile("/var/lib/pacman/sync/" + r + ".db", { binary: true, size: r === "core" ? 131072 : 8126464, owner: "root" }); });
    ["bash", "cat", "ls", "nano", "vim", "less", "grep", "git", "pacman", "sudo", "yay", "cargo"].forEach(function (b) { addFile("/usr/bin/" + b, { binary: true, mode: "-rwxr-xr-x", size: 180224, owner: "root" }); });

    gen(HOME + "/.bashrc", function () {
      return "#\n# ~/.bashrc\n#\n\n# If not running interactively, don't do anything\n[[ $- != *i* ]] && return\n\nalias ls='ls --color=auto'\nalias grep='grep --color=auto'\nPS1='\\[\\e[32m\\]\\u@\\h\\[\\e[0m\\] \\[\\e[34m\\]\\w\\[\\e[0m\\] $ '\n\nexport PATH=\"$HOME/.local/bin:$PATH\"\nexport EDITOR=nano\n";
    });
    gen(HOME + "/.config/pacpad/config.toml", function () {
      return "# ~/.config/pacpad/config.toml (every key is optional)\n# unset keys fall back to detection: xdg default browser, xdg-terminal-exec / $TERMINAL, then yay or paru\nbrowser = \"chromium\"\nterminal = \"ghostty\"\naur_helper = \"yay\"\n";
    });
    gen(HOME + "/notes/todo.md", function () {
      return "# pacpad\n\n- [x] Installed, Search and Apps tabs\n- [x] AUR delegation, embedded terminal, protected-package guard, --dry-run\n- [x] webapp + TUI launchers from the TUI and the CLI\n- [x] website\n- [ ] Updates tab: version-by-version list of pending updates\n- [ ] AUR PKGBUILD, CI and release binaries\n";
    });
    mkdirp(HOME + "/Downloads");
    addFile(HOME + "/Downloads/archlinux-2026.11.01-x86_64.iso", { binary: true, size: 1288699904 });
    addFile(HOME + "/.local/bin/pacpad", { binary: true, mode: "-rwxr-xr-x", size: 6914048 });
    addFile(HOME + "/.cache/pacpad/index.bin", { binary: true, size: 1048576 });
    regen();
    window.PP.onChange(regen);
  }
  function gen(p, fn, extra) { var n = addFile(p, extra); n.gen = fn; return n; }

  // the parts that track the demo: launchers, icons and the local package db
  function regen() {
    var PP = window.PP, apps = HOME + "/.local/share/applications", icons = HOME + "/.local/share/pacpad/icons", local = "/var/lib/pacman/local";
    [apps, icons, local].forEach(function (d) { if (nodes[d]) Object.keys(nodes[d].kids).forEach(function (k) { if (!(nodes[d + "/" + k] && nodes[d + "/" + k].mem)) removeNode(d + "/" + k); }); });
    mkdirp(apps); mkdirp(icons); mkdirp(local);
    PP.apps.forEach(function (a) {
      var slug = PP.slug(a.name), cls = a.kind === "webapp" ? "pacpad-" + slug : "pacpad-tui-" + (a.style || "float");
      var icon = a.kind === "webapp" ? icons + "/" + slug + ".png" : null;
      if (icon) addFile(icon, { binary: true, size: 4318, mtime: a.created + "T12:00:00Z" });
      var f = gen(apps + "/pacpad-" + slug + ".desktop", function () {
        var exec = a.kind === "webapp" ? "/usr/bin/chromium --app=" + a.url + " --class=" + cls + " --name=" + a.name : "xdg-terminal-exec --app-id=" + cls + " -e sh -c \"" + a.command + "\"";
        var out = ["[Desktop Entry]", "Version=1.0", "Type=Application", "Name=" + a.name, "Comment=" + a.name, "Exec=" + exec];
        if (icon) out.push("Icon=" + icon);
        out.push("Terminal=false", "StartupNotify=true", "StartupWMClass=" + cls, "X-PacPad-Managed=true", "X-PacPad-Kind=" + a.kind);
        if (a.url) out.push("X-PacPad-Url=" + a.url);
        if (a.command) out.push("X-PacPad-Command=" + a.command);
        if (icon) out.push("X-PacPad-Icon=" + icon);
        out.push("X-PacPad-Created=" + a.created);
        return out.join("\n") + "\n";
      }, { mtime: a.created + "T12:00:00Z", mode: "-rwxr-xr-x" });
      f.size = f.gen().length;
    });
    PP.packages.forEach(function (p) {
      var dir = local + "/" + p.name + "-" + p.version;
      mkdirp(dir, { owner: "root", mtime: p.date + "T12:00:00Z" });
      var d = gen(dir + "/desc", function () {
        var o = ["%NAME%", p.name, "", "%VERSION%", p.version, "", "%BASE%", p.name, "", "%DESC%", p.desc, "", "%URL%", p.url, "", "%ARCH%", "x86_64", "", "%INSTALLDATE%", String(Math.floor(Date.parse(p.date + "T12:00:00Z") / 1000)), "", "%SIZE%", String(Math.round(p.size * 1048576)), ""];
        if (p.groups) o.push("%GROUPS%", p.groups.join("\n"), "");
        if (p.reason === "dependency") o.push("%REASON%", "1", "");
        o.push("%LICENSE%", p.license, "");
        if (p.deps.length) o.push("%DEPENDS%", p.deps.join("\n"), "");
        return o.join("\n");
      }, { owner: "root", mtime: p.date + "T12:00:00Z" });
      d.size = d.gen().length;
    });
  }

  // ------------------------------------------------------------- the repo
  function loadRepo(files, live) {
    mkdirp(PROJ, { repo: true });
    files.forEach(function (f) {
      var p = PROJ + "/" + f[0];
      addFile(p, { repo: f[0], size: f[1], mtime: repoDate, mode: /\.sh$/.test(f[0]) ? "-rwxr-xr-x" : "-rw-r--r--" });
      var d = parent(p);
      while (d !== PROJ && nodes[d]) { nodes[d].mtime = repoDate; d = parent(d); }
    });
    mkdirp(PROJ + "/.git");
    gen(PROJ + "/.git/HEAD", function () { return "ref: refs/heads/" + REF + "\n"; });
    gen(PROJ + "/.git/config", function () { return "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n\tlogallrefupdates = true\n[remote \"origin\"]\n\turl = https://github.com/" + OWNER + "/" + REPO + "\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n[branch \"" + REF + "\"]\n\tremote = origin\n\tmerge = refs/heads/" + REF + "\n"; });
    addFile(PROJ + "/target/release/pacpad", { binary: true, mode: "-rwxr-xr-x", size: 6914048 });
    nodes[PROJ].live = live;
  }

  function fetchJSON(url, ms) {
    return new Promise(function (res, rej) {
      var t = setTimeout(function () { rej(new Error("timeout")); }, ms || 5000);
      fetch(url, { headers: { Accept: "application/vnd.github+json" } }).then(function (r) {
        clearTimeout(t);
        if (!r.ok) return rej(new Error("HTTP " + r.status));
        r.json().then(res, rej);
      }, function (e) { clearTimeout(t); rej(e); });
    });
  }

  function init() {
    if (readyP) return readyP;
    var snap = window.PP_REPO_SNAPSHOT || { files: [], commits: [] };
    readyP = fetchJSON("https://api.github.com/repos/" + OWNER + "/" + REPO + "/git/trees/" + REF + "?recursive=1").then(function (j) {
      var files = j.tree.filter(function (e) { return e.type === "blob"; }).map(function (e) { return [e.path, e.size]; });
      if (!files.length) throw new Error("empty tree");
      loadRepo(files, true);
    }).catch(function () { loadRepo(snap.files, false); }).then(function () { return api; });
    return readyP;
  }

  // ------------------------------------------------------------- reading
  var cache = {};
  function read(p) {
    var n = nodes[p];
    if (!n) return Promise.reject(new Error("No such file or directory"));
    if (n.type === "dir") return Promise.reject(new Error("Is a directory"));
    if (n.mem !== undefined) return Promise.resolve(n.mem);
    if (n.gen) return Promise.resolve(n.gen());
    if (n.binary || BINARY.test(p)) return Promise.reject(new Error("binary file"));
    if (n.repo) {
      if (!cache[p]) cache[p] = fetch("https://raw.githubusercontent.com/" + OWNER + "/" + REPO + "/" + REF + "/" + n.repo.split("/").map(encodeURIComponent).join("/")).then(function (r) {
        if (!r.ok) throw new Error(r.status === 404 ? "not on " + REF + " yet" : "HTTP " + r.status);
        return r.text();
      }).catch(function (e) { delete cache[p]; throw new Error("couldn't fetch from GitHub (" + e.message + ")"); });
      return cache[p];
    }
    return Promise.resolve("");
  }
  function original(p) { var n = nodes[p]; if (!n || !n.repo) return Promise.resolve(null); var keep = n.mem; delete n.mem; var r = read(p); if (keep !== undefined) n.mem = keep; return r; }

  // ------------------------------------------------------------- writing (this tab only)
  function writable(p) { return p === HOME || p.indexOf(HOME + "/") === 0 || p === "/tmp" || p.indexOf("/tmp/") === 0; }
  function write(p, text) {
    if (!writable(p)) throw new Error("Permission denied");
    var n = nodes[p];
    if (n && n.type === "dir") throw new Error("Is a directory");
    if (!nodes[parent(p)] || nodes[parent(p)].type !== "dir") throw new Error("No such file or directory");
    if (!n) { n = addFile(p); n.created = true; }
    if (n.repo && n.mem === undefined && n.origSize === undefined) { n.origSize = n.size; n.origMtime = n.mtime; }
    n.mem = text; n.size = new Blob([text]).size; n.mtime = new Date().toISOString(); n.gen = null;
    return n;
  }
  function mkdir(p, parents) {
    if (!writable(p)) throw new Error("Permission denied");
    if (nodes[p]) throw new Error("File exists");
    if (!nodes[parent(p)] && !parents) throw new Error("No such file or directory");
    mkdirp(p, { mem: undefined, created: true });
  }
  function rm(p) {
    if (!writable(p)) throw new Error("Permission denied");
    removeNode(p);
  }

  // ------------------------------------------------------------- paths
  function resolve(cwd, p) {
    if (!p || p === "~") return HOME;
    if (p.indexOf("~/") === 0) p = HOME + p.slice(1);
    else if (p[0] !== "/") p = (cwd === "/" ? "" : cwd) + "/" + p;
    var out = [];
    p.split("/").forEach(function (s) { if (!s || s === ".") return; if (s === "..") out.pop(); else out.push(s); });
    return "/" + out.join("/");
  }
  function pretty(p) { return p === HOME ? "~" : p.indexOf(HOME + "/") === 0 ? "~" + p.slice(HOME.length) : p; }
  function list(p, all) {
    var n = nodes[p];
    if (!n || n.type !== "dir") return [];
    return Object.keys(n.kids).filter(function (k) { return all || k[0] !== "."; }).sort(function (a, b) {
      return a.replace(/^\./, "").toLowerCase() < b.replace(/^\./, "").toLowerCase() ? -1 : 1;
    }).map(function (k) { return nodes[(p === "/" ? "" : p) + "/" + k]; });
  }
  function walk(p, fn) {
    var n = nodes[p]; if (!n) return;
    fn(n);
    if (n.type === "dir") list(p, true).forEach(function (c) { if (c.name !== ".git") walk(c.path, fn); });
  }
  function modified() {
    var out = [];
    Object.keys(nodes).forEach(function (k) {
      var n = nodes[k];
      if (k.indexOf(PROJ + "/") !== 0 || k.indexOf(PROJ + "/.git") === 0 || n.type !== "file") return;
      if (n.repo && n.mem !== undefined) out.push({ path: n.repo, kind: "modified" });
      else if (!n.repo && n.created) out.push({ path: k.slice(PROJ.length + 1), kind: "untracked" });
    });
    return out.sort(function (a, b) { return a.path < b.path ? -1 : 1; });
  }
  function commits() {
    if (repoCommits) return Promise.resolve(repoCommits);
    var snap = (window.PP_REPO_SNAPSHOT || {}).commits || [];
    return fetchJSON("https://api.github.com/repos/" + OWNER + "/" + REPO + "/commits?sha=" + REF + "&per_page=30").then(function (j) {
      repoCommits = j.map(function (c) { return [c.sha, c.commit.author.name, c.commit.author.date, c.commit.message.split("\n")[0]]; });
      return repoCommits;
    }).catch(function () { repoCommits = snap; return snap; });
  }

  var api = {
    HOME: HOME, PROJ: PROJ, REF: REF, OWNER: OWNER, REPO: REPO,
    init: init, stat: function (p) { return nodes[p] || null; }, list: list, read: read, original: original,
    write: write, mkdir: mkdir, rm: rm, resolve: resolve, pretty: pretty, walk: walk, isBinary: isBinary,
    modified: modified, commits: commits, writable: writable,
    url: function (p) { var n = nodes[p]; return n && n.repo ? "https://github.com/" + OWNER + "/" + REPO + "/blob/" + REF + "/" + n.repo : null; },
    live: function () { return !!(nodes[PROJ] && nodes[PROJ].live); }
  };
  skeleton();
  window.PPFS = api;
})();
