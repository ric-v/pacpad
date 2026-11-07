// Shared, in-memory "system" that both the live demo and the shell
// simulation read and change. Package names, versions and descriptions are
// realistic Arch values; nothing here touches a real machine.
(function () {
  "use strict";

  var P = function (name, version, repo, size, reason, date, license, desc, deps, url, extra) {
    var p = { name: name, version: version, repo: repo, size: size, reason: reason, date: date,
      license: license, desc: desc, deps: deps || [], url: url || "", installed: !!reason };
    if (extra) for (var k in extra) p[k] = extra[k];
    return p;
  };

  // installed packages (reason: "explicit" | "dependency"; orphan = dependency nothing needs)
  var packages = [
    P("btop", "1.4.0-1", "extra", 1.8, "explicit", "2026-09-14", "Apache-2.0", "A monitor of system resources, bpytop ported to C++", ["gcc-libs", "glibc"], "https://github.com/aristocratos/btop"),
    P("firefox", "132.0-1", "extra", 251, "explicit", "2026-10-30", "MPL-2.0", "Fast, Private & Safe Web Browser", ["dbus-glib", "ffmpeg", "gtk3", "libpulse", "nss", "ttf-font"], "https://www.mozilla.org/firefox/"),
    P("git", "2.47.0-1", "extra", 27.4, "explicit", "2026-10-08", "GPL-2.0-only", "the fast distributed version control system", ["curl", "expat", "perl", "pcre2", "zlib"], "https://git-scm.com/"),
    P("glibc", "2.40+r16-2", "core", 48.1, "dependency", "2026-08-02", "GPL-2.0-or-later", "GNU C Library", ["linux-api-headers", "tzdata", "filesystem"], "https://www.gnu.org/software/libc", { groups: ["base"] }),
    P("go", "2:1.23.2-1", "extra", 238.4, "dependency", "2026-09-21", "BSD-3-Clause", "Core compiler tools for the Go programming language", [], "https://go.dev/", { orphan: true }),
    P("linux", "6.11.6.arch1-1", "core", 136, "explicit", "2026-11-01", "GPL-2.0-only", "The Linux kernel and modules", ["coreutils", "initramfs", "kmod"], "https://github.com/archlinux/linux"),
    P("neovim", "0.10.2-1", "extra", 27.9, "explicit", "2026-10-12", "Apache-2.0", "Fork of Vim aiming to improve user experience, plugins, and GUIs", ["libuv", "luajit", "tree-sitter", "unibilium"], "https://neovim.io"),
    P("pacman", "7.0.0.r3-1", "core", 4.9, "explicit", "2026-07-19", "GPL-2.0-or-later", "A library-based package manager with dependency support", ["bash", "curl", "gpgme", "libarchive"], "https://www.archlinux.org/pacman/"),
    P("python-pip", "24.2-1", "extra", 13.1, "dependency", "2026-06-02", "MIT", "The PyPA recommended tool for installing Python packages", ["python"], "https://pip.pypa.io/", { orphan: true }),
    P("ripgrep", "14.1.1-1", "extra", 4.6, "explicit", "2026-10-12", "MIT, Unlicense", "Searches directories for a regex, respecting .gitignore", ["gcc-libs", "pcre2"], "https://github.com/BurntSushi/ripgrep"),
    P("spotify", "1:1.2.48-1", "aur", 312, "explicit", "2026-10-21", "custom", "A proprietary music streaming service", ["alsa-lib", "gtk3", "libxss", "nss"], "https://www.spotify.com"),
    P("sudo", "1.9.16-1", "core", 7.6, "dependency", "2026-08-02", "custom", "Give certain users the ability to run some commands as root", ["glibc", "libgcrypt", "pam"], "https://www.sudo.ws/"),
    P("systemd", "256.7-1", "core", 29.8, "dependency", "2026-10-03", "GPL-2.0-or-later", "system and service manager", ["acl", "dbus", "kmod", "libcap"], "https://systemd.io", { groups: ["base"] }),
    P("yay", "12.4.2-1", "aur", 8.9, "explicit", "2026-07-20", "GPL-3.0-or-later", "Yet another yogurt. Pacman wrapper and AUR helper written in go.", ["git", "pacman"], "https://github.com/Jguer/yay"),
    P("zstd", "1.5.6-1", "core", 2.3, "dependency", "2026-05-11", "BSD-3-Clause", "Zstandard - Fast real-time compression algorithm", ["glibc", "gcc-libs", "lz4", "xz"], "https://facebook.github.io/zstd/")
  ];

  // available but not installed
  var available = [
    P("bat", "0.24.0-3", "extra", 6.1, null, "", "Apache-2.0", "Cat clone with syntax highlighting and git integration", ["gcc-libs", "libgit2", "oniguruma"], "https://github.com/sharkdp/bat"),
    P("eza", "0.20.6-1", "extra", 3.2, null, "", "MIT", "A modern replacement for ls", ["gcc-libs", "libgit2"], "https://github.com/eza-community/eza"),
    P("fd", "10.2.0-1", "extra", 3.9, null, "", "Apache-2.0", "Simple, fast and user-friendly alternative to find", ["gcc-libs"], "https://github.com/sharkdp/fd"),
    P("fzf", "0.56.0-1", "extra", 4.1, null, "", "MIT", "Command-line fuzzy finder", ["bash"], "https://github.com/junegunn/fzf"),
    P("helix", "24.07-1", "extra", 84.2, null, "", "MPL-2.0", "A post-modern modal text editor", ["gcc-libs", "glibc"], "https://helix-editor.com"),
    P("lazygit", "0.44.1-1", "extra", 18.2, null, "", "MIT", "Simple terminal UI for git commands", ["git"], "https://github.com/jesseduffield/lazygit"),
    P("obsidian", "1.7.5-1", "extra", 286, null, "", "custom", "A powerful knowledge base that works on top of a local folder of plain text Markdown files", ["electron32"], "https://obsidian.md"),
    P("ripgrep-all", "0.10.6-1", "extra", 9.4, null, "", "AGPL-3.0-or-later", "rga: ripgrep, but also search in PDFs, E-Books, Office documents, zip, tar.gz, etc.", ["ripgrep"], "https://github.com/phiresky/ripgrep-all"),
    P("starship", "1.21.1-1", "extra", 9.7, null, "", "ISC", "The cross-shell prompt for astronauts", ["gcc-libs", "zlib"], "https://starship.rs"),
    P("zellij", "0.41.1-1", "extra", 35.2, null, "", "MIT", "A terminal multiplexer with batteries included", ["gcc-libs", "glibc", "libgit2"], "https://zellij.dev"),
    P("zoxide", "0.9.6-1", "extra", 1.4, null, "", "MIT", "A smarter cd command for your terminal", ["gcc-libs"], "https://github.com/ajeetdsouza/zoxide")
  ];

  var apps = [
    { kind: "webapp", name: "Linear", url: "https://linear.app", created: "2026-11-02" },
    { kind: "webapp", name: "Excalidraw", url: "https://excalidraw.com", created: "2026-10-18" },
    { kind: "tui", name: "btop", command: "btop", style: "float", created: "2026-10-18" }
  ];

  var listeners = [];
  function changed(what) { listeners.forEach(function (fn) { try { fn(what); } catch (e) {} }); }

  function slug(name) { return name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, ""); }

  var PROTECTED = ["pacman", "sudo", "systemd", "glibc"];
  function protection(p) {
    if (!p || !p.installed) return null;
    if (PROTECTED.indexOf(p.name) >= 0) return "a critical system package";
    if (p.name.indexOf("linux") === 0) return "a kernel package";
    if (p.groups && p.groups.indexOf("base") >= 0) return "a member of the base group";
    return null;
  }

  function all() { return packages.concat(available); }
  function find(name) { var a = all(); for (var i = 0; i < a.length; i++) if (a[i].name === name) return a[i]; return null; }

  function search(q) {
    q = (q || "").trim().toLowerCase();
    if (!q) return [];
    var scored = [];
    all().forEach(function (p) {
      var n = p.name.toLowerCase(), d = p.desc.toLowerCase(), s = -1, dm = false;
      if (n === q) s = 1000;
      else if (n.indexOf(q) === 0) s = 800 - n.length;
      else if (n.indexOf(q) >= 0) s = 600 - n.length;
      else if (fuzzy(n, q)) s = 300 - n.length;
      else if (d.indexOf(q) >= 0) { s = 100; dm = true; }
      if (s >= 0) scored.push({ p: p, s: s, desc: dm });
    });
    scored.sort(function (a, b) { return b.s - a.s || (a.p.name < b.p.name ? -1 : 1); });
    return scored;
  }
  function fuzzy(s, q) { var i = 0; for (var j = 0; j < s.length && i < q.length; j++) if (s[j] === q[i]) i++; return i === q.length; }

  function install(names) {
    names.forEach(function (n) {
      var i = available.findIndex(function (p) { return p.name === n; });
      if (i >= 0) {
        var p = available.splice(i, 1)[0];
        p.installed = true; p.reason = "explicit"; p.date = today();
        packages.push(p);
      }
    });
    packages.sort(byName); changed("packages");
  }
  function remove(names) {
    names.forEach(function (n) {
      var i = packages.findIndex(function (p) { return p.name === n; });
      if (i >= 0) {
        var p = packages.splice(i, 1)[0];
        p.installed = false; p.reason = null; p.orphan = false;
        available.push(p);
      }
    });
    available.sort(byName); changed("packages");
  }
  function mark(names, reason) {
    names.forEach(function (n) { var p = find(n); if (p && p.installed) { p.reason = reason; if (reason === "explicit") p.orphan = false; } });
    changed("packages");
  }
  function byName(a, b) { return a.name < b.name ? -1 : 1; }
  function today() { return "2026-11-07"; }

  function addApp(app) {
    app.created = today();
    var i = apps.findIndex(function (a) { return a.kind === app.kind && a.name === app.name; });
    if (i >= 0) apps[i] = app; else apps.push(app);
    changed("apps");
    return "~/.local/share/applications/pacpad-" + slug(app.name) + ".desktop";
  }
  function removeApp(kind, name) {
    var i = apps.findIndex(function (a) { return a.kind === kind && a.name === name; });
    if (i < 0) return false;
    apps.splice(i, 1); changed("apps"); return true;
  }

  function counts() {
    var c = { total: packages.length, explicit: 0, dependency: 0, foreign: 0, orphan: 0 };
    packages.forEach(function (p) {
      if (p.reason === "explicit") c.explicit++; else c.dependency++;
      if (p.repo === "aur") c.foreign++;
      if (p.orphan) c.orphan++;
    });
    return c;
  }

  // snapshot for the demo's reset between autoplay loops
  var initial = JSON.stringify({ packages: packages, available: available, apps: apps });
  function reset() {
    var d = JSON.parse(initial);
    packages.length = 0; Array.prototype.push.apply(packages, d.packages);
    available.length = 0; Array.prototype.push.apply(available, d.available);
    apps.length = 0; Array.prototype.push.apply(apps, d.apps);
    changed("reset");
  }

  window.PP = {
    reset: reset,
    packages: packages, available: available, apps: apps,
    find: find, search: search, install: install, remove: remove, mark: mark,
    addApp: addApp, removeApp: removeApp, protection: protection, slug: slug, counts: counts,
    onChange: function (fn) { listeners.push(fn); },
    user: "richie", host: "archbox"
  };
})();
